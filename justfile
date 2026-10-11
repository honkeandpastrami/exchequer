# Developer commands. `just --list` shows them all.

pg_bin := "/opt/homebrew/opt/postgresql@17/bin"
pgdata := justfile_directory() / "server/.pgdata"

# The dev database lives entirely in server/.pgdata/ (git-ignored): data, log
# and socket. It accepts no TCP connections and runs only between db-up and
# db-down.
export PGHOST := pgdata
export PGUSER := "exchequer"
export PGDATABASE := "exchequer"
export PSQL_HISTORY := pgdata / "psql_history"
export DATABASE_URL := "postgres://exchequer@localhost/exchequer?host=" + pgdata

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

# Format, lint and test the server.
check:
    cd server && cargo fmt --check
    cd server && cargo clippy --all-targets -- -D warnings
    cd server && cargo test

# Regenerate the committed OpenAPI document.
spec:
    cd server && cargo run -q -p server -- openapi > ../api/openapi.json

# Run the server against the dev database (just db-up first).
serve:
    cd server && DB_HOST="{{pgdata}}" DB_USER=exchequer DB_NAME=exchequer cargo run -p server
