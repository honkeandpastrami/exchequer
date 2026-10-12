#!/bin/sh
# image-smoke.sh: start a built server image against a database on localhost
# and check it answers; check the backup image has its tools.
#
#   deploy/tests/image-smoke.sh SERVER_IMAGE BACKUP_IMAGE [VERSION]
#
# With VERSION, /version must report it. Uses docker unless CONTAINER is set
# (e.g. CONTAINER=podman). CI runs it against a Postgres service.

set -eu

server=$1
backup=$2
version=${3:-}
run=${CONTAINER:-docker}

$run run --rm "$server" schema-version
$run run -d --name exchequer-smoke --network host \
    -e DB_HOST=localhost -e DB_USER=exchequer -e DB_NAME=exchequer \
    -e EXCHEQUER_BIND=127.0.0.1:7438 "$server" > /dev/null
trap '$run logs exchequer-smoke; $run rm -f exchequer-smoke > /dev/null' EXIT

for _ in $(seq 30); do
    curl -sf http://127.0.0.1:7438/healthz > /dev/null && break
    sleep 1
done
reported=$(curl -sf http://127.0.0.1:7438/version)
echo "$reported"
if [ -n "$version" ]; then
    echo "$reported" | grep -q "\"version\":\"$version\"" || {
        echo "expected version $version" >&2
        exit 1
    }
fi
# The web app is served at / (design §13).
curl -sf http://127.0.0.1:7438/ | grep -q '_app/immutable'
test "$($run inspect -f '{{.State.Running}}' exchequer-smoke)" = true
$run run --rm "$backup" sh -c 'command -v exchequer-backup && pg_dump --version && pg_restore --version'
