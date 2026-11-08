# Exchequer — design

What each piece of Exchequer is for, and how the pieces connect. It's the map a reviewer
reads to know where a change belongs and what else it touches. It isn't a repository of
details: endpoints, fields, parameters and algorithms are in the source and the spec, and
how each part operates is in its own doc:

| Doc | What it holds |
|---|---|
| [`requirements.md`](requirements.md) | Why Exchequer exists, what it must do |
| [`import.md`](import.md) | How import behaves: duplicates, transfers, review, commit |
| [`ai.md`](ai.md) | How suggestions behave: rules, local AI, remote AI |
| [`ux.md`](ux.md) | How the web app looks and behaves |
| [`firefly-migration.md`](firefly-migration.md) | Migrating from Firefly III |
| [`deployment.md`](deployment.md) | Pull requests, releases, deployment |
| [`deploy/README.md`](../deploy/README.md) | Installing and operating a server |
| [`issues.md`](issues.md) | How issues are posted; work not built is tracked as issues |

**Section numbers are a stable contract.** Code, migrations and docs cite this doc as
`design §N`, so a section is never renumbered: new sections are appended, and a section
whose content moves elsewhere stays as a short stub that links to it.

Examples use a **fictional household**: *Alex* and *Sam*, a small business *Studio* run by
Sam, and the *Household* itself. Amounts are round and illustrative.

## 1. Guiding principles

1. **The household flow is the reason this app exists.** Every account has an *owner* and
   every income or expense a *purpose* (who it was for); the monthly settlement is computed
   from those two labels (§4). Where another choice conflicts with this flow, the flow wins.
2. **The server is a pure REST service over a database.** It acts only when a request comes
   in: no cron jobs, timers or background workers, so it can sleep for days and lose nothing.
3. **The server reaches the internet only because of a request** (§19.3). Clients do the
   local work: reading files and talking to people.
4. **The API contract is the product.** The OpenAPI spec is the boundary; the CLI and the
   web app are clients like any other, and neither is privileged.
5. **Store facts, derive everything else.** Balances, holdings, settlements and reports are
   computed from stored transactions when asked. Editing a transaction changes every result
   that depends on it.
6. **Every amount carries a commodity.** Currencies, stocks, funds and crypto are all
   commodities; an account holds any number of them, and converting is an ordinary
   transaction.
7. **No floating point for money, anywhere** (§19.1).
8. **Import is a server feature.** Parsing, deduplication, account detection and suggestions
   happen on the server; a client uploads raw bytes and reviews the server's proposal.

## 2. Topology

```
 ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
 │     CLI      │   │   Web app    │   │ any other    │
 │ import,rules │   │ everything   │   │ client       │
 └──────┬───────┘   └──────┬───────┘   └──────┬───────┘
        │   HTTPS + bearer token (OpenAPI contract)
        └──────────────────┼──────────────────┘
                    ┌──────▼───────┐        outbound only on request:
                    │  API server  │ ─────▶ FX / quote provider (prices)
                    │  (stateless) │ ─────▶ remote AI (suggestions, §8; optional)
                    └──────┬───────┘
                    ┌──────▼───────┐
                    │  PostgreSQL  │  ◀── pg_dump timer (host systemd)
                    └──────────────┘
   podman (user-level quadlets): api + db on a private network
```

Outside data arrives two ways: **prices** the server fetches itself, lazily, when a
valuation needs one it lacks (§3.4); **bank files** the user downloads and a client uploads.

**Where each job lives:**

| Job | Server crate | Web app | CLI |
|---|---|---|---|
| Ledger rules: kinds, invariants, settlement math (§3, §4) | `ledger` | everywhere | — |
| Storage, migrations | `store` | — | — |
| HTTP, auth, the spec (§5, §10) | `api` | Setup → Security | `login`, `tokens`, `sessions` |
| Valuation and prices (§3.4) | `ledger::valuation` (choosing a price), `api` (fetching), `external-calls` (sources) | Accounts, Reports, Setup → Prices | `prices refresh` |
| Settlement (§4) | `ledger`, `api` | Settlement | — |
| Import: parse, match, stage, commit (§6) | `bank-import`, `api` | Imports | `import` |
| Suggestions (§8) | `suggest`, `suggest-rules`, `suggest-local-ai`, `suggest-remote-ai` | Imports (review) | `rules` |
| Reports and queries (§7) | `ledger::query`, `api` | Reports, Accounts | — |
| Balance checks (§7.1) | `ledger::discrepancy`, `api` | Checks | — |
| Data leaving the server (§19.2, §19.3) | `external-calls` | — | — |
| Configuration, startup | `server` | — | — |
| Migration from Firefly (§11) | — (a client) | — | `firefly-migrate` |

