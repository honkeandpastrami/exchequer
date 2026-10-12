# Exchequer — design

This document describes what Exchequer is, how its pieces fit, and the independent
components that can be designed, built and tested on their own. Requirements are in
[`requirements.md`](requirements.md).

Examples use a **fictional household**: *Alex* and *Sam*, a small business *Studio* run by
Sam, and the *Household* itself. Amounts are round and illustrative.

## 1. Guiding principles

1. **The household flow is the reason this app exists.** Every account has an *owner* (a
   person, a business or the household), and every income/expense has a *purpose* (who it
   was for). The monthly settlement between each party and the household is computed from
   those two labels (§4). Where another design choice conflicts with this flow, this flow
   wins.
2. **The server is a pure REST service over a database.** It only acts when a request comes
   in. It runs no cron jobs, timers, background workers or startup tasks beyond schema
   migrations, so it can sleep for days and lose nothing.
3. **The server can reach the internet, but only while handling a request.** It calls
   exchange-rate/quote providers and the LLM provider when a request needs them, never on a
   schedule. Clients do the local work: reading files from disk and talking to humans.
4. **The API contract is the product.** An OpenAPI spec is the boundary. Any client that
   speaks it is first-class (the Transmission model). The CLI and the web app are both
   clients, and neither is privileged.
5. **Store facts, derive everything else.** Balances, holdings, settlements and reports are
   computed when requested from the stored transactions. Editing one transaction changes
   every result that depends on it. Caches are added only if measurements show a need, and
   they must be invalidated deterministically.
6. **Every amount carries a commodity.** Fiat currencies, stocks, funds and crypto are all
   *commodities*. An account holds any number of them. Converting between them is an
   ordinary transaction.
7. **No floating point for money, anywhere.** Every amount, quantity, price and rate is an
   exact decimal, from the bank file all the way to the screen (§3.5).
8. **Import is a server feature.** Parsing, deduplication, account detection and
   categorization happen on the server. The client uploads raw bytes and then reviews the
   server's proposal.

## 2. Topology

```
 ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
 │  CLI / TUI   │   │   Web app    │   │ future       │
 │ import,review│   │ reports,edit │   │ clients      │
 └──────┬───────┘   └──────┬───────┘   └──────┬───────┘
        │   HTTPS + bearer token (OpenAPI contract)
        └──────────────────┼──────────────────┘
                    ┌──────▼───────┐        outbound only on request:
                    │  API server  │ ─────▶ FX / quote provider (prices)
                    │  (stateless) │ ─────▶ LLM provider (classify/match/explain)
                    │              │ ─────▶ OIDC JWKS (if Entra auth is on)
                    └──────┬───────┘
                    ┌──────▼───────┐
                    │  PostgreSQL  │  ◀── pg_dump timer (host systemd)
                    └──────────────┘
   podman (user-level quadlets): api + db on a private network
```

Where outside data comes from:

- **Exchange rates and stock quotes:** the server fetches them itself, **lazily**, the first
  time a valuation needs a price it doesn't have (§3.4). A server that sleeps for a week
  simply catches up on the next report.
- **Bank files:** the user downloads them, and a client uploads them.

## 3. Core domain model

### 3.1 Entities

