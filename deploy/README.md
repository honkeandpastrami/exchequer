# Deploying Exchequer

Exchequer runs as rootless podman containers managed by systemd through
[quadlets](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html)
(design §14). `examples/` has a working set; copy it and keep your real units privately,
since paths and schedules are yours.

| File | What |
|---|---|
| `exchequer.network` | Private network shared by the three containers |
| `exchequer-db.volume`, `exchequer-db.container` | PostgreSQL 17, no published port |
| `exchequer-api.container` | The API, published on `127.0.0.1:7438` |
| `exchequer-backup.container`, `exchequer-backup.timer` | Daily `pg_dump` to a local drive |
| `../exchequer-backup`, `../backup.Containerfile` | The dump/restore script, and the backup image that carries it |
| `../build-images` | Builds the server and backup images |

Needs podman 5 or newer (`Notify=healthy`).

## Install

Run these as the (non-root) user that will own the service, from a clone of this repo.

1. Check podman is version 5 or newer, and pick a backup directory, ideally on a separate
   drive. The examples use `/mnt/backup/exchequer`; edit `exchequer-backup.container` if
   yours differs.

   ```sh
   podman --version
   mkdir -p -m 700 /mnt/backup/exchequer
   ```

2. Create the secrets once. They never appear in a unit file, an env file or this repo.

   ```sh
   openssl rand -base64 32 | tr -d '\n' | podman secret create exchequer_db_password -
   openssl rand -base64 32 | tr -d '\n' | podman secret create exchequer_bootstrap_token -
   ```

3. Build the server and backup images (a few minutes the first time):

   ```sh
   deploy/build-images
   ```

4. Install the units. Podman reads the container, network and volume files from
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

5. Check it answers:

   ```sh
   curl -s http://127.0.0.1:7438/healthz
   curl -s http://127.0.0.1:7438/version
   ```

6. Make the first real token with the bootstrap token (design §10), and save the `token`
   it prints in your password manager: it's shown only once. The bootstrap token goes
   through stdin, so it never shows up in `ps`:

   ```sh
   podman secret inspect --showsecret --format '{{.SecretData}}' exchequer_bootstrap_token |
       sed 's/^/Authorization: Bearer /' |
       curl -s -H @- -H 'Content-Type: application/json' -d '{"name": "admin"}' \
           http://127.0.0.1:7438/api/v1/tokens
   ```

The API applies schema migrations at startup. `GET /healthz` and `GET /version` need no
token.

## Reverse proxy

TLS and access from other machines are left to a reverse proxy (design §10). The API
also works under a path, as long as the proxy strips it; with nginx:

```nginx
location /exchequer/ {
    proxy_pass http://127.0.0.1:7438/;   # the trailing / strips /exchequer
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

## Updating

Pull, then rebuild both images with `deploy/build-images`. The API's `AutoUpdate=local`
lets `podman-auto-update.timer` restart it on the new image overnight (enable it once with
`systemctl --user enable --now podman-auto-update.timer`); to switch now, run
`systemctl --user restart exchequer-api.service`. The backup uses the new image on its
next run.

Both images carry the label `local-build`: they can't be pulled again, so keep them out
of any image prune, e.g. `podman image prune -af --filter label!=local-build`.

## First-run rehearsal

Before the database holds anything real, rehearse a backup and a restore once:

1. Take a backup and look at it: `systemctl --user start exchequer-backup.service`, then
   `ls -l /mnt/backup/exchequer`.
2. Follow [Restore](#restore) with that file.
3. The `admin` token from step 6 still works, so the data came back:

   ```sh
   podman secret inspect --showsecret --format '{{.SecretData}}' exchequer_bootstrap_token |
       sed 's/^/Authorization: Bearer /' |
       curl -s -H @- http://127.0.0.1:7438/api/v1/tokens
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

```sh
deploy/restore /mnt/backup/exchequer/exchequer-20260101T033000Z-v0002.dump
```

It stops the API, runs `exchequer-backup restore --replace` in the backup image, and
starts the API again. The backup script refuses a file not named like a backup or a dump
newer than the server image (an older one is fine: the server migrates it at startup),
and checks both before dropping anything. It also refuses while anything is still
connected to the database. If a step fails, the API is left stopped; fix the problem and
run it again. The same restore runs in the test suite (`just backup-test`).