## 3. Core domain model

### 3.1 Entities

| Entity | Its job |
|---|---|
| **Commodity** | A unit amounts are counted in: a currency, a security, a fund, crypto; it sets the precision |
| **Party** | Who money belongs to: the *Household*, a person or a business, arranged as the settlement tree (§4.1) |
| **Account** | Something of **yours** that holds value. It has an **owner** (a party) and is **liquid** or not (§4.1); it holds any commodities, with no fixed currency. Its kind only changes how it's shown. Bank files name it by its account numbers |
| **Counterparty** | Anyone **outside** you: merchants, employers, agencies. No direction (the same store sells and refunds), cheap to create, mergeable, with aliases. Two system counterparties take adjustments: *Opening balances* and *Reconciliation* |
| **Transaction** | One real-world event: a date, a description, its splits, and where it came from (its import batch and bank rows) |
| **Split** | One movement inside a transaction, from one leg to another, each leg an account or a counterparty (or an unknown outside) with its commodity and quantity. An income or expense also names its **purpose** and **category** (§4.1); a settlement payment may name the month it settles (§4.3) |
| **Category / Tag** | A category classifies an income or expense (*Groceries*, *Salary*), never where money went. Tags are free-form. There are no budgets: owner and purpose do their job |
| **Settlement** | One month's closing: open or closed, with each person's stipend and each business's funder for the month. Its amounts are always computed live (§4.4) |
| **Price** | `1 BASE = rate QUOTE` on a date, entered by hand or fetched from a source (§3.4) |
| **RuleScript** | The import rules: one script, versioned, append-only (§8) |
| **BalanceCheck** | What the bank says an account held on a date (§7.1) |
| **ImportBatch / staged import** | The import workflow's state (§6) |
| **AuditLog** | Append-only history of which token changed what |
| **User / Passkey / Token** | Who signs in, how, and the credentials clients use (§10) |

### 3.2 There are no transaction types

The kind of a split is **never stored and never entered**. It's derived from who is on each
side:

| From | To | Derived kind |
|---|---|---|
| yours | external | expense |
| external | yours | income |
| yours, liquid | yours, not liquid (an investment, a mortgage) | expense |
| yours, not liquid | yours, liquid | income |
| yours | yours, same commodity, both liquid or both not | transfer |
| yours | yours, different commodity, both liquid or both not | conversion |
| yours | a system counterparty, or the reverse | adjustment |
| external | external | *rejected: doesn't involve you* |

- Money into an investment or a loan is gone for a long time, so it's spent; money out of
  one is income. Like any expense or income it has a category (*Invest*, *Divest*), and
  reports count it once, on the liquid side.
- An **adjustment** sets or corrects a balance (an opening balance, a reconciliation). It's
  neither income nor expense and takes no purpose or category.
- Changing a split's account or counterparty changes its kind, and nothing else needs
  updating. The API returns the kind read-only, for filters and reports.

What this removes: separate expense and revenue accounts for one payee (a refund is
`Store → Alex Credit`, netting against its category); fake cash sources for investment
income; categories that restate a kind ("Credit card payment"); and payee sprawl, which
becomes a classification problem (§8) instead of a structural one.

### 3.3 Splits handle every case, including investments

A split moves `qty_from` of commodity A out of one side and `qty_to` of commodity B into
the other. The ledger has one invariant:

> If A = B, then `qty_from = qty_to`. If A ≠ B, the split is a **conversion**, and its
> implied rate is `qty_from / qty_to`, derived from the split, not stored as a price.

| Event | Splits | Kind |
|---|---|---|
| Groceries | Alex Credit (USD 80) → Grocer | expense |
| Refund | Store (USD 30) → Alex Credit | income, nets against its category |
| Card payment | Alex Checking (USD 500) → Alex Credit | transfer |
| Invest savings | Joint Savings (USD 5,000) → Joint Brokerage | expense |
| Buy fund | Joint Brokerage (USD 1,000) → Joint Brokerage (VTI 4) | conversion, same account |

