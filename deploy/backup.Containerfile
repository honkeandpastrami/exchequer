# The backup image: the postgres image (pg_dump, pg_restore, psql) plus
# deploy/exchequer-backup. Build it with deploy/ as the context:
#   podman build -t exchequer-backup -f deploy/backup.Containerfile deploy
FROM docker.io/library/postgres:17
COPY --chmod=755 exchequer-backup /usr/local/bin/exchequer-backup
