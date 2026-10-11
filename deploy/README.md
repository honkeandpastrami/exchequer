# Deploying Exchequer

Exchequer runs as rootless podman containers managed by systemd through
[quadlets](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html)
(design §14). `examples/` has a working set; copy it and keep your real units privately,
since paths and schedules are yours.

| File | What |
|---|---|
| `exchequer.network` | Private network shared by the three containers |
| `exchequer-db.volume`, `exchequer-db.container` | PostgreSQL 17, no published port |
| `exchequer-api.container` | The API, published on `127.0.0.1:8080` |
| `exchequer-backup.container`, `exchequer-backup.timer` | Daily `pg_dump` to a local drive |
| `../exchequer-backup` | The dump/restore script the backup unit runs |

Needs podman 5 or newer (`Notify=healthy`).

## Install

Run these as the (non-root) user that will own the service, from a clone of this repo.

1. Check podman is version 5 or newer, and pick a backup directory, ideally on a separate
   drive. The examples use `/mnt/backup/exchequer`; edit `exchequer-backup.container` if
   yours differs.

   ```sh
   podman --version
   mkdir -p /mnt/backup/exchequer
   ```

2. Create the secrets once. They never appear in a unit file, an env file or this repo.

   ```sh
   openssl rand -base64 32 | tr -d '\n' | podman secret create exchequer_db_password -
   openssl rand -base64 32 | tr -d '\n' | podman secret create exchequer_bootstrap_token -
   ```

3. Build the server image (a few minutes the first time):

   ```sh
   podman build -t exchequer-server .
   podman run --rm localhost/exchequer-server:latest schema-version   # prints a number
   ```

4. Install the backup script where `exchequer-backup.container` mounts it from:

   ```sh
   install -D -m 755 deploy/exchequer-backup ~/.local/lib/exchequer/exchequer-backup
   ```

5. Install the units. Podman reads the container, network and volume files from
   `containers/systemd`; the timer is a plain systemd unit and goes in `systemd/user`.

   ```sh
   mkdir -p ~/.config/containers/systemd ~/.config/systemd/user
   cp deploy/examples/exchequer.network deploy/examples/exchequer-db.volume \
      deploy/examples/*.container ~/.config/containers/systemd/
   cp deploy/examples/exchequer-backup.timer ~/.config/systemd/user/
   systemctl --user daemon-reload
   systemctl --user start exchequer-api.service      # starts the database too
   systemctl --user enable --now exchequer-backup.timer
   loginctl enable-linger "$USER"                    # keep running after logout
   ```

   The API and database start at boot on their own (their `[Install]` sections).
   If `start` fails, `journalctl --user -u exchequer-db -u exchequer-api` says why.

6. Check it answers:

   ```sh
   curl -s http://127.0.0.1:8080/healthz
   curl -s http://127.0.0.1:8080/version
   ```

7. Make the first real token with the bootstrap token (design §10), and save the `token`
   it prints in your password manager: it's shown only once. The bootstrap token goes
   through stdin, so it never shows up in `ps`:

   ```sh
   podman secret inspect --showsecret --format '{{.SecretData}}' exchequer_bootstrap_token |
       sed 's/^/Authorization: Bearer /' |
       curl -s -H @- -H 'Content-Type: application/json' -d '{"name": "admin"}' \
           http://127.0.0.1:8080/api/v1/tokens
   ```

The API applies schema migrations at startup. `GET /healthz` and `GET /version` need no
token.

## First-run rehearsal

Before the database holds anything real, rehearse a backup and a restore once:

1. Take a backup and look at it: `systemctl --user start exchequer-backup.service`, then
   `ls -l /mnt/backup/exchequer`.
2. Follow [Restore](#restore) with that file.
3. The `admin` token from step 7 still works, so the data came back:

   ```sh
   podman secret inspect --showsecret --format '{{.SecretData}}' exchequer_bootstrap_token |
       sed 's/^/Authorization: Bearer /' |
       curl -s -H @- http://127.0.0.1:8080/api/v1/tokens
   ```

## Backups

`exchequer-backup.timer` runs `exchequer-backup dump /backups 30` daily. Each run writes
`exchequer-<UTC time>-v<schema>.dump` (`pg_dump` custom format) and keeps the newest 30.
A run missed while the machine was off happens at the next boot. To run one now:

```sh
systemctl --user start exchequer-backup.service
journalctl --user -u exchequer-backup.service
```

## Restore

Restoring replaces the whole database. It's a command run by hand, never an API endpoint.
The same steps run in the test suite (`just backup-test`).

1. Stop the API, so nothing writes during the restore:

   ```sh
   systemctl --user stop exchequer-api.service
   ```

2. Check the dump isn't newer than the server image. A dump from an older schema is fine:
   the server migrates it at startup.

   ```sh
   max=$(podman run --rm localhost/exchequer-server:latest schema-version)
   ```

3. Replace the database with an empty one (this deletes the current data):

   ```sh
   podman exec exchequer-db dropdb -U exchequer exchequer
   podman exec exchequer-db createdb -U exchequer exchequer
   ```

4. Restore. The script refuses a non-empty database, a dump newer than `$max`, or a file
   not named like a backup.

   ```sh
   podman run --rm --network exchequer --secret exchequer_db_password \
       -e PGHOST=exchequer-db -e PGUSER=exchequer -e PGDATABASE=exchequer \
       -e PGPASSWORD_FILE=/run/secrets/exchequer_db_password \
       -v ~/.local/lib/exchequer/exchequer-backup:/usr/local/bin/exchequer-backup:ro,Z \
       -v /mnt/backup/exchequer:/backups:ro,Z \
       docker.io/library/postgres:17 \
       exchequer-backup restore /backups/exchequer-20260101T033000Z-v0002.dump "$max"
   ```

5. Start the API again and check it:

   ```sh
   systemctl --user start exchequer-api.service
   curl -s http://127.0.0.1:8080/version
   ```
