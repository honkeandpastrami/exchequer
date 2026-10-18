# firefly-migrate

One-time migration from Firefly III into Exchequer (design §11). It reads Firefly's REST
API and writes only through Exchequer's. **Firefly is never modified:** the client refuses
every request but GET before it reaches the network, and a test holds it to that.

```sh
cd python
uv run firefly-migrate plan  MAPPING   # read Firefly, print the report; writes nothing
uv run firefly-migrate run   MAPPING   # plan, then write into an EMPTY Exchequer server
uv run firefly-migrate check MAPPING   # compare Firefly with what Exchequer holds
uv run firefly-migrate settle MAPPING      # the settlement pass, then the settlement check
uv run firefly-migrate settlement MAPPING  # only the settlement check
uv run firefly-migrate numbers MAPPING     # account numbers Firefly has that Exchequer lacks
uv run firefly-migrate numbers MAPPING --apply  # ... and add them (shows last four only)
uv run firefly-migrate fingerprints MAPPING [--apply]  # record which bank rows were imported
```

`MAPPING` is a TOML file you keep privately (never in this repo). It names:

- `[firefly]` `api_base` and `token_file`, a file holding only a Firefly personal access
  token made for the migration (revoke it when done);
- `[exchequer]` `url` and `token_file`, an Exchequer token;
- `[scope]` the `year` to migrate, the cutoff date `through`, and `closed_through`, the
  last month already settled in the old system;
- `[parties]`, `[commodities]` and `[[accounts]]` (several Firefly accounts may collapse
  into one, e.g. one per security into a single brokerage). Account names are unique per
  owner, so two people can each have a `Checking`; elsewhere in the mapping an account is
  its name, or `Owner · Name` when several owners have one by that name;
- `[purpose]` which Firefly budgets and tags mean which party (an income or expense with
  neither is an error, in the dry run too: there's no default);
- `[counterparties]`, `[categories]` and `[tags]`: dividends from Firefly's cash account,
  transfer-only categories to drop, encoding tags to retire;
- `[settlement]` (for `settle`, always in USD): `stipends.<person>` and
  `funders.<business>`, each a table of months to values, carried forward until the next
  month given (amounts as strings, e.g. `"2026-01" = "500.00"`); `settles`, Firefly
  transaction IDs whose settlement payment settles a month other than the one before it
  was made.

See `tests/conftest.py` for a complete example for a fictional household.

What it does: one opening balance per account and commodity on Jan 1 (the exact sum of
everything before), then every transaction of the year up to the cutoff, each keeping its
Firefly ID as an external reference (`firefly`, group ID). The report lists float-damaged
amounts it rounded, conflicting purpose labels, labels it dropped (e.g. a budget on a
transfer) and anything it couldn't migrate. `check` compares balances per account and
commodity at every month end, and category totals per month.

`settle` works on an already migrated ledger and never reads Firefly. It links the listed payments, sets every month's stipends and funders through
`closed_through`, and closes those months. Run it again after changing the mapping: it
changes only what differs, reopening a closed month just long enough. The settlement check
then shows, per person and month, the stipend set next to the *implied* stipend (claims −
paid, design §4.5), and what earlier months carried in. In a closed month, a difference
marked ⚠ is new: a mislabeled transaction, a payment linked to the wrong month, or a past
mistake now carried forward (design §4.4). One marked ↺ pays down what was carried.

Not yet migrated (each comes with the build step that adds it): import batches (step 5),
rules (step 6), bills (step 9).
