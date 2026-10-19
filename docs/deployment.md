# Changes, releases and deployment

How a change gets from a branch to the running server. Installing a deployment for the first
time is in [`deploy/README.md`](../deploy/README.md).

```
branch ─▶ pull request ─▶ checks ─▶ merge to main ─▶ release vX.Y.Z ─▶ podman auto-update
```

## Pull requests

- **`main` takes no direct pushes.** A repository rule refuses them ("Changes must be made
  through a pull request"); every change arrives through a pull request. No review is
  required to merge.
- **One pull request, one change.** It's squash-merged, so it becomes one commit on `main`
  whose subject is the pull request's title. That commit is what the release is built from.
- **Everything that belongs together lands together:** an API change, the regenerated spec
  and SDKs (`just spec`, `just sdk`) and the clients that use them (design §16.1).
- **Docs change with the code.** A pull request that changes behavior updates
  [`design.md`](design.md), which describes what's built, and [`ai.md`](ai.md) or
  [`ux.md`](ux.md) where they're affected.
- Issues follow [`issues.md`](issues.md); a pull request that fixes one says so
  (`Fixes #N`), and merging closes it.

## Checks

Three workflows check a pull request, and check `main` again after the merge. Each runs only
when its part of the repository changed:

| Workflow | Runs when these change | Runs |
|---|---|---|
| `server` | `server/`, `api/`, `deploy/`, `web/`, `justfile`, `Containerfile` | `just check`, then builds and smoke-tests both images |
| `web` | `web/`, `api/`, `justfile` | `just web-check` |
| `python` | `python/`, `api/`, `justfile` | `just python-check` |

They run the same `just` recipes as a local checkout, so a change that passes locally
passes in CI. A pull request is merged once its checks pass.

## Versions

- **One version for everything** (server, web app, SDKs, CLI, migration tool), and it lives
  only in git tags `vX.Y.Z`. Nothing in the repository holds it: builds read it from git
  (`deploy/version`; `hatch-vcs` for Python), so a release never edits a file.
- On a release tag a build reports `X.Y.Z`; anywhere else, `X.Y.Z-dev.N+g<commit>` (N
  commits after the last release), plus `.dirty` with uncommitted changes. `just version`
  shows it, and `/version` reports the running server's, with its schema version.
- **The pull request's labels pick the bump:**

  | Label | Bump | Example |
  |---|---|---|
  | none | minor | 0.27.0 → 0.28.0 |
  | `patch` | patch | 0.27.0 → 0.27.1 |
  | `major` | major | 0.27.0 → 1.0.0 |

  `major` wins over `patch` on one pull request. Labels are read when the release runs,
  so they can be changed up to the merge.

## Releases

**Every merge to `main` is a release.** The `release` workflow runs on each push to `main`:

1. **Pick the version** (`deploy/release-version`): the last release's version, bumped
   once by the largest bump among the pull requests merged since. Usually that's one pull
   request; if several merged while a release was running, the next release covers them
   all.
2. **Tag the merge commit** `vX.Y.Z`, locally.
3. **Build** the server image (the web app's static build inside it) and the backup image,
   and **smoke-test** both (`deploy/tests/image-smoke.sh`).
4. **Build the Python packages** (SDK, CLI, migration tool) at that version.
5. **Push the tag,** then **publish** both images to GitHub's container registry as
   `X.Y.Z`, `X.Y` and `latest`, and create the **GitHub Release** `vX.Y.Z` with the Python
   packages attached and notes generated from the merged pull requests.

The tag is pushed only after everything is built and tested, so a failed release leaves
no tag behind. Running the failed workflow again picks the same version; a run on a commit
already tagged reuses its tag. Releases run one at a time.

The images are public: they hold code, never household data (design §16.1).

## Deployment

The server runs the newest release; nothing is built on it (design §14).

- **Updating.** The API's quadlet has `AutoUpdate=registry`: `podman-auto-update.timer`
  pulls a new `latest` and restarts the API on it, overnight. To update now:

  ```sh
  podman auto-update
  ```

  The backup quadlet has `Pull=newer`, so each backup uses the newest release too.
- **Schema migrations run at startup,** each in its own transaction. A migration that
  checks existing data (e.g. that every income and expense has a category) refuses to run
  while the data fails it; the server then doesn't start, the database is left as it was,
  and `podman auto-update` rolls back to the previous image, since the new one never
  became healthy. Fix the data through the API on the running version first, then update.
- **Check it** after an update: `curl -s http://127.0.0.1:7438/version` names the release
  and the schema version, and `journalctl --user -u exchequer-server` has the startup.
- **Going back.** Set the quadlet's `Image=` to the release's version tag
  (`…/exchequer-server:0.27.0`) and restart. A server refuses a database migrated past its
  own schema, so going back across a migration also means restoring the backup taken
  before it (`deploy/restore`, [`deploy/README.md`](../deploy/README.md#restore)); backup
  file names carry the schema version for that.
- If a release can't be pulled, `deploy/build-images` builds both images from a checkout
  under the same names; the next release replaces them.
