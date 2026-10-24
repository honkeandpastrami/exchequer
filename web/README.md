# Exchequer web app

SvelteKit with the static adapter (design §13). The build is served by the API
container at `/`, so it talks to `/api/v1` on the same origin. Routes live in the URL
hash and every path is relative, so it works under any proxy prefix.

```sh
npm ci
npm run dev      # http://localhost:5173, API proxied to 127.0.0.1:7438 (EXCHEQUER_API)
npm run gen      # regenerate src/lib/api/schema.d.ts from ../api/openapi.json
just web-check   # from the repo root: what CI runs
```

Sign in with a passkey (run the API with `EXCHEQUER_ORIGIN=http://localhost:5173` and
bootstrap a user as in [`deploy/README.md`](../deploy/README.md#install), step 6), or by
pasting a token; it's kept in the browser's localStorage until you sign out.

## Layout

- `src/lib/theme/tokens.css`: every font, color, size and spacing, light and dark.
  Restyling the app means editing this file; components use only these variables.
- `src/lib/theme/base.css`: element defaults built from the tokens.
- `src/lib/api/`: the generated types (`schema.d.ts`, committed) and the client.
- `src/lib/money.ts`: amounts stay decimal strings; sums use `big.js`. ESLint rejects
  `parseFloat` and `Number`.
- `src/lib/views/`: data shaping for each page, kept out of components and unit-tested.
- `src/lib/components/`, `src/routes/`: the UI.
