# exchequer

Exchequer's command line.

```sh
cd python
uv run exchequer login                     # sign in through the web app; writes the token file
uv run exchequer import FILE...            # upload, review, commit bank files
uv run exchequer import FILE... --yes      # take every default, no questions
uv run exchequer rules edit -m "why"       # edit the rules script in $EDITOR, upload on save
uv run exchequer rules log                 # its versions; also show, upload, diff, rollback
uv run exchequer rules dry-run --month 2026-09 [--file candidate.rhai]
```

It reads the server and token from `EXCHEQUER_URL` and `EXCHEQUER_TOKEN_FILE`, else from
`~/.config/exchequer/config.toml`:

```toml
url = "https://example.com/exchequer"
token_file = "~/.config/exchequer/token"   # a file holding only the token
```

`exchequer login` fills the token file: approve its code in the web app, signed in with your
passkey. The token is the CLI's own, revocable on its own, and never printed.
