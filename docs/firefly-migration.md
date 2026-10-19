# Migrating from Firefly III

How to use `firefly-migrate` (design §11): the mapping file, how a year fits the server's
data, and the commands. The commands are listed in
[`python/firefly-migrate/README.md`](../python/firefly-migrate/README.md).

## The mapping file

A TOML file you keep privately, never in this repository; one per year migrated.
`tests/conftest.py` in the tool has a complete example for a fictional household.

- `[firefly]`: `api_base` and `token_file`, a file holding only a Firefly personal access
  token made for the migration (revoke it when done).
- `[exchequer]`: `url` and `token_file`, an Exchequer token.
- `[scope]`: the `year`, the cutoff date `through`, and `closed_through`, the last month
  already settled in the old system (for `settle`).
- `[parties]`, `[commodities]` and `[[accounts]]`. Several Firefly accounts may collapse
  into one, e.g. one per security into a single brokerage. Account names are unique per
  owner; elsewhere in the mapping an account is its name, or `Owner · Name` when several
  owners have one by that name. A non-liquid account names the categories of money into
  and out of it, `category_in` and `category_out` (e.g. `"Invest"` and `"Divest"`):
  Firefly recorded those as transfers, with no category.
- `[purpose]`: which Firefly budgets and tags mean which party.
- `[counterparties]`: `cash_account_by_destination`, the institution behind Firefly's cash
  account on dividends; `rename`, Firefly names to counterparty names.
- `[categories]`: `drop`, transfer-only categories; `rename`, Firefly names to category
  names.
- `[tags]`: `retire`, tags that only encoded labels; `import_batch_prefix`, the prefix of
  Firefly's import tags.
- `[transactions]`: decisions for single Firefly transactions, by ID: a `purpose`, a
  `category`, or `reconciliation = true` (an adjustment: no purpose or category, the other
  side becomes the reconciliation counterparty). They win over Firefly's labels and the
  mapping's defaults. An ID that isn't in the year is an error.
- `[settlement]` (for `settle`, always in USD): `stipends.<person>` and
  `funders.<business>`, each a table of months to values carried forward until the next
  month given (amounts as strings, e.g. `"2026-01" = "500.00"`); `settles`, Firefly IDs
  of payments that settle a month other than the one before they were made.

## Labels

There's no default purpose or category (design §4.1). An income or expense whose Firefly
data has no budget or purpose tag, or no category, gets the label from, in order:

1. the mapping's `[transactions]` decision for it;
2. the server's copy of the transaction (with `--overlap`).

Without either, it stops the run. Before the year only the amounts matter (they're summed
into the opening balances), so missing labels there are only counted.

## How a year fits the server

`plan` reads Firefly and the server and shows what `run` would write, create and replace,
and every reason it would stop. `run` writes nothing while there is one. Take a backup
before a `run` into a server in use.

- **An empty server:** one opening balance per account and commodity on Jan 1 (the exact
  sum of everything before), then every transaction of the year up to the cutoff, each
  keeping its Firefly ID as an external reference.
- **Before the server's years:** the server's opening balances on the day after the cutoff
  are replaced by the year's end, which must match them to the cent. Set `through` to
  Dec 31. Their month is reopened for it if closed, then closed again.
- **After them:** no opening balances; the server's balances on Jan 1 must be Firefly's.
- **Dates the server already has data for** stop it, unless `--overlap`: then transactions
  already there unchanged are skipped (a run cut short can be run again), and anything
  else there, changed or not from Firefly, stops it.
- **Always stops:** near duplicates, bank rows already recorded, Firefly IDs already on
  the server, writes into closed months, and parties, commodities or accounts the server
  has with another kind.

Reference data is reused by name and created when missing. Transactions get one import
batch per Firefly import tag, and one for the rest. Not migrated: rules (the rules script
starts empty, design §8) and bills.

## The report

Float-damaged amounts it rounded, conflicting purpose labels (the budget wins), labels it
dropped (e.g. a budget on a transfer), labels Firefly lacks, and anything it couldn't
migrate.

## After `run`

- `check` compares balances per account and commodity at every month end, and category
  totals per month.
- `settle` works on the migrated ledger and never reads Firefly. It links the payments in
  `settles`, sets every month's stipends and funders through `closed_through`, and closes
  those months; run again, it changes only what differs. The settlement check then shows,
  per person and month, the stipend set next to the *implied* stipend (claims − paid,
  design §4.5) and what earlier months carried in. In a closed month a difference marked
  ⚠ is new (a mislabeled transaction, a payment linked to the wrong month, or a past
  mistake carried forward, design §4.4); one marked ↺ pays down what was carried.
- `numbers` and `fingerprints` add the account numbers and the bank rows' IDs, so later
  imports resolve accounts and recognize rows already migrated (design §6).