One brokerage account holds several securities: **holdings** are an account's balance
grouped by commodity. No lots or cost basis are tracked; gains could be computed from
conversion splits later without a schema change.

### 3.4 Valuation is computed when requested

**Job:** say what an amount in one commodity is worth in another on a date, saying which
price it used, without ever making a request wait for a provider.

- Balances are returned in their native commodities first; a valued figure is asked for
  (`value_in`), and the settlement values every split in USD at its own date (§4.2).
- **One price, chosen by rule:** the most recent on or before the date; a fresh one beats an
  older one; one price (or its inverse) beats a chain of two through a pivot; manual prices
  beat fetched ones, then sources in their configured order. A figure says which price and
  date it used, and whether it's stale, derived or missing. The server never invents
  numbers.
- **Lazy fetching:** a valuation missing a price answers at once with what's stored (marked
  stale or missing, and `fetching`), and starts one background fetch for everything it
  needs. Clients wait on the fetch and ask again; the web app does this by itself. Storing
  a fetched price is the one GET that writes: a historical close is an external fact being
  cached.
- **Outages degrade answers, never fail them.** A source that fails is passed over for a
  while; the stored price, marked stale, stands in. A forced refresh asks anyway.
- **Sources are pluggable** (`PriceSource`, in `external-calls`): which run and in which
  order is configuration. Today Tiingo (securities and FX, with a key) and the ECB's
  reference rates (FX, no key). Only commodity codes and dates are sent.

The exact rules are in `ledger::valuation`; fetching, its windows and configuration in
`api::valuation_prices`, `external-calls` and the server's `config.rs`.

### 3.5 Numbers: no floating point

Moved to §19.1.

### 3.6 PII at runtime: logs and remote AI calls

Moved to §19.2.

## 4. Component: household ownership and monthly settlement

**Job:** turn owner and purpose labels into who owes whom each month, so the household can
settle up and close the month. This is the product; the rest supports it.

### 4.1 Parties, owners and purposes

- **Parties** form a small **settlement tree**: each person settles with the Household, and
  each business settles with its **funder**, the Household or a person, chosen per month.

  ```
  Household ─┬─ Alex
             ├─ Sam
             └─ Studio     (funder = Household; could move under Sam)
  ```
- **Owner (per account, set once).** *Household* owns Joint Savings, Joint Brokerage and
  Mortgage; *Alex* owns Alex Checking and Alex Credit; *Studio* owns Studio Checking.
- **Liquid vs. investment (per account).** Checking, savings, cards and cash are liquid;
  brokerages and mortgages aren't. Activity inside an investment (vests, dividends, buys,
  sells) has no purpose and doesn't enter the settlement; money into or out of one is an
  expense or income (§3.2).
- **Totals (display).** Liquid accounts, investments and loans are totalled apart and mixed
  only in a **Net** line, so a mortgage doesn't hide the rest.
- **Purpose (per split).** The party the money was *for*. It applies to income and expenses
  on liquid accounts; transfers and conversions have neither purpose nor category.
- **No default.** Every income and expense on a liquid account names its purpose and
  category: nothing is saved or committed without both, so nothing is ever
  "uncategorized". Suggestions name both (§8).

### 4.2 What creates a claim between parties

Every value is in USD, the settlement currency, at the transaction's own date (§3.4). A
split no price connects to USD is listed as unpriced and left out.

| Event | Example | Claim |
|---|---|---|
| Income or expense where owner ≠ purpose | Groceries on Alex Credit, purpose Household | Household owes Alex $80 |
| | Salary into Alex Checking, purpose Household | Alex owes Household the salary |
| Money out of an investment | Alex Brokerage → Alex Checking, for the Household | Alex owes Household, until moved on; into a Household account it's simply shared |
| Money into an investment or a loan | Alex Checking → Mortgage *(principal)* | Household owes Alex |
| **Settlement payment**: any other transfer between liquid accounts of different owners | Alex Checking → Joint Savings | Counts as *paid* against the claims between those parties |
| **Stipend**, set per person per month | | Household owes the person their stipend |
| Owner = purpose, transfers within one owner, conversions, investment activity | | none |

