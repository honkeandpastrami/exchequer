# Developer commands. `just --list` shows them all.

pg_bin := env("EXCHEQUER_PG_BIN", "/opt/homebrew/opt/postgresql@17/bin")
pgdata := justfile_directory() / "server/.pgdata"

# The dev database lives entirely in server/.pgdata/ (git-ignored): data, log
# and socket. It accepts no TCP connections and runs only between db-up and
# db-down. CI points EXCHEQUER_PGHOST at its own database instead (prefixed,
# so a PGHOST already in your shell never redirects the dev recipes).
export PGHOST := env("EXCHEQUER_PGHOST", pgdata)
export PGUSER := "exchequer"
export PGDATABASE := "exchequer"
export PSQL_HISTORY := pgdata / "psql_history"
export DATABASE_URL := "postgres://exchequer@localhost/exchequer?host=" + PGHOST

[private]
default:
    @just --list

# Start the dev database, creating it on first use.
db-up:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -d "{{pgdata}}/data" ]; then
        mkdir -p "{{pgdata}}" && chmod 700 "{{pgdata}}"
        "{{pg_bin}}/initdb" -D "{{pgdata}}/data" -U exchequer --auth=trust \
            -E UTF8 --locale=en_US.UTF-8 > "{{pgdata}}/initdb.log"
    fi
    "{{pg_bin}}/pg_ctl" -D "{{pgdata}}/data" -l "{{pgdata}}/postgres.log" -w \
        -o "-c listen_addresses='' -c unix_socket_directories='{{pgdata}}'" start
    "{{pg_bin}}/psql" -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname = 'exchequer'" \
        | grep -q 1 || "{{pg_bin}}/createdb" exchequer
    # For tools that read .env (sqlx macros, editors).
    echo "DATABASE_URL=$DATABASE_URL" > "{{justfile_directory()}}/server/.env"
    # The sqlx macros check queries against this schema at compile time.
    cd "{{justfile_directory()}}/server" && cargo sqlx migrate run

# Stop the dev database.
db-down:
    "{{pg_bin}}/pg_ctl" -D "{{pgdata}}/data" -m fast stop

# Delete the dev database entirely and start a fresh one.
db-reset:
    -"{{pg_bin}}/pg_ctl" -D "{{pgdata}}/data" -m fast stop
    rm -rf "{{pgdata}}"
    just db-up

# Open psql on the dev database.
psql:
    "{{pg_bin}}/psql"

# Format, lint and test everything (the dev database must be up).
check:
    cd server && cargo fmt --check
    cd server && cargo sqlx prepare --workspace --check -- --all-targets
    cd server && cargo clippy --all-targets -- -D warnings
    cd server && cargo deny check
    shellcheck deploy/exchequer-backup deploy/build-images deploy/tests/backup.sh
    cd server && cargo test
    just backup-test

# Dump a scratch database with deploy/exchequer-backup and restore it into
# another (the dev database must be up).
backup-test:
    PATH="{{pg_bin}}:$PATH" deploy/tests/backup.sh

# Regenerate the committed OpenAPI document.
spec:
    cd server && cargo run -q -p server -- openapi > ../api/openapi.json.tmp
    mv api/openapi.json.tmp api/openapi.json

# Regenerate the committed query cache (server/.sqlx), so builds work
# without a database. Run after changing any query or migration.
prepare:
    cd server && cargo sqlx prepare --workspace -- --all-targets

# Run the server against the dev database (just db-up first).
serve:
    cd server && DB_HOST="{{pgdata}}" DB_USER=exchequer DB_NAME=exchequer cargo run -p server
