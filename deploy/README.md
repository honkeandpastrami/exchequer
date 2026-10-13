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
| `../build-images` | Builds both images from a checkout, as a fallback to the released ones |

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

   Optionally, for remote AI ([`docs/ai.md`](../docs/ai.md#remote-ai)), the Azure AI
   Foundry resource's key, pasted on stdin; then uncomment the remote AI lines in
   `exchequer-api.container`:

   ```sh
   podman secret create exchequer_remote_ai_key -
   ```

3. Pull the server and backup images of the newest release:

   ```sh
   podman pull ghcr.io/honkeandpastrami/exchequer-server:latest \
       ghcr.io/honkeandpastrami/exchequer-backup:latest
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

The web app is served at `/` by the same container: open `http://127.0.0.1:7438/` and
sign in with a token (kept in that browser until you sign out).

## Reverse proxy

TLS and access from other machines are left to a reverse proxy (design §10). The API
and the web app also work under a path, as long as the proxy strips it; with nginx:

```nginx
location /exchequer/ {
    proxy_pass http://127.0.0.1:7438/;   # the trailing / strips /exchequer
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

Open the web app with the trailing slash (`/exchequer/`): its paths are relative to it.

## Updating

Releases are published by GitHub (design §15). Everything in the repo shares one
version, and it lives only in git tags: to release, tag a commit on `main`:

```sh
git tag v0.2.0 && git push origin v0.2.0
```

The `release` workflow builds both images with that version, smoke-tests them, and publishes them as `ghcr.io/honkeandpastrami/exchequer-server` and
`…/exchequer-backup`, tagged `0.2.0`, `0.2` and `latest`, with a GitHub Release carrying
the Python packages.

The API's `AutoUpdate=registry` lets `podman-auto-update.timer` pull the new release and
restart the API on it overnight (enable it once with
`systemctl --user enable --now podman-auto-update.timer`); to update now, run
`podman auto-update`. The backup quadlet has `Pull=newer`, so each run uses the newest
release. To go back to a release, set the quadlet's `Image=` to its version tag.

`deploy/build-images` builds both images from a checkout under the same names, if a
release can't be pulled; the next release replaces them.

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

It stops the API and takes a safety dump of the current database through
`exchequer-backup.service`, so restoring the wrong file can be undone by restoring that
dump. Then it runs `exchequer-backup restore --replace` in the backup image and starts the
API again. The backup script refuses a file not named like a backup or a dump newer than
the server image (an older one is fine: the server migrates it at startup), and checks
both before dropping anything. It also refuses while anything is still connected to the
database. If a step fails, the API is left stopped; fix the problem and run it again.

If the current database is too damaged to dump, the safety dump fails and nothing is
restored; then run `deploy/restore --no-safety-dump FILE`.

The same restore runs in the test suite (`just backup-test`).