**Routing.** A claim between two parties is added to every edge on the tree path between
them: Sam paying a Studio expense crosses Studio → Household → Sam while Studio is
household-funded, and Studio → Sam directly if Sam becomes the funder.

### 4.3 The monthly statement and close

For month M, each tree edge (child *c* ↔ parent) gets:

```
due_c(M)      = Σ claims routed across the edge in M   (> 0: c pays the parent; < 0: parent pays c)
paid_c(M)     = Σ settlement payments across the edge linked to M
carried_c(M)  = outstanding_c at the end of M − 1
outstanding_c = Σ over all months (due − paid) = carried + due − paid
```

- **People:** `due` includes the stipend; after settling, `outstanding` returns to ~0.
- **Businesses:** no stipend, and settling to zero is optional: a business edge's
  `outstanding` is its **net funding**.
- **Household pool:** household income, expenses by category, stipends, net business
  funding, and the change in shared savings.

**Review and close** is a bookkeeping step, not an approval workflow:

1. Import the month's files for every account (§6), reviewing purposes and categories.
2. Set the month's **stipends** and **funders**, carried forward from the previous month.
3. Read the statement and **close** the month.
4. **Pay:** a settlement payment settles the month before the one it's made in; one meant
   for another month names it on its split (`settles`).

### 4.4 Closed months and reopening

*Closed* guards against **accidental** edits; it isn't a lock.

- Edits and commits that touch a closed month are refused, saying so.
- **Reopen** is always available: fix the mistake, close again.
- Nothing is frozen at closing: `due(M)` is computed live, so a fix to a closed month shows
  up as the difference against what was paid, **carried** into every later statement until
  a payment pays it down. Past payments are never rewritten.

### 4.5 Validation against migrated history

Before stipends are entered, the statement computes each person's **implied stipend** per
month (`claims − paid`). With fixed stipends it's constant or step-wise; a month that breaks
the pattern points to a mislabeled transaction, an adjustment, or a past mistake carried
forward (§4.4), told apart by `carried`. This is the settlement component's acceptance test
on migrated history (§11).

## 5. Component: REST API

**Job:** the one way in, for every client (principle 4).

- **Contract-first:** the OpenAPI spec is generated from the server code (each route
  registered with its spec entry), versioned under `/api/v1`, and committed; clients and
  SDKs are generated from it. The resources are what the spec lists.
- **Conventions every resource shares:** cursor pagination; `If-Match`/ETag on edits, so
  two edits can't overwrite each other; idempotency keys on POSTs, so clients can retry;
  errors as RFC 9457 Problem Details naming the rule that was broken.
- Amounts travel as decimal strings (§19.1).

## 6. Component: import pipeline

**Job:** turn a bank file into ledger transactions, without duplicates and with every
transfer recorded once, after a person has reviewed what will happen. How it behaves, rule
by rule, is in [`import.md`](import.md).

```
upload raw file ─▶ detect ─▶ parse ─▶ resolve account ─▶ match (duplicates, transfers)
     ─▶ suggest (rules → local AI → remote AI) ─▶ STAGED import
     ─▶ client review (a choice and labels per row) ─▶ COMMIT (atomic)
```

Staging lives on the server, so a review started in one client can finish in another.
Nothing enters the ledger before commit.

### 6.1 Format detection and parsers

Each format is a parser behind one trait in `bank-import`: CSV (a column-alias table), and
OFX/QFX/QBO via `ofx-rs`, wrapped by `bank_import::ofx_parsing` for upstream bugs until
they're fixed. Parsers emit normalized rows (date, amount, description, native ID) plus the
raw row, kept as provenance. Dates are the server's local dates.

### 6.2 Account resolution

A file's account number is looked up among the accounts' numbers. A statement whose account
isn't known (a CSV, a new number) is staged anyway and waits for one: picked in review, or
suggested (§8). Picking it adds the file's number to the account, so its next files resolve
by themselves.

### 6.3 Deduplication

