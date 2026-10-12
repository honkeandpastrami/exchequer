# exchequer

Exchequer's command line.

```sh
cd python
uv run exchequer import FILE...            # upload, review, commit bank files
uv run exchequer import FILE... --yes      # take every default, no questions
```

It reads the server and token from `EXCHEQUER_URL` and `EXCHEQUER_TOKEN_FILE`, else from
`~/.config/exchequer/config.toml`:

```toml
url = "https://example.com/exchequer"
token_file = "~/.config/exchequer/token"   # a file holding only the token
```

Make a token for the CLI alone, so it can be revoked on its own. The token is never printed.
