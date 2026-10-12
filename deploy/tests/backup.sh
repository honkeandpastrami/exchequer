#!/bin/sh
# Restore test for exchequer-backup (design §14.2): fill a scratch database,
# dump it, restore the dump into a second one, and check they match. Also
# checks the refusals and the pruning.
#
# Needs a running Postgres reachable through the libpq variables (PGHOST,
# PGUSER), with pg_dump and pg_restore on PATH, and the server built:
# `just backup-test` sets all of that up for the dev database.

set -eu
export LC_ALL=C

here=$(cd "$(dirname "$0")" && pwd)
backup="$here/../exchequer-backup"
server="$here/../../server"
src=exchequer_backup_src
dst=exchequer_backup_dst
work=$(mktemp -d)
trap 'rm -rf "$work"; dropdb --if-exists $src; dropdb --if-exists $dst' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

# Runs exchequer-backup, expecting it to fail with a message containing $1.
refuses() {
    expected=$1
    shift
    if "$@" > "$work/out" 2>&1; then
        fail "expected a refusal ($expected): $*"
    fi
    grep -q "$expected" "$work/out" || fail "wrong message for $*: $(cat "$work/out")"
}

# Data and schema of a database, without lines that differ between dumps.
snapshot() {
    pg_dump --no-owner --no-acl --dbname="$1" | grep -v '^\\\(un\)\{0,1\}restrict '
}

latest=$(cd "$server" && cargo run -q -p server -- schema-version)

for db in $src $dst; do
    PGOPTIONS='-c client_min_messages=warning' dropdb --if-exists "$db"
    createdb "$db"
done
(cd "$server" && cargo sqlx migrate run -D "postgres:///$src?host=$PGHOST" > /dev/null)

# Values that are easy to damage: long exact decimals, non-ASCII text, NULLs.
psql -XAq -v ON_ERROR_STOP=1 -d $src <<'SQL'
INSERT INTO commodity (id, code, kind, precision) VALUES
    ('01900000-0000-7000-8000-000000000001', 'USD', 'fiat', 2),
    ('01900000-0000-7000-8000-000000000002', 'VTSAX', 'fund', 6);
INSERT INTO party (id, name, kind) VALUES
    ('01900000-0000-7000-8000-000000000003', 'Household', 'household');
INSERT INTO account (id, name, kind, owner_id, liquid) VALUES
    ('01900000-0000-7000-8000-000000000004', 'Brokerage', 'brokerage',
     '01900000-0000-7000-8000-000000000003', false);
INSERT INTO import_batch (id, kind) VALUES
    ('01900000-0000-7000-8000-000000000006', 'manual');
INSERT INTO transaction (id, date, description, notes, import_batch_id) VALUES
    ('01900000-0000-7000-8000-000000000005', '2026-01-02', 'Café ☕ purchase', NULL,
     '01900000-0000-7000-8000-000000000006');
INSERT INTO split (transaction_id, position,
                   from_account_id, from_commodity_id, from_quantity,
                   to_account_id, to_commodity_id, to_quantity) VALUES
    ('01900000-0000-7000-8000-000000000005', 0,
     '01900000-0000-7000-8000-000000000004', '01900000-0000-7000-8000-000000000001',
     12345678901234567890.12,
     '01900000-0000-7000-8000-000000000004', '01900000-0000-7000-8000-000000000002',
     0.000001);
SQL

# Dump.
mkdir "$work/backups"
PGDATABASE=$src "$backup" dump "$work/backups" > /dev/null
set -- "$work/backups"/*
[ "$#" -eq 1 ] || fail "expected one dump, found: $*"
dump=$1
[ -n "$(find "$dump" -perm 600)" ] || fail "the dump is readable by others: $(ls -l "$dump")"
name=$(basename "$dump")
expected_suffix=$(printf -- '-v%04d.dump' "$latest")
case $name in
    exchequer-[0-9]*T[0-9]*Z"$expected_suffix") ;;
    *) fail "unexpected dump name $name" ;;
esac

# Refusals, before anything is restored.
refuses "newer than this server" env PGDATABASE=$dst "$backup" restore "$dump" $((latest - 1))
refuses "not found" env PGDATABASE=$dst "$backup" restore "$work/backups"
cp "$dump" "$work/renamed.dump"
refuses "not named like a backup" env PGDATABASE=$dst "$backup" restore "$work/renamed.dump"
refuses "not empty" env PGDATABASE=$src "$backup" restore "$dump" "$latest"
refuses "use PGPASSWORD_FILE" env PGPASSWORD=x PGDATABASE=$dst "$backup" restore "$dump"

# Restore, through a password file (ignored by the dev database's trust auth,
# but the file handling runs). The copy matches the original exactly.
printf 'pa:ss\\word' > "$work/password"
PGDATABASE=$dst PGPASSWORD_FILE="$work/password" "$backup" restore "$dump" "$latest" > /dev/null
snapshot $src > "$work/src.sql"
snapshot $dst > "$work/dst.sql"
diff -u "$work/src.sql" "$work/dst.sql" || fail "the restored database differs"
grep -q '12345678901234567890.12' "$work/dst.sql" || fail "decimal not restored"

# The server accepts the restored schema: migration checksums still match.
(cd "$server" && cargo sqlx migrate run -D "postgres:///$dst?host=$PGHOST" > /dev/null)

# A second restore over the restored data is refused.
refuses "not empty" env PGDATABASE=$dst "$backup" restore "$dump" "$latest"

# --replace checks the dump before dropping anything: a refusal leaves the data.
refuses "newer than this server" env PGDATABASE=$dst "$backup" restore --replace "$dump" $((latest - 1))
refuses "not named like a backup" env PGDATABASE=$dst "$backup" restore --replace "$work/renamed.dump"
[ "$(psql -XAtq -d $dst -c 'SELECT count(*) FROM split')" -eq 1 ] ||
    fail "--replace touched the database before refusing"

# --replace swaps whatever is there for the dump.
psql -XAq -v ON_ERROR_STOP=1 -d $dst -c 'DELETE FROM split'
PGDATABASE=$dst "$backup" restore --replace "$dump" "$latest" > /dev/null
snapshot $dst > "$work/dst.sql"
diff -u "$work/src.sql" "$work/dst.sql" || fail "the replaced database differs"

# A run that failed between drop and create can be repeated: --replace also
# works when the database is missing.
dropdb $dst
PGOPTIONS='-c client_min_messages=warning' PGDATABASE=$dst \
    "$backup" restore --replace "$dump" "$latest" > /dev/null
snapshot $dst > "$work/dst.sql"
diff -u "$work/src.sql" "$work/dst.sql" || fail "restore into a missing database differs"

# Pruning keeps the newest KEEP dumps and leaves other files alone.
rm "$dump"
for t in 20260101T000000Z 20260102T000000Z 20260103T000000Z; do
    touch "$work/backups/exchequer-$t-v0001.dump"
done
touch "$work/backups/notes.txt"
PGDATABASE=$src "$backup" dump "$work/backups" 2 > /dev/null
remaining=$(cd "$work/backups" && printf '%s\n' * | sed 's/^exchequer-2.*Z'"$expected_suffix"'$/NEW/' | tr '\n' ' ')
[ "$remaining" = "exchequer-20260103T000000Z-v0001.dump NEW notes.txt " ] ||
    fail "pruning left: $remaining"

echo "backup test passed"