Re-uploading a file, or overlapping date ranges, adds nothing. A row is matched against the
ledger exactly (the bank's ID or the row's content) and by near match (same amount within
days); near matches are only ever *possible* duplicates, for review. Rules:
[`import.md` §6.3](import.md#63-duplicates).

### 6.4 Transfers

A card payment appears in both files; it's one transfer, never an expense plus an income.
Two staged rows that can be each other's other side are paired into one transfer unless a
choice of yours says otherwise; a row whose transfer is already in the ledger is a duplicate
of it. Rules: [`import.md` §6.4](import.md#64-transfers).

### 6.5 Suggestions

The statement's account when unknown, then each row's counterparty, purpose and category,
each with a confidence, a reason and the suggester that made it (§8).

### 6.6 Staging and review

Each row has a **label** (what matching found) and a **choice** (what committing does).
Three rules decide everything:

1. **Every choice has a default,** with a confidence.
2. **Your choices win:** when what's found disagrees with something you chose, your choice
   stays the default, short of certain.
3. **A row's colour is one function** of its state: not added, needs you, to review, or
   ready. Every client and page shows the same.

Labels, choices, defaults, states and readiness:
[`import.md` §6.6](import.md#66-review).

### 6.7 Commit

All or nothing: added rows become transactions in one import batch, a transfer pair becomes
one transfer, and a duplicate that isn't added records the row with the transaction it
matched. Undoing a batch is one call. Rules: [`import.md` §6.7](import.md#67-commit).

### 6.8 Balance check

An OFX file's ledger balance is recorded on commit as a balance check (§7.1).

## 7. Component: query and reporting engine

**Job:** answer every "how much" question from the ledger, consistently with its rules.

- **One aggregation,** `POST /query`, over postings (a split's signed movement on one of
  your accounts): a flow over a range or a balance at its end, filtered, grouped and
  bucketed. Rows are per commodity; the query doesn't value.
- **Named reports** build on it: the settlement statement (§4.3), the household pool, and
  balances on a date, optionally valued (§3.4). The web app's reports use these.
- Splits are classified by `ledger`, as everywhere else, so a report can't disagree with the
  ledger. Nothing is cached: household-scale data aggregates in milliseconds.

### 7.1 Balance checks and the discrepancy finder

**Job:** tell the user whether the ledger agrees with the bank, and if not, which
transactions likely explain it.

The user enters what the bank shows, optionally dated, as the bank shows it; an OFX file's
ledger balance arrives the same way (§6.8). The server compares it with the ledger balance
on that date.

- **Match:** the account is *verified through* that date; later checks search only after it.
- **Mismatch:** the **discrepancy finder** (`ledger::discrepancy`, no AI) searches that
  window and returns candidates, most likely first, each saying why (recorded twice, not yet
  at the bank, dated across the boundary, a sign flip, a sum, swapped digits). It changes
  nothing.

## 8. Component: classification (rules and AI)

**Job:** propose each imported row's labels, so a person only reviews what the system isn't
sure of. How it operates is in [`ai.md`](ai.md).

- **Targets:** a statement's **account** when the file doesn't settle it, then each row's
  **counterparty, purpose and category**, as one proposal per row, each label with its own
  confidence. A row's **choice** is suggested by matching (§6.6). Tags aren't suggested.
- **Transfers are suggested like anything else:** a row's counterparty label is its *other
  side*, a counterparty or one of your accounts. Pairing two staged rows into one transfer
  is a rule, not a suggestion (§6.4).
- **Three levels, tried in order: rules → local AI → remote AI.** A level runs while a label
  is below the confidence threshold; labels already settled (by you, or confidently by an
  earlier level) are given to it as facts. Below the threshold everywhere, the best
  suggestion is kept, shown as unsure. The stack runs again whenever the evidence changes
  (another file staged or committed, a statement's account set, on request); your labels
  and confident suggestions stay.
- **Every suggestion names the exact system that made it,** with a confidence and a reason,
  and is recorded with what was committed, to measure each suggester and compare models.
- **Review and commit:** a suggestion is the value shown, never an extra option; commit
  takes what's on screen and is refused while something needs you (§6.6).

The levels, each behind one interface (`suggest::Suggester`) so any can be replaced:

1. **Rules:** one [Rhai](https://rhai.rs) script in the DB, sandboxed, with no floats.
   Uploading validates every name it uses; an invalid script never becomes active. Every
   version is kept. It starts empty: rules are written where local AI is measurably wrong.
2. **Local AI:** in-process, pure Rust, learning from the ledger itself; its confidence is
   calibrated on the ledger's own past, so *p* means right about a fraction *p* of the time.
   Methods are interchangeable and measured by a backtest.
3. **Remote AI, optional:** a decision model asked multiple-choice questions whose options
   are the ledger's own names, only about what the levels before it left unsure, and shown
   only what §19.2 allows. The system works fully without it.

## 9. Component: bills and recurrences

Not built: #35 (bills and recurrences), #36 (learned bills).

## 10. Component: authentication

**Job:** let household members and their scripts in, and no one else, with nothing on the
server that would let a thief sign in.

- **Users sign in with passkeys** (WebAuthn). No passwords, no outside identity provider;
  the server keeps only public keys.
- **Invites are the one way in.** An invite names a person and gives a one-time link that
  registers a passkey for them (a new user, or another device for an existing one). The
  first invite is made on the server itself (`exchequer-server invite`): shell access is the
  authority, so there's no bootstrap token over the network.
- **Everything runs on bearer tokens.** Signing in gives the browser a token; the CLI gets
  its own by being approved from a signed-in browser (like OAuth's device flow); scripts
  get named tokens made by a user. Every token belongs to a user, expires, and is stored
  hashed. Only a browser's or the CLI's token can make tokens, so a leaked script token
  can't outlive itself.
- **Sessions** are the clients using tokens. Each shows who, how it signed in, and from what
  (as the client reports it: for people to read, never a check), and can be signed out.
  **The kill switch** signs everyone out and revokes every token.
- **Passkeys are bound to the server's domain** (`EXCHEQUER_ORIGIN`); without it, signing in
  is off.
- **TLS** is the operator's reverse proxy's job.

**One ledger, several people.** Everyone signed in sees and edits everything; the audit log
records which token, and so which user, changed what. One household per server.

## 11. Component: Firefly III migration (one-time)

**Job:** move a household from Firefly III one calendar year at a time, writing only through
Exchequer's REST API and stopping before anything could duplicate or corrupt data. A Python
client (`firefly-migrate`); no migration code lives in the server, and Firefly is only read.
What becomes what, how a year fits the server's data, and the acceptance check (§4.5) are in
[`firefly-migration.md`](firefly-migration.md).

## 12. Component: CLI

**Job:** the terminal's way to do what's done from files: import bank files and edit the
rules script, plus signing in and refreshing prices. Everything else is the web app.

A thin client on the generated Python SDK; it never imports server code, and never prints
its token. Commands: `exchequer --help`.

## 13. Component: web app

**Job:** everything a household does, in a browser. A static SvelteKit build served by the
API container at `/`, the same origin as `/api/v1`; it talks only through the TypeScript
client generated from the spec, and looks and behaves as [`ux.md`](ux.md) says.

| Page | Its job | Built on |
|---|---|---|
| **Accounts** | Balances on a date, valued, by owner and liquid/investment/loan | balances report (§7), valuation (§3.4) |
| **Transactions** | Find and edit transactions and their splits | transactions |
| **Imports** | Upload, review and commit bank files | imports (§6), suggestions (§8) |
| **Checks** | Compare with the bank; find what explains a difference | balance checks (§7.1) |
| **Settlement** | Each month's state across a year; a month's statement, parameters, close and reopen | settlements (§4) |
| **Reports** | Spending, income, balance history, household pool | query and named reports (§7) |
| **Setup** | Parties, accounts, commodities, prices, counterparties, categories, tags; Security: users, passkeys, sessions, tokens, the kill switch | reference data, auth (§10) |

## 14. Component: deployment and operations

**Job:** run one server for one household, with little to look after. Installing and
operating it is in [`deploy/README.md`](../deploy/README.md); releasing in
[`deployment.md`](deployment.md).

- Rootless podman quadlets on a private network: the server (API and web app), PostgreSQL
  (no published port) and a backup job. The API listens on localhost, behind the operator's
  reverse proxy.
- Schema migrations run at startup; `/healthz` backs the container's health check;
  `/version` reports the release and schema version. Logs go to journald.
- The server runs released images and updates through `podman auto-update`; nothing is
  built on it.

### 14.1 Secrets

**No secret appears in a unit file, an env file or a repo.** Secrets are podman secrets,
handed to containers as files. For each secret setting `X` the server reads `X_FILE`, and
refuses to start if a secret arrives as a plain environment variable. Secrets are never
logged or reported. Each keyed outside service has its own secret. The list and the
commands are in `deploy/README.md`.

### 14.2 Backups

A host systemd timer runs a `pg_dump` container to local storage: a scheduled job, but the
host's, not the app's. A missed run fires on wake; each dump names its schema version, so a
restore can check compatibility. Restore is an admin command, never an endpoint, and is
tested in CI. No offsite copy yet: #38.

## 15. Stack

**Rust server, Python CLI and migration tool, TypeScript web app**, meeting at the OpenAPI
spec.

| Crate | Its job | Depends on |
|---|---|---|
| `ledger` | The ledger's rules: commodities, splits, invariants, derived kinds, settlement math, valuation, queries, the discrepancy finder, `AccountNumber`. Pure logic, no I/O | — |
| `store` | Postgres access and migrations | `ledger`, `suggest` |
| `bank-import` | Parsers, matching (duplicates, transfer pairs), staging | `ledger`, `suggest` |
| `suggest` | The suggestion interface every level implements, the stack, the backtest | — |
| `suggest-rules` | The Rhai rules script as a suggester | `suggest` |
| `suggest-local-ai` | Local AI methods, normalization, calibration; the one crate allowed floats | `suggest` |
| `suggest-remote-ai` | The decision model as a suggester: its questions and what it's shown | `suggest`, `suggest-local-ai` (normalization), `external-calls` |
| `external-calls` | Every outbound client (the decision model, price sources): the only place data leaves the server | — |
| `api` | HTTP handlers, auth, price fetching, the OpenAPI spec | all of the above |
| `server` | The binary: configuration, wiring, startup migrations | `api` and the crates it configures |

The CLI is Typer on an SDK generated from the spec, with amounts as `decimal.Decimal`. The
web app is SvelteKit with a client generated from the spec and exact decimal-string math in
`src/lib/money.ts`. Dependencies are in each toolchain's manifest.

## 16. Code organization

### 16.1 Repository

One public monorepo, BSD-2-Clause, with **nothing specific to any household**: no names,
institutions, account numbers, rules, amounts, mappings or bank files. Household data lives
in the database; deployment config and the migration mapping live with the operator.

The spec couples server and clients, so an API change, the regenerated SDKs and the client
updates land in one commit, and CI checks they agree.

### 16.2 Layout

One top-level directory per toolchain, each with its own manifest and lockfile: `server/`
(Cargo workspace), `python/` (uv workspace: SDK, CLI, Firefly migration), `web/` (npm),
plus `api/` (the committed spec), `deploy/` (backup image, examples), `docs/`.

### 16.3 Conventions

- **The spec and both SDKs are generated and committed,** so API changes show as readable
  diffs, and CI fails on drift.
- **Every change reaches `main` through a pull request, and every merge is a release**
  ([`deployment.md`](deployment.md)). **One version for everything,** taken from git tags,
  so nothing is edited to release.
- **Real bank files never enter the repo,** not even anonymized. Test files are synthetic,
  for the fictional household; tests on real files read a local path and run only on the
  operator's machine.
- **Every check runs through the `justfile`,** locally and in CI.

### 16.4 License

**BSD-2-Clause, so no GPL dependencies:** MIT, Apache-2.0, BSD and ISC, with named
exceptions in `server/deny.toml`; CI checks every lockfile's licenses. GPL tools are run as
external programs, never linked.

## 17. Not built

Work not built is tracked as GitHub issues ([`issues.md`](issues.md)); this doc changes
with the pull request that builds it.

## 18. Decisions

| # | Topic | Decision | Where |
|---|---|---|---|
| 1 | Securities | One account per institution holds many commodities; per-security accounts are collapsed on migration | §3.3, §11 |
| 2 | Outbound calls | Only because of a request (prices, remote AI), possibly after it answers; never on a timer | §19.3 |
| 3 | Pending transactions | Not modeled; imports contain finalized transactions only | §6 |
| 4 | Attachments / raw files | Neither is stored; banks can always re-export | §6 |
| 5 | Migration scope | One calendar year at a time; older years optional | §11 |
| 6 | Budgets | Not modeled; owner and purpose replace their common use | §3.1 |
| 7 | Piggy banks | Not modeled | §11 |
| 8 | Transfer-only categories | Not modeled; categories classify income/expense, and transfers carry no purpose | §3.2 |
| 9 | Owner and purpose | First-class, the basis of settlement | §4 |
| 10 | Balance checks | One user-entered number, optionally dated; mismatches go to the discrepancy finder | §7.1 |
| 11 | Stipend | Set per person when closing each month | §4.3 |
| 12 | Businesses | Their own party and settlement; the funder is set per month | §4.1 |
| 13 | Investments | Not shared while invested; money into or out of them (a mortgage too) is an expense or income, with a purpose and a category like any other | §3.2, §4.1, §4.2 |
| 14 | Transfers between people | Settlement payments | §4.2 |
| 15 | Privacy within the household | Everyone sees and edits everything | §10 |
| 16 | Investment proceeds into personal accounts | Income with a purpose like any other: a claim, settled when moved on | §4.2 |
| 17 | Mortgage | Household-owned and never liquid; its principal is spending | §3.2, §4.2 |
| 18 | Stack | Rust server; Python CLI and migration; TypeScript + SvelteKit web app | §15 |
| 19 | Code organization | One public monorepo; household data in the DB; deployment config private | §16 |
| 20 | License | BSD-2-Clause, no GPL dependencies | §16.4 |
| 21 | Import rules | One Rhai script, versioned in the DB, validated before it becomes active | §8 |
| 22 | Repo hygiene | Conventions only: fictional examples, synthetic fixtures, push protection | §16.3 |
| 23 | PII at runtime | `AccountNumber` masking, no bodies in logs; remote AI gets features, not identifiers | §19.2 |
| 24 | Name | Exchequer: originally the treasury of the royal household | — |
| 25 | Price sources | Pluggable and configured; one price before a chain | §3.4 |
| 26 | Releases | Every merged pull request is a release; `main` takes no direct pushes | §16.3 |
| 27 | Design doc references | Section numbers are stable; sections are appended, never renumbered | intro |

## 19. Rules across every piece

### 19.1 Exact decimals

**Rule:** every amount, quantity, price, rate, and anything derived from them, uses exact
decimal arithmetic in every layer. A float touching one is a bug.

| Layer | How it holds |
|---|---|
| Bank file parsing | Text → decimal directly; no float in parsers or fingerprints |
| Postgres | `NUMERIC`; a schema test fails on any `real` or `double precision` column |
| Server (Rust) | `Amount` (`rust_decimal`) and `Rate` (`bigdecimal`) accept no `f32`/`f64`, so mixing is a compile error; `Amount` arithmetic is checked. Float lints and `disallowed_types` are denied workspace-wide; code that needs a float for something else opts out locally, with a reason |
| API | Amounts are JSON strings; a JSON number is rejected |
| CLI | `decimal.Decimal` end to end; `just python-check` rejects floats in client code |
| Web app | Amounts stay strings; the rare client-side math goes through `money.ts`; lint bans `Number()`/`parseFloat` |
| Rules scripts | Rhai built without floats; amounts arrive as `Amount` |
| AI | Amounts go into prompts as strings; no arithmetic is ever taken from a model |

**Rounding** happens only at explicit boundaries: ledger amounts are exact (too many decimals
is rejected, never rounded); a conversion rounds once, at the end, half-even, and a total is
the sum of the rounded values shown; derived rates are rounded once and never rewrite a
stored amount.

**Scope:** money only. Similarity scores, confidences and a model's internals may use
floats, as long as a float never becomes stored data (confidences are stored per mille).
**Imported data may already be damaged** (`1234.5699999999999` from a float-based tool): it's
reported for confirmation, never stored or rounded silently.

### 19.2 PII

The server shows the household's data to the household, and leaks it nowhere else.

- **Logs:** account numbers only as their last four digits (`AccountNumber` masks itself;
  the full number takes an explicit `reveal()`); request and response bodies and raw bank
  rows are never logged.
- **Clients:** the API returns account numbers masked unless a request asks for them
  (`?reveal=numbers`); the web app asks only when the user chose to see them (ux.md §10).
- **Remote AI** gets features, not identifiers: account numbers as their last four, and
  descriptions with long digit runs replaced. A request the service refuses makes remote AI
  step aside for that row; the import never fails because of it.

### 19.3 Outbound calls

The server calls out (price sources, remote AI) only because a request needs it, possibly
finishing after the request answers, and never on a schedule. Every outbound client lives in
`external-calls`, the one place data leaves the server.
