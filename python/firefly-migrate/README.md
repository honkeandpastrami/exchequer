# firefly-migrate

One-time migration from Firefly III into Exchequer (design §11). It reads Firefly's REST
API and writes only through Exchequer's. **Firefly is never modified:** the client refuses
every request but GET before it reaches the network, and a test holds it to that.

```sh
cd python
uv run firefly-migrate plan  MAPPING   # read Firefly, print the report; writes nothing
uv run firefly-migrate run   MAPPING   # plan, then write into an EMPTY Exchequer server
uv run firefly-migrate check MAPPING   # compare Firefly with what Exchequer holds
```

`MAPPING` is a TOML file you keep privately (never in this repo). It names:

- `[firefly]` `api_base` and `token_file`, a file holding only a Firefly personal access
  token made for the migration (revoke it when done);
- `[exchequer]` `url` and `token_file`, an Exchequer token;
- `[scope]` the `year` to migrate and the cutoff date `through`;
- `[parties]`, `[commodities]` and `[[accounts]]` (several Firefly accounts may collapse
  into one, e.g. one per security into a single brokerage);
- `[purpose]` which Firefly budgets and tags mean which party (unlabeled splits get the
  account's owner);
- `[counterparties]`, `[categories]` and `[tags]`: dividends from Firefly's cash account,
  transfer-only categories to drop, encoding tags to retire.

See `tests/conftest.py` for a complete example for a fictional household.

What it does: one opening balance per account and commodity on Jan 1 (the exact sum of
everything before), then every transaction of the year up to the cutoff, each keeping its
Firefly ID as an external reference (`firefly`, group ID). The report lists float-damaged
amounts it rounded, conflicting purpose labels, labels it dropped (e.g. a budget on a
transfer) and anything it couldn't migrate. `check` compares balances per account and
commodity at every month end, and category totals per month.

Not yet migrated (each comes with the build step that adds it): settlement links and
closed months (step 4), import batches (step 5), rules (step 6), bills (step 9).
