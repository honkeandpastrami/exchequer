# Developer commands. `just --list` shows them all.

pg_bin := env("EXCHEQUER_PG_BIN", "/opt/homebrew/opt/postgresql@17/bin")
pgdata := justfile_directory() / "server/.pgdata"

# The dev database lives entirely in server/.pgdata/ (git-ignored): data, log
# and socket. It accepts no TCP connections and runs only between db-up and
# db-down. CI points EXCHEQUER_PGHOST at its own database instead (prefixed,
# so a PGHOST already in your shell never redirects the dev recipes).
sdk_generator := "0.29.1"
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
    shellcheck deploy/exchequer-backup deploy/build-images deploy/restore deploy/tests/backup.sh
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

# Regenerate the committed Python SDK (python/sdk) from api/openapi.json.
sdk:
    cd python && uvx --quiet openapi-python-client@{{sdk_generator}} generate \
        --path ../api/openapi.json --config sdk-config.yaml --meta uv \
        --output-path sdk --overwrite

# Lint, type-check and test the Python workspace; fail if the SDK is stale.
python-check:
    cd python && uv sync --locked --quiet
    cd python && uv run ruff format --check
    cd python && uv run ruff check
    cd python && uv run ty check
    cd python && uv run pytest -q
    # No GPL dependencies (design §16.4).
    cd python && uv run --quiet --with pip-licenses pip-licenses --partial-match --fail-on GPL > /dev/null
    # No floats near money (design §3.5): the generated SDK included.
    ! grep -rnE --include='*.py' 'float\(|: *float\b|-> *float\b' python/sdk python/firefly-migrate
    just sdk
    git diff --exit-code -- python/sdk
    test -z "$(git ls-files --others --exclude-standard -- python/sdk)"

# Regenerate the committed query cache (server/.sqlx), so builds work
# without a database. Run after changing any query or migration.
prepare:
    cd server && cargo sqlx prepare --workspace -- --all-targets

# Run the server against the dev database (just db-up first).
serve:
    cd server && DB_HOST="{{pgdata}}" DB_USER=exchequer DB_NAME=exchequer cargo run -p server

# The web app: format, lint, types, tests, licences, the generated API types
# matching api/openapi.json, and a production build.
web-check:
    cd web && npm ci --no-audit --no-fund --loglevel=error
    cd web && npm run lint
    cd web && npm run check
    cd web && npm test
    # No GPL dependencies (design §16.4).
    cd web && npx --yes license-checker-rseidelsohn@5.0.1 --excludePrivatePackages --summary --failOn 'GPL-2.0;GPL-3.0;AGPL-3.0;LGPL-2.1;LGPL-3.0;GPL-2.0-only;GPL-3.0-only;GPL-2.0-or-later;GPL-3.0-or-later;AGPL-3.0-only;AGPL-3.0-or-later;LGPL-2.1-only;LGPL-3.0-only;LGPL-2.1-or-later;LGPL-3.0-or-later' > /dev/null
    cd web && npm run build
    cd web && npm run gen > /dev/null
    git diff --exit-code -- web/src/lib/api
