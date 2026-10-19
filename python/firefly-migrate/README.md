# firefly-migrate

One-time migration from Firefly III into Exchequer, one year at a time. It reads Firefly's
REST API and writes only through Exchequer's; **Firefly is never modified** (the client
refuses every request but GET). How to use it, the mapping file and what each command
does: [`docs/firefly-migration.md`](../../docs/firefly-migration.md).

```sh
cd python
uv run firefly-migrate plan  MAPPING [--overlap]  # the dry run: report and fit; writes nothing
uv run firefly-migrate run   MAPPING [--overlap]  # plan, then write the year
uv run firefly-migrate check MAPPING              # compare Firefly with what Exchequer holds
uv run firefly-migrate settle MAPPING             # the settlement pass, then its check
uv run firefly-migrate settlement MAPPING         # only the settlement check
uv run firefly-migrate numbers MAPPING [--apply]  # account numbers Exchequer lacks
uv run firefly-migrate fingerprints MAPPING [--apply]  # bank rows already imported
```