| Entity | Purpose | Notes |
|---|---|---|
| **Commodity** | USD, EUR, VTI, BTC… | `code`, `kind` (fiat/security/fund/crypto/other), `precision` (2 for USD, 0 for JPY, 3+ for shares). |
| **Party** | Who money belongs to | *Household*, people and businesses, arranged as a settlement tree (§4.1). |
| **Account** | Something of **yours** that holds value | Required **`owner`** (a Party) and a **`liquid`** flag (investment accounts aren't liquid, §4.1). Checking, savings, credit card, brokerage, mortgage, cash. It has **no fixed currency** and holds any commodities. `kind` only affects display: sign convention, icon, credit-card metadata such as the credit limit. Import metadata: institution, account-number aliases, and "pairs" (a card whose payments and purchases arrive under two account numbers). |
| **Counterparty** | Anyone **outside** you | Merchants, employers, utilities, tax agencies. It has **no direction**: the same store is where you spend and who refunds you. Cheap to create (imports create them), mergeable, with *aliases* (raw bank strings → canonical name). Two system counterparties: `Opening balances` and `Reconciliation`. |
| **Transaction** | One real-world event | Date, description, notes, tags, provenance (import batch, raw row, external IDs). Contains one or more splits. |
| **Split** | One movement inside a transaction | A **from-leg** and a **to-leg**, each `(account or counterparty, commodity, quantity)`, plus **`purpose`** (a Party: *who is this money for?*), `category`, `bill`, and an optional own description. Purpose and category apply only to income/expense and liquidations, never to transfers or conversions. |
| **Category / Tag** | Classification | A category is the *class* of an income or expense (Groceries, Salary), never where the money went. Tags are free-form (trips, projects). There are **no budgets**: owner and purpose do the job budgets were often used for. |
| **Settlement** | One month's closing | `(month, status: open/closed, closed_at)`, the month's **stipend per person** and **funder per business** (set by the client before closing), plus links to the transfers that paid it. Amounts are always computed live; nothing is frozen (§4.4). |
| **Price** | `1 BASE = rate QUOTE` on a date | Sources: `provider:<name>`, `manual`, or `implied` (derived from a conversion split). Records `fetched_at`. Covers FX and security quotes alike. |
| **Bill** | A recurring *expectation* | Schedule, amount *phases*, matching hints, and an explicit **lifecycle** (start, optional end). **Finite by default**: status is derived, and "probably ended" is a real state. Entered by hand or learned (§9.1). |
| **Recurrence** | A template that generates transactions | Only *materialized* when a client calls an endpoint, never by a timer. |
| **RuleScript** | The import conditions | One Rhai script stored as **append-only versions** `(version, source, author, created_at, note)`. The newest valid version is active (§8). |
| **BalanceCheck** | "Account X held N of commodity C at date D" | Entered by the user from the bank's app (optionally dated), or taken from OFX `LEDGERBAL`. Mismatches go to the discrepancy finder (§7.1). |
| **ImportBatch / StagedRow** | The import workflow state | §6. |
| **AuditLog** | Append-only change history | Records who changed what. |

### 3.2 There are no transaction types

Many tools make the user choose withdrawal, deposit or transfer, with a different account
type for each. Here the kind is **never stored and never entered**. It is computed from who
is on each side of a split:

| From | To | Derived kind |
|---|---|---|
| yours | external | expense |
| external | yours | income |
| yours | yours, same commodity | transfer |
| yours | yours, different commodity | conversion |
| external | external | *rejected: doesn't involve you* |

The API returns `kind` as a read-only computed field for filters and reports (income vs.
expense excludes transfers and conversions by default). Changing a split's account or
counterparty changes its kind, and nothing else needs updating.

Workarounds this removes:

- **Separate expense and revenue accounts for the same payee.** A refund is simply
  `Store → Alex Credit` with category *Shopping*, and it nets against Shopping spending.
- **Fake "cash" sources for investment income.** A dividend is `Brokerage (counterparty) →
  Alex Brokerage`.
- **Categories that only restate the kind** ("Credit card payment", "Savings transfer").
  Transfers have no category and no purpose at all, so they can never be mislabeled as
  household spending.
- **Payee sprawl.** Counterparties are lightweight records with aliases and merge support,
  so naming is a classification problem (§8), not a structural one.

### 3.3 Splits handle every case, including investments

A split moves `qty_from` of commodity A out of one side and `qty_to` of commodity B into the
other. The ledger has one invariant:

> If A = B, then `qty_from = qty_to`. If A ≠ B, the split is a **conversion**, and its
> implied rate `qty_from / qty_to` is recorded as an `implied` price.

| Event | Splits | Kind |
|---|---|---|
| Groceries | Alex Credit (USD 80) → Grocer | expense |
| Warehouse store, split | Alex Credit → Store 120 *(Groceries)*; Alex Credit → Store 45 *(Home supplies)* | expense ×2 |
| Refund | Store (USD 30) → Alex Credit | income, nets against its category |
| Card payment | Alex Checking (USD 500) → Alex Credit | transfer |
| Mortgage payment | Joint Savings → Mortgage *(principal)*; Joint Savings → Lender *(interest)* | transfer + expense |
| Stock vest | Employer (ACME 10) → Alex Brokerage | income, valued in USD at the vest-date price |
| Sell stock | Alex Brokerage (ACME 10) → Alex Brokerage (USD 1,500) | conversion, same account |
| Buy fund | Joint Brokerage (USD 1,000) → Joint Brokerage (VTI 4) | conversion |
| Dividend | Fund manager (USD 25) → Joint Brokerage | income |
| FX exchange | Alex Checking (USD 1,100) → Alex EUR Account (EUR 1,000) | conversion |

Converting between commodities inside one account is allowed, so one brokerage account holds
several securities. **Holdings** are the query "balance of account X grouped by commodity."
No lots or cost basis are tracked. If gains reporting is ever needed, it can be computed
from conversion splits after the fact, without a schema change.

### 3.4 Valuation is computed when requested

- Balances are stored and returned **in native commodities** first.
- Any endpoint returning amounts accepts `?value_in=USD&price_date=…`. The server converts
  using the most recent price on or before the date, via direct or inverse rates and, if
  needed, one hop through a pivot currency.
- Converted figures always state **which price and which date** were used, and flag missing
  or stale prices. The server never silently invents numbers.

**Price fetching** happens inside the request:

1. A valuation needs `ACME→USD` on date D. If the price exists, it's used.
2. If not, the server fetches the missing prices **in one batch** (every commodity × date the
   request needs, back-filling gaps since the last stored price), stores them, then answers.
   This is the one deliberate exception to "a GET never writes": a historical closing price
   never changes, so storing it is caching an external fact.
3. A price for a day still in progress is *provisional*: refetched when older than a
   configurable age and replaced by the close once available.
4. If the provider is unreachable or rate-limited, the request still succeeds with the
   latest stored price and marks the figure stale. Outages degrade answers, never fail them.
5. `POST /prices/refresh` forces a fetch.

The provider is pluggable and should cover FX plus US equities and funds. Its API key is a
podman secret (§14.1). Manual prices always win over fetched ones.

### 3.5 Numbers: no floating point

**Rule:** every monetary amount, commodity quantity, price, exchange rate, and any value
derived from them uses exact decimal arithmetic in every layer: parsing, storage, the API,
computation, AI prompts and clients. A float touching one of these values is a bug.

| Layer | Representation | Enforcement |
|---|---|---|
| Bank file parsing | String → decimal directly. `ofx-rs` yields `rust_decimal::Decimal`; the CSV path parses strings straight into `Decimal`. | No float conversion anywhere in parsers or fingerprinting. |
| Postgres | `NUMERIC`. Quantities are checked against the commodity's precision. Prices and rates use `NUMERIC` with a fixed high scale (e.g. 20). | A schema test fails if any column is `real` or `double precision`. |
| Server (Rust) | Newtypes: **`Amount`** (wraps `rust_decimal::Decimal`, 28 significant digits) for amounts and quantities; **`Rate`** (wraps `bigdecimal::BigDecimal`, arbitrary precision) for prices and conversion math. Neither has any operation accepting `f32`/`f64`, so mixing is a **compile error**. `Amount` arithmetic is checked: overflow or inexact rescaling is an error, never a silent rounding. | `clippy::float_arithmetic`, `clippy::float_cmp` and `disallowed_types = [f32, f64]` are denied in every crate except `heuristics`. |
| API (JSON) | Amounts are **strings** (`"1500.00"`); the OpenAPI schema declares `type: string` with a decimal pattern. | `Amount`'s deserializer accepts only JSON strings; a JSON number is rejected with 422. |
| CLI (Python) | `decimal.Decimal` end to end; the generated SDK maps amount strings to `Decimal`. | Ruff + a CI check ban `float(` and `: float` in client code. |
| Web app | Amounts stay strings; the rare client-side math uses a decimal library. The server returns computed totals. | Lint rule against `Number()`/`parseFloat` on amount fields. Charts map values to pixels internally; labels always come from the strings. |
| AI | Amounts go into prompts as strings. **No arithmetic is ever taken from an LLM**: totals, differences and subset sums are computed in `Decimal`; the model only ranks and explains. | Structured outputs reference transaction IDs, never amounts the code would trust. |

**Rounding** happens only at explicit boundaries:

- **Ledger amounts are exact.** An amount with more decimals than its commodity's precision
  is rejected, never rounded silently.
- **Conversions** compute at full precision and round **once**, at the end, to the target
  commodity's precision, using one documented mode (`ROUND_HALF_EVEN`). Reports sum unrounded
  converted values and round the total, so a column never disagrees with its total.
- **Implied prices** are divisions, stored at the fixed rate scale and marked as derived.
  They are used for valuation only, never to rewrite a stored amount.

**Exempt:** heuristic *scores* that never become money: text similarity in dedup, classifier
confidence, and the internal math of the learned model. They live in the `heuristics` crate,
the only place float lints are allowed. Amount *features* for the model are derived from
`Decimal` in one audited function.

**Imported data may already be damaged.** Data exported from float-based tools can carry
values like `1234.5699999999999`. Importers and the Firefly migration detect any amount with
more decimals than its commodity allows and report it for confirmation instead of storing
it or rounding it silently.

**Test corpus:** values that break floats (`0.1 + 0.2`, zero-decimal currencies,
three-decimal share quantities, large balances, float-damaged inputs) plus `proptest`
properties asserting that split sums, balances and converted totals are exactly
reproducible.

### 3.6 PII at runtime: logs and LLM calls

The server stores the household's data and shows it to the household. It must not leak that
data through side channels:

- **Logs.** Account numbers are only ever shown as their last four digits, and tests assert
  on masked values. `AccountNumber` in `domain` formats itself as `…1234` in `Display` and
  `Debug`; getting the full number takes an explicit `.reveal()`, used only by parsers and
  the DB layer. Request and response bodies and raw bank rows are never logged.
- **LLM calls.** All LLM traffic goes through `outbound` (§15). What each prompt may contain
  is designed together with classification (§8). *Open: to be designed.*

## 4. Component: household ownership and monthly settlement

### 4.1 Parties, owners and purposes

- **Parties** form a small **settlement tree**: each person settles with the Household, and
  each business settles with its **funder**, which is the Household or a person, chosen per
  month so it can change over time.

  ```
  Household ─┬─ Alex
             ├─ Sam
             └─ Studio     (funder = Household; could move under Sam)
  ```
- **Owner (per account, set once).** Example: *Household* owns Joint Savings, Joint
  Brokerage and Mortgage; *Alex* owns Alex Checking, Alex Credit, Alex Brokerage and Alex EUR
  Account; *Sam* owns Sam Checking and Sam Credit; *Studio* owns Studio Checking and Studio
  Credit.
- **Liquid vs. investment (per account).** Checking, savings, credit cards and cash are
  *liquid*; brokerages are *investment* accounts. Investments are never shared while they
  stay invested: activity inside them (vests, dividends, buys, sells, fees) has no purpose
  and doesn't enter the settlement.
- **Purpose (per split, the manual part).** The party the money was *for*. It applies only to
  income and expenses on liquid accounts, and to *liquidations* (below). Transfers and
  conversions have no purpose and no category, because they don't change how much anyone
  has.
- **Defaults:** a split's purpose defaults to its account's owner. Household accounts need
  almost no review. On personal accounts, where a large share of spending is often
  household spending, the purpose is **predicted** (§8).

### 4.2 What creates a claim between parties

Every value is in USD, the settlement currency, **at the transaction's own date** (§3.4),
computed in `Decimal` (§3.5).

| Event | Example | Claim |
|---|---|---|
| Income/expense where owner ≠ purpose | Groceries on Alex Credit, purpose Household | Household owes Alex $80 |
| | Salary into Alex Checking, purpose Household | Alex owes Household the salary |
| | Studio supplies on Sam Credit, purpose Studio | Studio owes Sam |
| **Liquidation**: transfer from an investment account into a liquid one | Alex Brokerage → Joint Savings, $10,000 | Treated like income with a purpose, **default Household**. Landing in a Household account, it simply *becomes shared*: no claim. Landing in Alex Checking with purpose Household, Alex owes it to the Household. |
| **Settlement payment**: any other transfer between liquid accounts of different owners | Alex Checking → Joint Savings; Joint Savings → Sam Checking; Alex → Sam directly; Joint Savings → Studio Checking | Counts as *paid* against the claims between those parties |
| **Stipend** | set per person each month (§4.3) | Household owes the person their stipend |
| Owner = purpose, transfers within one owner, conversions, investment activity | — | none |

**Routing.** A claim between two parties is added to every edge on the tree path between
them. Sam paying a Studio expense is routed Studio → Household → Sam while Studio is
household-funded, and directly Studio → Sam if Sam becomes the funder. Moving a business
only changes which edges its claims cross.

**Mortgage.** A Household-owned mortgage paid from Joint Savings is Household → Household
and never enters the settlement. A payment from a personal account counts as that person
paying for the Household.

### 4.3 The monthly statement and close

For month M, each tree edge (child *c* ↔ parent) gets:

```
due_c(M)      = Σ claims routed across the edge in M   (> 0: c pays the parent; < 0: parent pays c)
paid_c(M)     = Σ settlement payments across the edge linked to M
carried_c(M)  = outstanding_c at the end of M − 1
outstanding_c = Σ over all months (due − paid) = carried + due − paid
```

- **People:** `due` includes the stipend. After settling, `outstanding` returns to ~0.
- **Businesses:** the same math without a stipend, and settling to zero is optional. A
  business edge's `outstanding` is its **net funding** ("the Household has put $X into
  Studio").
- **Refunds** net automatically: a refund is income with the original expense's purpose.
- **Investment proceeds in transit.** A liquidation landing in a *personal* account (purpose
  Household by default) is shown as its own line, "investment proceeds in transit: $X", not
  mixed into the regular due. It clears when (a) the money moves on to a Household account
  (matched by amount, not counted as a regular settlement payment), (b) a large household
  expense offsets it, or (c) the user marks the liquidation *personal* in review. An
  uncleared line carries forward as a reminder.
- **Household pool view:** household income, household expenses by category, stipends, net
  business funding, and the resulting change in shared savings.

**Review and close.** A bookkeeping step, not an approval workflow: no sign-offs; whoever
does it closes the month.

1. Import the month's files for every account (§6).
2. **Purpose review queue:** income/expense splits on person- and business-owned liquid
   accounts, liquidations, and anything with low confidence. Rows come pre-filled by the
   classifier (§8) with confidence and reason. Keys: `h` household, `p` account owner, `b`
   business, `enter` accept, plus *accept all above a threshold*. Rows are grouped by
   counterparty, so "all 40 grocery charges" is one decision.
3. **Set the month's parameters:** each person's **stipend** and each business's **funder**,
   pre-filled from the previous month. Closing requires them (a stipend of 0 is fine).
4. **Close:** the server shows the statement and the transfers to make, then marks M closed.
5. When those transfers arrive in a later import, they are matched and linked to M (§6).

### 4.4 Closed months and reopening

*Closed* only protects against **accidental** edits. It's a guard rail, not a lock.

- Edits to transactions dated in a closed month are rejected with a clear error ("2026-06 is
  closed; reopen it to edit"). Import rows dated in a closed month are flagged in staging,
  with the option to reopen.
- **Reopen** is one action, always available. Fix the mistake (a label, the stipend, the
  funder), then close again.
- Nothing is frozen at closing: `due(M)` is always computed live. After reopen/fix/re-close,
  the statement shows the new `due(M)` next to what was already paid; **the difference is the
  correction**, settled with one extra transfer or carried into the next statement as a line.
- **Carrying forward is how past settlements are corrected.** A month's payments are never
  rewritten: a fix to a closed month changes its `due`, and the difference shows up as
  `carried` in every later statement until some payment, at any time, pays it down. A
  business edge may carry a balance indefinitely (its net funding).

### 4.5 Validation against migrated history

Migrated history (§11) brings existing purpose labels and the real settlement transfers.
Before any stipends are entered, the statement computes the **implied stipend** per person
per month (`claims − paid`). If stipends were fixed amounts, the implied values are constant
or step-wise. A month that breaks the pattern points to a mislabeled transaction, a manual
adjustment, or a past mistake: one that the old system made and that is now carried forward
(§4.4), or a later payment that pays such a mistake down. The check tells these apart by
`carried`. This is the acceptance test for the settlement component.

## 5. Component: REST API

- Contract-first: the OpenAPI 3.1 spec is generated from the server code, versioned under
  `/api/v1`, and committed so clients can generate typed SDKs.
- **Resources:** parties, settlements (`GET /settlements/{month}/statement`,
  `POST …/close`, `POST …/reopen`), commodities, accounts, counterparties, transactions
  (splits nested), categories, tags, bills, recurrences, prices, balance checks, import
  batches, rules, tokens.
- **Queries:** a separate `/query` and `/reports` family (§7).
- **Conventions:** cursor pagination; `If-Match`/ETag on edits; idempotency keys on POSTs so
  clients can retry safely; Problem Details (RFC 9457) errors that name the violated
  invariant.
- Amounts travel as **decimal strings**, never JSON numbers (§3.5).

## 6. Component: import pipeline

```
upload raw file ─▶ detect ─▶ parse ─▶ resolve account ─▶ fingerprint/dedup
     ─▶ transfer matching ─▶ classify (conditions → learned → LLM) ─▶ STAGED batch
     ─▶ client review (accept / edit / reject per row) ─▶ COMMIT (atomic)
```

1. **Format detection and parsers.** Each format is a plugin behind one Rust trait: CSV with
   a column-alias table and per-bank profiles, OFX/QFX/QBO via `ofx-rs` (OFX 1.x SGML and 2.x
   XML), and content sniffing for OFX content under unexpected extensions. `ofx-rs` 0.2.0 is
   wrapped by a hot-patch (`import::ofx`) for three bugs: bank-specific
   tags inside a transaction (kept as *extensions*, e.g. a merchant category code, useful
   for classification), dropped entities like `&amp;`, and mangled non-ASCII text in
   OFX 1.x. The patch is removed once upstream fixes them. Parsers emit normalized rows (date, amount,
   description, native ID) plus the raw row for provenance.
2. **Account resolution.** OFX account number → configured aliases and pairs → LLM
   suggestion → ask the client.
3. **Deduplication,** done once, on the server, with full DB access:
   - *Exact:* native ID (FITID or bank reference), or a content hash with an occurrence
     suffix for identical rows in one file; unique per account.
   - *Near:* same account and amount, date within ±N days, description similarity via
     `pg_trgm`, one-to-one matching. Near-duplicates are **flagged in staging**, not
     prompted mid-run.
   - Re-uploading a file, or overlapping date ranges, is a no-op.
4. **Transfer matching.** A card payment appears in both the checking and the card file.
   Incoming rows are matched to existing opposite-signed splits on your other accounts, so a
   transfer isn't recorded as an expense plus an income. Transfers between a person and the
   Household are also matched against open settlements (§4.3).
5. **Classification** (§8): proposed purpose, counterparty, category, tags and bill, each
   with a confidence and a reason.
6. **Staging and review.** The batch lives on the server, so a review started in one client
   can finish in another. Rows are `new`, `duplicate`, `near-duplicate`, `transfer-match` or
   `needs-account`.
7. **Commit** is all or nothing. Each transaction keeps its `import_batch_id`, so undoing a
   batch is one call. Uploaded files aren't kept; only each row's parsed raw form is.
8. **Balance check.** An OFX `LEDGERBAL` is recorded as a balance check, and a mismatch is
   shown after commit (§7.1).

## 7. Component: query and reporting engine

- **One general aggregation endpoint:** filter (dates, accounts, owner, purpose, categories,
  tags, bills, text) × group by (owner, purpose, account, category, tag, commodity, payee)
  × bucket (day … year) × optional `value_in`.
- **Named reports on top:** the monthly settlement statement and household pool (§4.3),
  personal spending vs. stipend, net worth over time, income vs. expense, spending by
  category/tag, balance history, holdings (quantity plus optional market value), bill status.
- **Implementation:** plain SQL over postings with window functions for running balances.
  Household-scale data aggregates in milliseconds; materialized summaries only if profiling
  demands them.

### 7.1 Balance checks and the discrepancy finder

The user enters what the bank's app shows: one number, optionally with a date
(`exchequer balance check "Alex Checking" 1234.56 --date 2026-10-11`). The server compares it
with the ledger balance on that date.

- **Match:** the account is *verified through D*. Later checks only search the window after
  the last verified date.
- **Mismatch:** the **discrepancy finder** runs on that window:
  1. **Deterministic candidates (no AI):** a transaction equal to the difference (missing or
     duplicated); a sign flip (difference = 2 × amount); a small subset summing to the
     difference; a near-duplicate pair; a transposition typo (difference divisible by 9);
     transactions just before or after D; date ranges no import covered.
  2. **AI explanation (optional):** the window, the difference and the ranked candidates go
     to the LLM, which returns a short ranked explanation. The AI explains but never edits.

## 8. Component: classification (import conditions + AI)

One interface, `suggest(row, context) → proposals with confidence + reason`, with three
providers tried in order. **Purpose is the most important target**, and migrated history
provides labeled training data from day one.

1. **Import conditions as code, not a rules engine.** One small **[Rhai](https://rhai.rs)**
   script, stored in the DB (`RuleScript`). Rhai is a scripting language for embedding in
   Rust, with Rust-like syntax. It's sandboxed (no file system, network or process access)
   and limited in operations per call. The active version is compiled once and runs for each
   staged row before the other providers. If/else is the rule language.

   ```rust
   fn classify(row, s) {
       let d = row.description.to_upper();
       if d.starts_with("ACME CORP PAYROLL") && row.amount > amount("0") {
           s.counterparty("ACME Corp"); s.category("Salary"); s.purpose(HOUSEHOLD);
       } else if d.contains("CITY WATER") {
           s.counterparty("City Water"); s.category("Utilities"); s.bill("Water"); s.purpose(HOUSEHOLD);
       }
   }
   ```

   Rhai is built with `no_float`, and amounts reach scripts as the server's `Amount` type
   (compared via `amount("…")` literals), so §3.5 holds inside scripts. Suggestions are
   labeled `rule:v<version>:<line>`.

   **Editing and versioning:**
   - `PUT /rules` uploads a new version. The server compiles it and checks every referenced
     name (categories, bills, accounts, parties) against the DB. **An invalid script is
     rejected with line-numbered errors and never becomes active.** A valid one is active
     immediately, without a restart.
   - `POST /rules/dry-run?from=&to=` reports what a candidate script *would* change.
   - Every version is kept: list, diff, and rollback (re-uploading an old version as a new
     one). Each version records its author and an optional note.
   - CLI: `exchequer rules edit | log | diff | rollback | dry-run`.
   - `POST /reclassify?from=&to=` re-runs all classifiers over existing transactions and
     returns a *staged* batch of proposed changes, never silent edits.
2. **Learned from history.** A local model trained on past labels (purpose, category,
   counterparty) over normalized description, counterparty, account owner and amount
   features. Hand-written in Rust (token-based naive Bayes / nearest neighbor) in the
   `heuristics` crate. Retrained on request (e.g. after a commit), never on a schedule.
3. **LLM fallback** for rows the first two can't classify confidently, and for account
   detection. Pluggable provider over plain HTTPS + JSON, structured output validated
   against known IDs. Optional: the system works fully without it.

Suggestions are never auto-committed; they pre-fill staged rows for review.

## 9. Component: bills and recurrences

- **Bills** are expectations. Imports link matching splits to them; status is derived when
  read: *upcoming / paid / late / probably ended / ended*. A bill is never assumed infinite.
  - **Schedule:** an interval (`n × day/week/month/year`, e.g. every 2 months, every 4 weeks)
    or an **explicit date list** (e.g. quarterly estimated taxes on irregular dates, as one
    bill).
  - **Amount phases:** a price change starts a new phase instead of widening one range.
  - **Lifecycle:** `start_date`, optional `end_date`, and a derived *probably ended*.
- **Recurrences** (e.g. cash payments no bank file will ever show) produce transactions only
  when a client calls `POST /recurrences/materialize?until=…`.

### 9.1 Learned bills (low priority)

Deterministic statistics over the transaction history, no AI:

- **Series detection:** group splits by counterparty (and account/purpose), find regular
  gaps and stable amounts; split multiple series per counterparty by amount cluster and day
  of month.
- **Schedule inference:** median gap and spread pick the interval, with tolerance for
  weekends and posting lag; fixed calendar dates become a date list.
- **Amount inference:** fixed, a range (utilities), or a step change (new phase), in
  `Decimal`.
- **Lifecycle inference:** start = first occurrence. When expected payments stop (e.g. two
  missed periods), the bill becomes *probably ended*, and the user confirms to set
  `end_date`. Payments resuming after an end start a new phase.
- **When:** after each import commit, for the counterparties it touched, plus
  `POST /bills/detect`. No timer.
- **Output is proposals** in a review queue ("Track as monthly bill?", "Mark ended?", "New
  phase?"). Rejections are remembered.
- Migrated bills are kept as-is; the detector's disagreements with them are reported for the
  user to resolve. Yearly bills need more than one year of history to be detected.

## 10. Component: authentication

A pluggable middleware layer: every request resolves to a principal or is rejected.

- **Tier 1 (first): static bearer tokens,** created by a CLI admin command and stored
  **hashed** (SHA-256: the secrets are 256 random bits, so a slow password hash adds cost without adding safety). One token per client, individually revocable. A bootstrap token
  comes from a podman secret (§14.1).
- **Tier 2 (preferred): OIDC with Microsoft Entra ID** (personal Microsoft accounts). The CLI
  uses the device-code flow; the web app uses auth code + PKCE. The server validates JWTs
  against Microsoft's published keys (outbound HTTPS, cached) and allows an allowlist of
  household members' subject IDs.
- **Not Sign in with Apple:** it needs a paid developer account and HTTPS redirect URIs on a
  public domain, and has no device-code flow, which is a poor fit for a VPN-only service.
  **Passkeys** are a good later option for the web app.
- **TLS** is terminated by the operator's existing reverse proxy.

**One ledger, several people.** Each household member has their own login; everyone sees and
edits everything; the audit log records who changed what. No multi-tenancy: one household
per server.

## 11. Component: Firefly III migration (one-time)

A tool for households moving from Firefly III. It is **re-runnable against an empty DB**
until the output is right, and migrates **one calendar year at a time**.

- **Source and target:** a Python tool that reads Firefly's REST API and writes **only
  through Exchequer's REST API**. No migration code lives in the server.
- **Mapping file** (TOML, hand-edited, kept by the operator, never committed):
  - **Per-security accounts collapse.** Firefly forces one account per security (a custom
    currency each). They become one brokerage account per institution holding several
    commodities, and Firefly transfers carrying a foreign amount become conversion splits.
  - **Currencies → commodities,** including custom currencies used for securities.
  - **Owned accounts** keep their identity. Firefly reports credit-card balances *including*
    the configured "virtual balance" (credit limit), so the migration subtracts it and keeps
    the limit as metadata. Inactive accounts migrate as archived.
  - **Expense and revenue accounts → counterparties,** merged by normalized name. Firefly's
    initial-balance and reconciliation accounts map to the system counterparties.
  - **Transaction types are dropped;** the kind is derived (§3.2). Withdrawals to a
    liability become transfers, as they should have been.
  - **Owner and purpose.** Each account gets an owner. Each split's purpose is derived from
    whatever the household used to encode it (typically a tag such as "Shared" and/or
    per-person budgets), with the mapping declaring which tag or budget means which party.
    Conflicting labels are listed in the report. Budgets and the encoding tags are then
    retired.
  - **Settlement history:** transfers between personal accounts and shared savings become
    settlement payments linked to their month; transfers out of investment accounts become
    liquidations; transfers to a business are funding.
  - **Categories, tags and bills** carry over. **Not migrated:** budgets, piggy banks, and
    categories that only restate a transfer. Firefly rules are translated once into a Rhai
    script and uploaded as rule version 1.
  - **Float-damaged amounts** are rounded to the commodity's precision and **listed in the
    report** for confirmation, never fixed silently (§3.5).
  - **Split transactions** keep their split structure.
- **Year scope:** for year Y, one **opening-balance** transaction per account and commodity on
  Jan 1 (from Firefly's balance that day), then every transaction in Y, keeping Firefly IDs
  as external references. Migrating an earlier year later replaces those opening balances,
  giving the same result as migrating both years at once.
- **Acceptance check:** a comparison report of per-account balances at several dates,
  category totals per month, and recomputed settlements vs. transfers actually made (§4.5).
  Done when it shows zero differences, or every difference is explained.

## 12. Component: CLI / TUI client

A thin client built on the generated SDK; it never imports server code.

- `import <files…>`: upload, then a TUI review screen for the staged batch, then commit.
- `month review <M>`, `month close <M>` (asks for stipends and funders, shows the statement,
  prints the transfers to make), `month reopen <M>`, `settle status`.
- `rules edit | log | diff | rollback | dry-run` (§8), `bills review` (§9.1).
- `balance check`, `prices refresh`, `recur materialize`, `report <name>`.
- `token create | revoke`, `login` (device code, tier 2).

## 13. Component: web app

- A static build served by the API container at `/` (same origin as `/api/v1`, no CORS).
- First the **monthly review and settlement screens** (§4.3), then report dashboards (§7)
  and transaction search/editing with splits, then import review.
- Talks only through the generated TypeScript SDK.

## 14. Component: deployment and operations

- Two quadlets, `exchequer-api.container` and `exchequer-db.container`, on a private podman
  network. The API publishes only to localhost or a reverse proxy. Examples live in
  `deploy/examples/`; each operator keeps their real units privately.
- Schema migrations run once at startup. `/healthz` and `/version` endpoints. Logs go to
  stdout/journald.

### 14.1 Secrets

**No secret ever appears in a unit file, an env file or a repo.** All secrets are **podman
secrets**, generated on the host once and handed to containers as files under
`/run/secrets/`.

| Secret | Consumers |
|---|---|
| `exchequer_db_password` | db (sets it), api (connects), backup (dumps) |
| `exchequer_bootstrap_token` | api (first admin token, §10) |
| `exchequer_llm_api_key` | api (optional, §8) |
| `exchequer_price_api_key` | api (§3.4) |
| `exchequer_azure_backup_sas` | backup unit, offsite step (optional, §14.2) |

```bash
openssl rand -base64 32 | tr -d '\n' | podman secret create exchequer_db_password -
openssl rand -base64 32 | tr -d '\n' | podman secret create exchequer_bootstrap_token -
podman secret create exchequer_llm_api_key -     # pasted once, reads stdin
podman secret create exchequer_price_api_key -
```

```ini
# exchequer-db.container
[Container]
Image=docker.io/library/postgres:17
Secret=exchequer_db_password
Environment=POSTGRES_USER=exchequer POSTGRES_DB=exchequer
Environment=POSTGRES_PASSWORD_FILE=/run/secrets/exchequer_db_password
Network=exchequer.network
# no PublishPort: reachable only on the private network

# exchequer-api.container
[Container]
Secret=exchequer_db_password
Secret=exchequer_bootstrap_token
Environment=DB_HOST=exchequer-db DB_USER=exchequer DB_NAME=exchequer
Environment=DB_PASSWORD_FILE=/run/secrets/exchequer_db_password
```

Server requirements that follow:

- For each secret setting `X`, the server reads `X_FILE`, and **refuses to start** if a secret
  arrives as a plain `X` environment variable.
- Secrets are never logged or echoed by `/healthz` or `/version`, including in DSNs.
- Rotation: `podman secret rm` + `create`, `ALTER ROLE … PASSWORD`, restart.

### 14.2 Backups

**Local (required).** A host systemd timer runs a oneshot `pg_dump` container to a local
backup drive. It's a scheduled job, but it belongs to host operations, not to the app.

- `Persistent=true`: a run missed while the machine slept fires on wake.
- `pg_dump -Fc`; the filename includes the timestamp and the `sqlx` migration version, so a
  restore can check compatibility. Keep the last N dumps.
- Restore is a documented admin command, never an API endpoint, and is tested in CI by
  restoring into a scratch database.

**Offsite to Azure cold storage (nice-to-have).** A second step of the same unit:

- **Encrypted with `age` to the owner's public key** before upload. The host holds only the
  recipient key, so neither the host nor Azure can decrypt. The private key stays with the
  owner (e.g. on a hardware key).
- **Write-only SAS token** (create/write, no read/list/delete), stored as a podman secret.
- **Immutability policy** (time-based WORM) against deletion and ransomware.
- **Archive tier:** very cheap; rehydration takes hours and there's a 180-day minimum, which
  is fine for a last-resort copy.
- Uploads less often than local dumps (e.g. weekly, or only when the dump changed). The
  restore runbook is written down and rehearsed once.

## 15. Stack

**Rust server, Python CLI and migration tool, TypeScript web app.** The OpenAPI spec is the
boundary, so each side uses the language that suits it.

**Server (Rust):** a Cargo workspace.

| Crate | Contents | Key dependencies |
|---|---|---|
| `domain` | Commodities, accounts, splits, invariants, derived kinds, settlement math (§3, §4), `AccountNumber` (§3.6). Pure logic: no I/O, no async | `rust_decimal`, `bigdecimal`, `time` |
| `store` | Postgres access, compile-time-checked queries, migrations | `sqlx` |
| `import` | Parsers, dedup, transfer matching, staging (§6) | `ofx-rs`, `csv`, `strsim` |
| `heuristics` | Learned classifier, similarity scores; the only crate where float lints are allowed | — |
| `rules` | Embedded Rhai engine: compile, validate, run the stored script (§8) | `rhai` (`no_float`, `sync`) |
| `api` | HTTP handlers, auth middleware, OpenAPI generation | `axum`, `tokio`, `utoipa`, `utoipa-axum`, `serde`, `tracing` |
| `outbound` | Price and LLM clients; the only place data leaves the server | `reqwest` |
| `server` | The binary: configuration, wiring, startup migrations | — |

- **OpenAPI:** `utoipa-axum` registers each route together with its spec entry. The spec is
  committed, and CI fails on drift.
- **Auth:** SHA-256 token hashes (`sha2`); `jsonwebtoken` for Entra JWTs.
- **Database:** PostgreSQL 17 (`NUMERIC`, `pg_trgm`); `sqlx` migrations compiled into the
  binary.
- **Container:** multi-stage build, one static binary in a minimal image.
- **Tests:** `proptest` for invariants and settlement math, `sqlx` test databases, synthetic
  bank-file fixtures, API contract tests against the committed spec.

**CLI / TUI (Python 3.13):** Typer and Textual, on an SDK generated from the spec
(`openapi-python-client`), amounts as `decimal.Decimal`. Distributed through Homebrew.

**Firefly migration (Python):** reads Firefly's API, writes only through Exchequer's API
(§11).

**Web app (TypeScript):** SvelteKit with the static adapter, a client generated from the spec
(`openapi-typescript`), and a decimal library for the rare client-side math.

## 16. Code organization

### 16.1 Repository

One public monorepo, BSD-2-Clause. It contains **nothing specific to any household**: no
names, institutions, account numbers, merchant rules, amounts, mappings or real bank files.
Household data lives in the DB; deployment config (quadlets, install script) and the
one-time migration mapping live in the operator's private configuration.

The OpenAPI spec couples the server and the clients, so an API change, the regenerated SDKs
and the client updates land in one commit, and CI checks they agree. Each toolchain gets one
top-level directory with its own manifest and lockfile; dev tools live in the workspace of
their language.

### 16.2 Layout

```
exchequer/
├── README.md
├── LICENSE                        # BSD-2-Clause
├── docs/
│   ├── design.md                  # this document
│   └── requirements.md
├── api/
│   └── openapi.json               # committed; generated from the server
├── server/                        # Cargo workspace
│   ├── Cargo.toml                 # [workspace] + shared lints (float bans)
│   ├── rust-toolchain.toml
│   ├── clippy.toml                # disallowed-types = f32, f64
│   ├── crates/
│   │   ├── domain/  store/  import/  heuristics/  rules/  outbound/  api/  server/
│   │   └── ofx-check/             # dev tool, not shipped (build step 0)
│   ├── migrations/                # sqlx SQL migrations
│   └── fixtures/                  # synthetic bank files + settlement scenarios
├── python/                        # uv workspace, one lockfile
│   ├── pyproject.toml             # members = sdk, cli, firefly-migrate
│   ├── sdk/                       # generated Python SDK (committed)
│   ├── cli/                       # `exchequer`: Typer + Textual
│   └── firefly-migrate/           # one-time migration tool
├── web/                           # npm: SvelteKit + generated TS client (committed)
├── deploy/
│   ├── Containerfile              # web build → server build → minimal image
│   └── examples/                  # sample quadlets
└── .github/workflows/             # path-filtered: server / python / web / spec-drift
```

### 16.3 Conventions

- **The spec is generated, committed and checked for drift;** both SDKs are regenerated from
  it, and CI fails on any diff.
- **Generated SDKs are committed,** so API changes show up as readable diffs.
- **One container serves the API and the web app** (`/` and `/api/v1`): one origin, one
  image, one quadlet.
- **One version for everything** (server, web, SDK, CLI), and it lives only in git tags:
  builds take it from `git describe` (`deploy/version`; `hatch-vcs` for Python), so
  nothing in the repo is edited to release. They're built from one spec and move together. A tag `vX.Y.Z` on
  `main` publishes both images to GHCR, public since they hold no household data, and a
  GitHub Release with the Python packages. The deployment runs the newest release and
  updates through `podman auto-update`; nothing is built on the server.
- **Real bank files never enter the repo, not even anonymized.** Fixtures are **synthetic**:
  generated files for the fictional household that reproduce each format's quirks (SGML vs
  XML OFX, QBO headers, CSV layouts). Tests on real files read from a local path given at
  runtime and run only on the operator's machine.
- **Keeping the repo clean** relies on conventions (fictional examples, synthetic fixtures)
  and GitHub secret scanning with push protection.
- **Tooling:** Rust: `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, `cargo-deny`.
  Python: ruff, ty, pytest via tox. Web: `npm run check`. A top-level `justfile` runs any of
  them.

### 16.4 License

**BSD-2-Clause.** Consequently, **no GPL dependencies**: every planned dependency is MIT,
Apache-2.0 or BSD, and CI checks licenses (`cargo-deny` for Rust, plus a check over the Python
and npm lockfiles). GPL-licensed OFX libraries (e.g. Python's `ofxtools`) are never
imported; `ofx-check` compares against the output of an external program instead.

## 17. Build order

The components are independent behind the API contract, but they have a natural order:

0. **Groundwork:**
   1. Generic docs (this file and `requirements.md`).
   2. Create the GitHub repo, with secret scanning and push protection on.
   3. **OFX check:** parse real OFX/QFX/QBO downloads with `ofx-rs` (`ofx-check`, files read
      from a local path) and compare with the output of the tool in use today. This decides
      whether `ofx-rs` is used as-is, patched, or replaced for some banks.
1. **Ledger core + invariants** (§3) in `domain`: pure logic and property tests.
2. **REST API + token auth** (§5, §10 tier 1). Local backups ship here.
3. **Firefly migration** (§11), early on purpose: real data makes everything after it
   testable.
4. **Household settlement** (§4) + **query/report engine** (§7), validated against migrated
   history (§4.5). Balance checks and the deterministic discrepancy finder (§7.1) too.
5. **Import pipeline** (§6) + **CLI import/review** (§12).
6. **Classification** (§8): import conditions, then learned, then LLM (including the
   discrepancy finder's AI explanation).
7. **Price fetching + valuation** (§3.4).
8. **Web app** (§13).
9. **Bills/recurrences** (§9), **Entra OIDC** (§10 tier 2).
10. *Low priority:* learned bills (§9.1).
11. *Nice-to-have:* encrypted offsite backups (§14.2).

## 18. Decisions

| # | Topic | Decision | Where |
|---|---|---|---|
| 1 | Securities | One account per institution holds many commodities; per-security accounts are collapsed on migration | §3.3, §11 |
| 2 | Outbound calls | Allowed only while handling a request (prices, LLM, auth keys); never on a timer | §2, §3.4 |
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
| 13 | Investments | Not shared while invested; shared once liquidated into a liquid account | §4.1, §4.2 |
| 14 | Transfers between people | Settlement payments | §4.2 |
| 15 | Privacy within the household | Everyone sees and edits everything | §10 |
| 16 | Liquidations into personal accounts | Default purpose Household, shown as "in transit" lines cleared by matching | §4.3 |
| 17 | Mortgage | Household-owned; paid from shared accounts it's outside the settlement | §4.2 |
| 18 | Stack | Rust server; Python CLI and migration; TypeScript + SvelteKit web app | §15 |
| 19 | Code organization | One public monorepo; household data in the DB; deployment config private | §16 |
| 20 | License | BSD-2-Clause, no GPL dependencies | §16.4 |
| 21 | Import rules | One Rhai script, versioned in the DB, validated before it becomes active | §8 |
| 22 | Repo hygiene | Conventions only: fictional examples, synthetic fixtures, push protection | §16.3 |
| 23 | PII at runtime | `AccountNumber` masking, no bodies in logs; LLM prompt policy designed with §8 | §3.6 |
| 24 | Name | Exchequer: originally the treasury of the royal household | — |

### Open

- LLM prompt contents and minimization (§3.6, §8).
