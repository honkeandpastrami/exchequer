# Exchequer — design

This document describes Exchequer as it is built: what it is, how its pieces fit, and the
components that are built and tested on their own. Requirements are in
[`requirements.md`](requirements.md); how changes are merged, released and deployed is in
[`deployment.md`](deployment.md). Work not built yet is tracked as GitHub issues (§17).

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
   migrations, so it can sleep for days and lose nothing. The one thing a request leaves
   running after it answers is the price fetch it started (§3.4).
3. **The server can reach the internet, but only because of a request.** It calls
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
| **Account** | Something of **yours** that holds value | Required **`owner`** (a Party) and a **`liquid`** flag (investment accounts aren't liquid, §4.1). Checking, savings, credit card, brokerage, mortgage, cash. It has **no fixed currency** and holds any commodities. `kind` only affects display: sign convention, icon, credit-card metadata such as the credit limit. Import metadata: the account numbers bank files name it by. A card whose payments and purchases arrive under two numbers simply has both; a number belongs to one account only. |
| **Counterparty** | Anyone **outside** you | Merchants, employers, utilities, tax agencies. It has **no direction**: the same store is where you spend and who refunds you. Cheap to create (imports create them), mergeable, with *aliases* (raw bank strings → canonical name). Two system counterparties: `Opening balances` and `Reconciliation`. |
| **Transaction** | One real-world event | Date, description, notes, tags, provenance (import batch, raw row, external IDs). Contains one or more splits. |
| **Split** | One movement inside a transaction | A **from-leg** and a **to-leg**, each `(account or counterparty, commodity, quantity)`; an outside leg may leave the counterparty unknown (none, never a placeholder name), plus **`purpose`** (a Party: *who is this money for?*), `category`, an optional own description, and, for a settlement payment, the month it settles when that isn't the default (§4.3). Purpose and category apply only to income and expenses, never to transfers or conversions. |
| **Category / Tag** | Classification | A category is the *class* of an income or expense (Groceries, Salary), never where the money went. Tags are free-form (trips, projects). There are **no budgets**: owner and purpose do the job budgets were often used for. |
| **Settlement** | One month's closing | `(month, status: open/closed, closed_at)`, the month's **stipend per person** and **funder per business** (set by the client before closing). Amounts are always computed live; nothing is frozen (§4.4). |
| **Price** | `1 BASE = rate QUOTE` on a date | Sources: `provider:<name>` or `manual`. Records `fetched_at`, and whether it's *provisional* (a day still in progress). Covers FX and security quotes alike. |
| **RuleScript** | The import conditions | One Rhai script stored as **append-only versions** `(version, source, author, created_at, note)`. The newest valid version is active (§8). |
| **BalanceCheck** | "Account X held N of commodity C at date D" | Entered by the user from the bank's app (optionally dated), or taken from OFX `LEDGERBAL`. Mismatches go to the discrepancy finder (§7.1). |
| **ImportBatch / StagedRow** | The import workflow state | §6. |
| **AuditLog** | Append-only change history | Records which token changed what. |
| **User / Passkey** | A household member who signs in | A name and one or more passkeys (§10). |
| **Token** | A client's credential | Named, stored hashed, revocable; a user's own, or a script's (§10). |

### 3.2 There are no transaction types

Many tools make the user choose withdrawal, deposit or transfer, with a different account
type for each. Here the kind is **never stored and never entered**. It is computed from who
is on each side of a split:

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

Money into an investment or a loan is gone for a long time, so for all practical purposes
it's spent: putting savings in a brokerage or paying down a mortgage is an expense, and
taking money out of an investment is income. Like any expense or income it has a category
(§4.1): one category for both directions nets them, the way a refund nets against its
category; two (*Invest*, *Divest*) show each. On the non-liquid account's side it's only
money arriving or leaving; reports count it once, on the liquid side (§7).

An **adjustment** sets or corrects a balance: an opening balance (a debt's leaves the
account, an asset's enters it) or a reconciliation. It isn't money earned or spent, so
it's neither income nor expense in any report, and it takes no purpose or category.

The API returns `kind` as a read-only computed field for filters and reports (income vs.
expense excludes transfers, conversions and adjustments). Changing a split's account or
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
> implied rate is `qty_from / qty_to`, derived from the split, not stored as a price.

| Event | Splits | Kind |
|---|---|---|
| Groceries | Alex Credit (USD 80) → Grocer | expense |
| Warehouse store, split | Alex Credit → Store 120 *(Groceries)*; Alex Credit → Store 45 *(Home supplies)* | expense ×2 |
| Refund | Store (USD 30) → Alex Credit | income, nets against its category |
| Card payment | Alex Checking (USD 500) → Alex Credit | transfer |
| Mortgage payment | Joint Savings → Mortgage *(principal)*; Joint Savings → Lender *(interest)* | expense ×2 |
| Invest savings | Joint Savings (USD 5,000) → Joint Brokerage | expense |
| Sell and withdraw | Alex Brokerage (USD 10,000) → Alex Checking | income |
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
- Valued figures: the balances report takes `?value_in=USD&price_date=…`, and the
  settlement values every split in USD at its own date (§4.2). The server converts using
  the most recent price on or before the date. A figure comes from **one price** (or
  its inverse) whenever one exists: sources disagree, and chaining two can compound the
  disagreement. Choosing among prices:
  1. **Fresh before stale:** a price at most a few days (configurable, default 5) older
     than the date beats any older one; only when nothing is fresh is the newest older
     figure used, marked stale.
  2. **Direct before derived:** one price beats a chain of two through a pivot commodity
     (GBP→EUR→USD), and a chain within one source (one publication) beats one mixing
     sources. A chain is a backup, and a derived figure says so.
  3. **Source order:** manual prices first, then the configured sources in order, then
     sources no longer configured.

  `GET /prices/value?from=GBP&to=USD&date=…` shows the choice for one pair.
- Converted figures always state **which price and which date** were used, and flag missing
  or stale prices. The server never silently invents numbers.

**Price fetching** never makes a request wait:

1. A valuation needs `ACME→USD` on date D. If the price exists, it's used.
2. If not, the request answers at once with the latest stored price (the figure marked
   stale, or missing) and `fetching: true`, and starts fetching the missing prices **in one
   batch** in the background (every commodity × date the request needs, back-filling gaps
   since the last stored price). A pair already being fetched isn't fetched twice.
   `GET /prices/fetching?wait=25` answers once every fetch running has finished (or the
   wait, at most 50 s, is over); then the same request answers with what arrived. The web
   app does this by itself, keeping what it shows until the new answer arrives. Storing a fetched price is the one
   deliberate exception to "a GET never writes": a historical closing price never changes,
   so storing it is caching an external fact.
3. A price for a day still in progress is *provisional*: refetched when older than a
   configurable age and replaced by the close once available.
4. If the provider is unreachable or rate-limited, nothing is fetched and answers keep the
   latest stored price, marked stale. Outages degrade answers, never fail them.
5. `POST /prices/refresh` starts a forced fetch and answers at once (`202`);
   `GET /prices/refresh?wait=25` answers once it's finished (or the wait is over) with how
   it went. While one runs,
   another isn't started.

**Sources** are pluggable and tried in order, each a `PriceSource` behind one interface
(how it would value a pair, and a batch fetch of pairs × dates), so a source can be added,
replaced or dropped without touching valuation:

1. **Tiingo** (primary): end-of-day closes for US stocks, ETFs and mutual funds (in USD),
   and FX. Its API key is the `exchequer_tiingo_api_key` podman secret (§14.1); without
   it, Tiingo sits out.
2. **ECB reference rates** (backup, FX only): official daily rates of about 30 currencies
   against the euro; public, no key. Any other pair is two of them chained through EUR, so
   EUR must be a commodity for that.

A price is asked of the next source when one doesn't cover the pair, fails, is
rate-limited or has nothing; the first that answers is recorded (`provider:<name>`), so a
source's prices outlive it. A source that answered isn't asked for the same pair and days
again within an hour (a ticker it doesn't know would otherwise cost a request on every
page). A source that didn't answer (the network, a timeout, its server's error) is passed
over for the rest of that fetch and for a minute after, then asked again: a network that's
down for a few minutes, as after the machine wakes, costs one failed request per source,
not one per pair, and prices arrive soon after it's back. A rate-limited source waits 15
minutes. `POST /prices/refresh` (also on Setup → Prices, and `exchequer prices refresh`)
asks anyway. Only
commodity codes and dates are sent.

A source that needs a key has its own secret, `exchequer_<source>_api_key`, read as
`<SOURCE>_API_KEY_FILE`; adding a source adds a secret, never a shared one. Which sources
run, and in which order, is configuration (`EXCHEQUER_PRICE_SOURCES`, default
`tiingo,ecb`; `off` for none), as is freshness (`EXCHEQUER_PRICE_FRESH_DAYS`, default 5).
Manual prices always win over fetched ones.

### 3.5 Numbers: no floating point

**Rule:** every monetary amount, commodity quantity, price, exchange rate, and any value
derived from them uses exact decimal arithmetic in every layer: parsing, storage, the API,
computation, AI prompts and clients. A float touching one of these values is a bug.

| Layer | Representation | Enforcement |
|---|---|---|
| Bank file parsing | String → decimal directly. `ofx-rs` yields `rust_decimal::Decimal`; the CSV path parses strings straight into `Decimal`. | No float conversion anywhere in parsers or fingerprinting. |
| Postgres | `NUMERIC`. Quantities are checked against the commodity's precision. Prices and rates use `NUMERIC` with a fixed high scale (e.g. 20). | A schema test fails if any column is `real` or `double precision`. |
| Server (Rust) | Newtypes: **`Amount`** (wraps `rust_decimal::Decimal`, 28 significant digits) for amounts and quantities; **`Rate`** (wraps `bigdecimal::BigDecimal`, arbitrary precision) for prices and conversion math. Neither has any operation accepting `f32`/`f64`, so mixing is a **compile error**. `Amount` arithmetic is checked: overflow or inexact rescaling is an error, never a silent rounding. | `clippy::float_arithmetic`, `clippy::float_cmp` and `disallowed_types = [f32, f64]` are denied workspace-wide, as a guard for money. Code that needs a float for something else opts out locally (`#[expect(…, reason = …)]`); the `heuristics` crate opts out as a whole. |
| API (JSON) | Amounts are **strings** (`"1500.00"`); the OpenAPI schema declares `type: string` with a decimal pattern. | `Amount`'s deserializer accepts only JSON strings; a JSON number is rejected with 422. |
| CLI (Python) | `decimal.Decimal` end to end; the generated SDK maps amount strings to `Decimal`. | `just python-check` fails on `float(`, `: float` or `-> float` in client code. |
| Web app | Amounts stay strings; the rare client-side math uses a decimal library. The server returns computed totals. | Lint rule against `Number()`/`parseFloat` on amount fields. Charts map values to pixels internally; labels always come from the strings. |
| AI | Amounts go into prompts as strings. **No arithmetic is ever taken from an LLM**: totals, differences and subset sums are computed in `Decimal`; the model only ranks and explains. | Structured outputs reference transaction IDs, never amounts the code would trust. |

**Rounding** happens only at explicit boundaries:

- **Ledger amounts are exact.** An amount with more decimals than its commodity's precision
  is rejected, never rounded silently.
- **Conversions** compute at full precision and round **once**, at the end, to the target
  commodity's precision, using one documented mode (`ROUND_HALF_EVEN`). A total is the sum of
  the rounded values shown, so a column always adds up to its total.
- **Derived rates** (an inverse, a chain of two prices, a conversion's implied rate) are
  rounded once to the fixed rate scale. They are used for valuation only, never to rewrite
  a stored amount.

**Scope:** the rule is about money: amounts, quantities, prices, rates and the transactions
that carry them. Anything else may use floats, as long as a float never becomes stored data:
text similarity in dedup (e.g. `pg_trgm`'s `similarity()`), classifier confidence, the
internal math of the learned model. Integers are still preferred where they come easily,
since they reproduce exactly. Amount *features* for the model are derived from `Decimal` in
one audited function.

**Imported data may already be damaged.** Data exported from float-based tools can carry
values like `1234.5699999999999`. Importers and the Firefly migration detect any amount with
more decimals than its commodity allows and report it for confirmation instead of storing
it or rounding it silently.

**Test corpus:** values that break floats (`0.1 + 0.2`, zero-decimal currencies,
three-decimal share quantities, large balances, float-damaged inputs) plus `proptest`
properties asserting that split sums, balances and converted totals are exactly
reproducible.

### 3.6 PII at runtime: logs and remote AI calls

The server stores the household's data and shows it to the household. It must not leak that
data through side channels:

- **Logs.** Account numbers are only ever shown as their last four digits, and tests assert
  on masked values. `AccountNumber` in `domain` formats itself as `…1234` in `Display` and
  `Debug`; getting the full number takes an explicit `.reveal()`, used only by parsers and
  the DB layer. Request and response bodies and raw bank rows are never logged.
- **Clients.** The API returns account numbers as their last four digits. A client gets a
  full number only by asking for it explicitly, per request (`?reveal=numbers`), with a
  token; the web app asks only when the signed-in user has chosen to see full numbers
  (`docs/ux.md` §8), a choice kept in that browser and never on the server. Masked or
  not, numbers never go to logs.
- **Remote AI calls** (§8) go through `outbound` (§15) and carry features, not
  identifiers: account numbers only as their last four, and descriptions normalized so
  runs of five or more digits (card, reference and trace numbers) are replaced. A request
  the service refuses (e.g. a content filter or PII guardrail its operator configured)
  makes the remote level step aside for that row, logged (the status and error code,
  never the request); the import never fails because of it. Details in [`docs/ai.md`](ai.md).

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
  *liquid*; brokerages are *investment* accounts, and a mortgage is never liquid.
  Investments are never shared while they stay invested: activity inside them (vests,
  dividends, buys, sells, fees) has no purpose and doesn't enter the settlement. Money
  into or out of them is an expense or income (§3.2).
- **Totals (display).** Balances are added up apart for *liquid* accounts, *investments*
  (not liquid) and *loans* (mortgages: large and slow, they'd hide the rest), and mixed
  only in a **Net** line.
- **Purpose (per split, the manual part).** The party the money was *for*. It applies only to
  income and expenses on liquid accounts (money into or out of an investment included).
  Transfers and
  conversions have no purpose and no category, because they don't change how much anyone
  has.
- **No default.** Every income and expense on a liquid account names its purpose and its
  category: a transaction can't be saved, nor an import row committed, without both, so
  nothing is ever "uncategorized". Money into or out of an investment or a loan is
  categorized like any other expense or income (*Invest*, *Divest*, *Mortgage
  principal*). Suggestions name both too (§8), so household accounts need almost no
  review; on personal accounts, where a large share of spending is often household
  spending, the purpose is **predicted**.

### 4.2 What creates a claim between parties

Every value is in USD, the settlement currency, **at the transaction's own date** (§3.4),
computed in `Decimal` (§3.5). A split with a leg in USD is worth that leg; one without is
its account leg valued at that day's price, and the statement line shows which price. A
split no price connects to USD is listed as unpriced and left out.

| Event | Example | Claim |
|---|---|---|
| Income/expense where owner ≠ purpose | Groceries on Alex Credit, purpose Household | Household owes Alex $80 |
| | Salary into Alex Checking, purpose Household | Alex owes Household the salary |
| | Studio supplies on Sam Credit, purpose Studio | Studio owes Sam |
| Money out of an investment (income), for the Household | Alex Brokerage → Joint Savings, $10,000 | Landing in a Household account, it simply *becomes shared*: no claim. Landing in Alex Checking, Alex owes it to the Household, until moved on (a settlement payment) |
| Money into an investment or a loan (an expense), for the Household | Alex Checking → Mortgage *(principal)* | The Household owes Alex |
| **Settlement payment**: any other transfer between liquid accounts of different owners | Alex Checking → Joint Savings; Joint Savings → Sam Checking; Alex → Sam directly; Joint Savings → Studio Checking | Counts as *paid* against the claims between those parties |
| **Stipend** | set per person each month (§4.3) | Household owes the person their stipend |
| Owner = purpose, transfers within one owner, conversions, investment activity | — | none |

**Routing.** A claim between two parties is added to every edge on the tree path between
them. Sam paying a Studio expense is routed Studio → Household → Sam while Studio is
household-funded, and directly Studio → Sam if Sam becomes the funder. Moving a business
only changes which edges its claims cross.

**Mortgage.** Its principal is Household spending. Paid from Joint Savings it never enters
the settlement; paid from a personal account, the Household owes that person.

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
- **Household pool view:** household income, household expenses by category, stipends, net
  business funding, and the resulting change in shared savings.

**Review and close.** A bookkeeping step, not an approval workflow: no sign-offs; whoever
does it closes the month.

1. Import the month's files for every account (§6).
2. **Review the labels** in staging: every income and expense names its purpose and
   category, pre-filled by suggestions (§8) with confidence and reason; only unsure ones
   need a look.
3. **Set the month's parameters:** each person's **stipend** and each business's **funder**,
   pre-filled from the previous month. Closing requires them (a stipend of 0 is fine).
4. **Close:** the statement (`GET /settlements/{M}/statement`, the web app's Settlement
   page) shows each edge's due, paid, carried and outstanding, and its lines; closing
   marks M closed.
5. **Pay:** a settlement payment settles the month before the one it's made in, so the
   transfers made in M + 1 count as `paid` for M. A payment meant for another month names
   it on its split (`settles`).

### 4.4 Closed months and reopening

*Closed* only protects against **accidental** edits. It's a guard rail, not a lock.

- Edits to transactions dated in a closed month are rejected with a clear error ("2026-06 is
  closed; reopen it to edit"), and so is a commit with a row dated in one, naming the row.
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
  `POST …/close`, `POST …/reopen`), commodities, accounts (and their numbers),
  counterparties, transactions (splits nested, and the bank rows they came from),
  categories, tags, prices, balance checks, imports (staging, §6), import batches, import
  fingerprints, rules, suggester backtests, tokens. `GET /healthz` and `GET /version`
  need no token.
- **Queries:** `POST /query` and the `/reports` family (§7).
- **Conventions:** cursor pagination; `If-Match`/ETag on edits; idempotency keys on POSTs so
  clients can retry safely; Problem Details (RFC 9457) errors that name the violated
  invariant.
- Amounts travel as **decimal strings**, never JSON numbers (§3.5).

## 6. Component: import pipeline

```
upload raw file ─▶ detect ─▶ parse ─▶ resolve account ─▶ fingerprint/dedup
     ─▶ suggest (rules → local AI → remote AI) ─▶ transfer matching ─▶ STAGED batch
     ─▶ client review (accept / edit / reject per row) ─▶ COMMIT (atomic)
```

1. **Format detection and parsers.** Each format is a plugin behind one Rust trait: CSV with
   a column-alias table, OFX/QFX/QBO via `ofx-rs` (OFX 1.x SGML and 2.x
   XML), and content sniffing for OFX content under unexpected extensions. `ofx-rs` 0.2.0 is
   wrapped by a hot-patch (`import::ofx`) for three bugs: bank-specific
   tags inside a transaction (kept as *extensions*, e.g. a merchant category code, useful
   for classification), dropped entities like `&amp;`, and mangled non-ASCII text in
   OFX 1.x. The patch is removed once upstream fixes them. Dates are the server's local
   dates (`TZ`): a time written with a zone is converted, one written without is taken as
   written. Parsers emit normalized rows (date, amount,
   description, native ID) plus the raw row for provenance.
2. **Account resolution.** An OFX statement's account number is looked up among the
   accounts' numbers (stored without spaces or dashes, shown as their last four unless the
   user asks for them, §3.6). A
   CSV names no account. A statement whose account isn't known (a CSV, an unknown number)
   is staged anyway, its rows `needs-account`, and the account is picked in review (the
   upload may name it if the client already knows), or suggested (§8). Picking it can
   add the file's number to the account.
3. **Deduplication,** done once, on the server, with full DB access:
   - *Exact:* native ID (FITID or bank reference), or a content hash with an occurrence
     suffix for identical rows in one file; unique per account (`import_fingerprint`,
     recorded on commit and deleted with its transaction). Transactions that came in another
     way get theirs through `POST /import-fingerprints`; the Firefly migration records each
     journal's `external_id`, the ID the preimporter gave the row (§11).
   - *Near:* same account, commodity and signed amount, date within ±3 days, matched
     one-to-one: the most similar descriptions first (`pg_trgm`), then the closest dates.
     Similarity only ranks: a transaction typed by hand rarely reads like the bank's text,
     so a low score doesn't clear a row. Near-duplicates are **flagged in staging**, not
     prompted mid-run.
   - Re-uploading a file, or overlapping date ranges, is a no-op.
4. **Transfers.** A card payment appears in both the checking and the card file; it's one
   transfer, never an expense plus an income.
   - *The other side* of a row is a counterparty or, for a transfer, one of your accounts:
     chosen in review, or suggested (§8). A transfer has no category or purpose; between a
     liquid and a non-liquid account it's an expense or income, which has them (§3.2,
     §4.1).
   - *Potential transfer matches:* each row's rows in your other accounts, in any staged
     import (this file's other statements, other files), with the opposite amount in the
     same commodity, dated within 5 days. Every suggestion level sees them
     (`row.transfer_candidates` in rules).
   - *Transfer match* (`transfer_match`): two staged rows that are the two sides of one
     transfer: one settled as a transfer to the other's account (chosen, or a confident
     suggestion), the other open or settled as the transfer back, a potential transfer
     match of each other. Matched one-to-one (`import::matching`): agreeing pairs first,
     then the closest dates. A chosen counterparty, or a decision made on a row, rules it
     out. Worked out again across all staged imports whenever one changes (upload, a
     statement's account, a row, discard, commit, refresh), after their open labels are
     suggested again (§8), so a pair can form from either side's new suggestion.
   - *Transfer duplicate* (`transfer_duplicate`): the transfer is already in the ledger,
     from the other account: a one-split transfer between the account and another of
     yours, the same amount the same way, within 5 days, holding no bank row of this
     account yet. Matched after exact duplicates and before near ones.
   - A transfer whose first side is committed before the other is staged is recorded as a
     transfer then; one-sided transactions in the ledger are never turned into transfers
     afterwards.
5. **Suggestions** (§8): the statement's account when unknown, then each row's
   counterparty, purpose and category, each with a confidence, a reason and the exact
   suggester.
6. **Staging and review.** The batch lives on the server, so a review started in one client
   can finish in another. Rows are `new`, `duplicate`, `near-duplicate`, `transfer-match`,
   `transfer-duplicate` or `needs-account`. `POST /imports` takes the file's bytes (at most
   10 MiB; optional `account_id`) and stages one statement per account in it; `GET
   /imports/{id}` shows each statement's account, ledger balance, skipped lines and row
   counts by status, `GET /imports/{id}/rows` the rows; `PATCH
   /imports/{id}/statements/{n}` sets a statement's account (optionally remembering the
   file's number for it) and works its rows' statuses out again; `DELETE` discards it.
7. **Commit** is all or nothing. Each row has a decision (`PATCH
   /imports/{id}/rows/{statement}/{position}`): `match` (the default for duplicates,
   near-duplicates, transfer matches and transfer duplicates), `new` (the default for new
   rows; never for a duplicate; for a transfer match, both sides are added as new and stay
   a potential match, so `match` brings the pair back; the two sides always share the
   decision), `replace` (a
   duplicate or near-duplicate whose bank row should win: the
   matched transaction keeps its ID, notes, tags and batch and takes the row's date,
   amount and description, and the labels set in review; it must be one split between
   the statement's account and the outside) or `skip` (shown as *Do not import*), plus optional labels for the new
   transaction (description, the other side, category, purpose). A row added as new
   (not a transfer match) can instead be **split** into parts that add up to its amount,
   each with its own other side, category, purpose and note; its labels are then the
   parts', all yours, and it isn't matched as a transfer. `POST
   /imports/{id}/commit` first works every status and transfer match out again; if one
   changed, nothing is committed and the client reviews again (409). Then: `new` rows
   become transactions in one `file` import batch, one split each (the other side is the
   chosen counterparty, unknown, or one of your accounts for a transfer), or one per part
   for a split row; a transfer match
   becomes one transfer, dated and described as the side the money left, and both rows
   are recorded with it, so the other file's row is a duplicate of it from then on; every
   row now in the ledger is recorded in `import_fingerprint`. **A row that matches a
   transaction already in the ledger (a duplicate, near-duplicate or transfer duplicate)
   never creates anything: what it says is added to that transaction.** Other staged
   imports are then worked out again, since the ledger changed. Each row is kept as each file had
   it (`imported_row`: raw fields, when), the transaction's provenance; a file is identified
   by its bytes' SHA-256, so a copy under another name adds nothing (`GET
   /transactions/{id}/imported-rows`). A closed month or a broken ledger rule refuses the whole commit,
   naming the row. Each transaction keeps its `import_batch_id`, so undoing a batch is one
   call; it deletes the new transactions and their recorded rows. Every transaction has a batch and
   edits never change it: a migration is a batch, and a transaction entered by hand is a
   batch of its own. Uploaded files aren't kept; only each row's parsed raw form is.

   **Readiness.** Each staged import says how close it is to committing (`readiness`):
   - `needs_work` (red): a statement has no account; a new row has a label, or a row
     its decision, that is neither chosen nor confidently suggested. Each comes with a
     count. A row's decision is matching's suggestion (docs/ai.md): sure of a new row
     or an exact duplicate, as sure as the dates are close for a transfer, and never
     sure of a near duplicate, which waits for you. Commit refuses exactly these (409:
     `labels-missing`, `decisions-missing`, or no account), so every client and page
     follows the same rule.
   - `suggested` (yellow): it would commit, but it relies on confident AI suggestions
     (local or remote) or a suggested account that you haven't confirmed, short of 100%.
   - `ready` (green): it commits exactly as shown. A suggestion from logic (the rules,
     matching), or any at 100%, counts as yours.

   The web app shows the same mark and reasons on the Imports list and on an import's
   review, where its rows to label or decide are one filter away; either page can
   commit any import that isn't `needs_work`, through the one commit endpoint. **Commit N ready** (`POST /imports/commit-ready {ids}`) commits the `ready` ones
   that were listed. It goes oldest first, each import on its own, and checks each one
   again just before its turn, since committing one changes the others. Each import comes
   back committed or skipped, with the reason: gone, not ready, changed since it was
   checked, or refused.
8. **Balance check.** An OFX `LEDGERBAL` is recorded on commit as a balance check (source
   `ofx`), returned with the commit's result, with candidates if it doesn't match (§7.1).

## 7. Component: query and reporting engine

- **One general aggregation endpoint,** `POST /query`, over postings (one split's movement
  on one of your accounts, signed): a **flow** (the sum over a range) or a **balance** (at
  its end), filtered by dates, accounts, owners, purposes, categories, counterparties,
  kinds, tags and text, grouped by account, owner, purpose, category, counterparty, kind or
  tag, and bucketed by day, week, month, quarter or year. Rows are always per commodity:
  the query doesn't value.
- **Named reports:** the monthly settlement statement (§4.3), the household pool of a month
  (`/reports/household-pool/{month}`), and balances per account and commodity on a date,
  optionally valued (`/reports/balances?value_in=`, §3.4). The web app builds its
  spending, balances and pool reports on these.
- **Implementation:** splits are classified by `domain`, as everywhere else, so a report
  can't disagree with the ledger rules. Household-scale data aggregates in milliseconds;
  nothing is cached.

### 7.1 Balance checks and the discrepancy finder

The user enters what the bank's app shows: one number, optionally with a date (the web
app's Checks page, `POST /balance-checks`), as the bank shows it (the amount owed is
positive on a card or a mortgage). An OFX file's ledger balance is recorded the same way
on commit (§6). The server compares it with the ledger balance on that date.

- **Match:** the account is *verified through D*. Later checks only search the window after
  the last verified date.
- **Mismatch:** the **discrepancy finder** (`domain::discrepancy`, no AI) searches that
  window and returns candidates, most likely first, each saying why: a pair of postings of
  the same amount a few days apart, one equal to the difference (recorded twice); one
  posting equal to the difference (recorded twice, or not yet at the bank); one dated just
  around D (the bank dated it on the other side); a sign flip (difference = 2 × amount);
  two or three postings summing to the difference; two adjacent digits swapped in one
  posting; any pair of postings of the same amount a few days apart; and, when no single
  posting accounts for it, a difference that's a multiple of 9 in the smallest unit (the
  mark of a transposition). Nothing is changed.

## 8. Component: classification (rules and AI)

Imported rows are labeled from suggestions made **in staging** and **reviewed in
staging**; nothing is committed unreviewed. How it operates (what's shown, what commit
takes, the API) is in [`docs/ai.md`](ai.md).

- **Targets,** in priority order: **counterparty, purpose, category**, suggested together
  as one proposal per row (they're highly correlated), each with its own confidence. A
  statement's **account** is suggested first, when the file doesn't settle it; a
  confident one counts as the account and the rows are worked out against it. A row's
  **decision** (match, new, replace, skip) is suggested too, by matching (§6, `docs/ai.md`).
  Tags aren't suggested.
- **Transfers are suggested like anything else:** the counterparty label is the row's
  *other side*, a counterparty or one of your accounts. Once it's settled as an account,
  the row's category and purpose are settled as none (unless one account is liquid and
  the other isn't: then it's an expense or income). Every
  level sees each row's potential transfer matches in other staged files (§6.4), and
  local AI learns from past transfers. Pairing two staged rows into one transfer is then a
  rule, not a suggestion (§6.4).
- **A stack of three levels,** tried in order: rules → local AI → remote AI. A level runs
  while a label is still below **0.9 confidence**; labels already settled (by an earlier
  level at ≥ 0.9, or by you) are given to the next level as facts. Below 0.9 everywhere,
  the best suggestion is kept but shown as unsure. **Suggesting again:** the whole stack
  runs again for the labels that aren't yours or confident, on every other staged import
  when a file is uploaded or committed, and on one import on request (`POST
  /imports/{id}/refresh`, which also works its statuses out again). Your labels and
  confident suggestions stay.
- **Every suggestion names the exact system that made it** (`suggester`: kind, name,
  version, details), with a confidence stored per mille (no float is stored, §3.5) and a
  reason. On commit, each suggestion is recorded with what was committed
  (`suggestion_outcome`), to measure each suggester and compare models.
- **Review:** badges, one hue per level, solid when confident, outlined when not; your own
  choices plain. **Commit takes what's on screen** and is refused (`409`) while a
  statement has no account or a new row has a label neither yours nor confident. An
  unknown counterparty is an explicit choice; a purpose or category never is none on an
  income or expense (§4.1).

The levels:

1. **Rules, as code: one [Rhai](https://rhai.rs) script** stored in the DB (`RuleScript`).
   Rhai is a scripting language for embedding in Rust, with Rust-like syntax. It's
   sandboxed (no file system, network or process access) and limited in operations per
   call. The active version is compiled once and runs for each staged row. If/else is the
   rule language. A rule that sets a label is confidence 1.0.

   ```rust
   fn classify(row, s) {
       let d = row.description.to_upper();
       if d.starts_with("ACME CORP PAYROLL") && row.amount > 0 {
           s.counterparty("ACME Corp"); s.category("Salary"); s.purpose("Household");
       } else if d.contains("CITY WATER") && row.account == "Household · Joint Checking" {
           s.counterparty("City Water"); s.category("Utilities"); s.purpose("Household");
       }
   }
   ```

   Rhai is built with `no_float`, and amounts reach scripts as the server's `Amount` type
   (compared via `amount("…")` literals), so §3.5 holds inside scripts. Suggestions name
   the script's version and line. The script **starts empty**: rules are written where
   local AI is measurably wrong in a way a rule fixes (typically a purpose that depends on
   the account, amount or date), not carried over from another tool.

   **Editing and versioning** (details in `docs/ai.md`):
   - `PUT /rules` uploads a new version. The server compiles it and checks every name it
     uses (counterparties, categories, parties, accounts) against the DB. **An invalid
     script is rejected with line-numbered errors (`rules-invalid`) and never becomes
     active.** A valid one is active immediately, without a restart. A script a later
     deletion made invalid is skipped (rules sit out) until it's fixed.
   - `POST /rules/dry-run?from=&to=` reports what the active or a candidate script *would*
     change in the ledger's past, without storing anything.
   - Every version is kept (`GET /rules/versions`): list, diff, and rollback (re-uploading
     an old version as a new one). Each version records its author and an optional note.
   - CLI: `exchequer rules show | edit | upload | log | diff | rollback | dry-run`.
2. **Local AI, lexical,** in-process and pure Rust: descriptions normalized (card and
   reference digits, dates, processor prefixes stripped), an alias memory (this text was
   counterparty X the last N times), and nearest labeled transactions by trigram/TF-IDF
   similarity voting on the whole combination of labels, weighted by similarity; the
   account's owner, direction and amount break ties. Confidence is calibrated by a
   backtest (train on earlier months, predict the next), so 0.9 means about 90% right on
   the ledger's own data. Embeddings aren't used (#10). No model files, no training
   pipeline. Local methods are interchangeable behind one interface, picked by
   `EXCHEQUER_LOCAL_AI` and measured on the ledger's past by a backtest endpoint
   (`docs/ai.md`).
3. **Remote AI, optional:** a decision model deployed on Azure AI Foundry, asked
   multiple-choice questions whose options are the ledger's own names (plus *unknown
   counterparty*, *none of these* and *can't tell*), and answering with a probability per option. One question set per row,
   one per statement for the account. The context it's given is computed from the data
   on each request (similar past transactions, usage statistics of the account and of
   each option), never hand-written descriptions that would go stale. What may be sent is
   in §3.6. It's asked only about what the levels before it left unsure. The questions
   are versioned (`q1`, …) and recorded with every suggestion, and the backtest can include
   it (`remote=true`), so a new wording is measured before it's used. Configured by
   `EXCHEQUER_REMOTE_AI_*` and a key secret (`docs/ai.md`). The system works fully
   without it.

## 9. Component: bills and recurrences

Not built: bills, expectations with a schedule, amount phases and a lifecycle that imports
would link splits to, and recurrences that produce transactions on request (#35); bills
learned from the transaction history (#36). The ledger has no bill field yet.

## 10. Component: authentication

A pluggable middleware layer: every request resolves to a principal or is rejected.

- **Users sign in with passkeys** (WebAuthn): Face ID, Touch ID, Windows Hello or a
  security key. There are no passwords and no outside identity provider. For each passkey the
  server keeps its credential ID, public key and signature counter, nothing secret, so a
  leaked database lets no one sign in. A user is a household member with a name; the audit
  log ties every change to one through its token.
- **Invites.** A user is added with a one-time invite link, valid for 24 hours: opening it
  registers a passkey for that user. Nobody registers without one. The same kind of link
  adds a passkey on another device, or replaces lost ones. Removing a user deletes their
  passkeys and revokes their tokens.
- **Sign-in** is usernameless: the browser offers the passkeys it holds for the server's
  domain. Success creates a **session**, a token that belongs to the user and expires after
  30 days; signing out revokes it.
- **The CLI signs in through the browser,** like OAuth's device flow: `exchequer login`
  shows a link with a short code, a signed-in user approves it, and the CLI receives a token
  of its own, kept in its token file.
- **Passkeys are bound to the server's domain:** `EXCHEQUER_ORIGIN` (e.g.
  `https://books.example.org`, or `http://localhost:5173` in development; a path under it
  is fine). Moving to another domain means registering again. Without it, passkey sign-in
  is off, and with it every way to get a new token: only tokens made before keep working.
- **Bearer tokens** underlie everything, of three kinds: **sessions** (a browser signed in),
  **the CLI's** (`exchequer login`) and **scripts'** (made by hand, in Setup → Security or
  with `exchequer tokens create`). Each **belongs to a user**: the one who signed in,
  approved the CLI, or made it. Only a session or the CLI's token makes tokens or approves
  the CLI: a leaked script's token can't outlive its expiry. They're stored **hashed** (SHA-256: the secrets are 256
  random bits, so a slow password hash adds cost without adding safety), named, and shown
  once. Invite links and the CLI's device codes are stored hashed too; a ceremony's
  challenge lives in the database for five minutes and is used once.
- **Every token expires.** Its time to live is chosen when it's made: 30 days for a
  session; for the CLI's and scripts', what the person making it asks for, at most 365
  days (`exchequer login` asks 90 unless told otherwise, and the approver sees it). Each
  records when it was created and, to the minute, when it was last used.
- **Revoking.** Any token can be revoked at any time, in Setup → Security, with
  `exchequer tokens revoke`, or through `DELETE /tokens/{id}`: it stops working at once,
  and nothing else does. Signing out revokes the browser's session. **The kill switch**
  (`DELETE /tokens`, also in Setup → Security and `exchequer tokens revoke-all`) revokes
  every token at once, everyone's, the caller's too: everyone is signed out and every CLI
  and script stops; passkeys still sign in, and the CLI signs in again with
  `exchequer login`.
- **Bootstrap: one way in, for passkeys and tokens alike.** The bootstrap token from a
  podman secret (§14.1) is accepted only for managing users: it adds the first user, whose
  invite registers their passkey. Every token then comes from a signed-in user, so the
  first token follows the first passkey: a session when they sign in, the CLI's when they
  approve `exchequer login`, a script's when they make one. Someone who lost every
  passkey gets a new invite from another user, or from the bootstrap token. The steps are
  in [`deploy/README.md`](../deploy/README.md#install), step 6.
- **TLS** is terminated by the operator's existing reverse proxy. Passkeys require it.

**One ledger, several people.** Everyone signed in sees and edits everything, users
included; the audit log records which token, and so which user, changed what. No
multi-tenancy: one household per server.

## 11. Component: Firefly III migration (one-time)

A tool for households moving from Firefly III. It migrates **one calendar year at a time**,
into an empty server or before or after the years the server already holds, and stops
before writing anything that could duplicate or corrupt data. How to use it, and the mapping file:
[`firefly-migration.md`](firefly-migration.md).

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
    There's no default (§4.1): a split neither names, or an income or expense with no
    Firefly category, takes the label from the server's copy of the transaction (with
    `--overlap`) or from the mapping's decision for that transaction (`[transactions]`:
    a purpose, a category, or a reconciliation adjustment), and stops the run without
    one, in the dry run too. Before the year, where only the sums matter, missing labels
    are only counted. Money into or out of a non-liquid
    account was a Firefly transfer, with no category: the mapping names one per account
    and direction (e.g. *Invest* and *Divest*, *Mortgage principal*).
    Conflicting labels are listed in the report. Budgets and the encoding tags are then
    retired.
  - **Settlement history:** transfers between personal accounts and shared savings become
    settlement payments linked to their month; transfers into or out of investments and
    mortgages become expenses and income; transfers to a business are funding.
  - **Categories and tags** carry over. **Not migrated:** budgets, piggy banks, bills, and
    categories that only restate a transfer. Firefly rules are not carried over: the rules
    script starts empty (§8).
  - **Float-damaged amounts** are rounded to the commodity's precision and **listed in the
    report** for confirmation, never fixed silently (§3.5).
  - **Split transactions** keep their split structure.
- **Year scope:** for year Y, one **opening-balance** transaction per account and commodity on
  Jan 1 (from Firefly's balance that day), then every transaction in Y, keeping Firefly IDs
  as external references.
- **Fitting the server's data.** `plan` (the dry run) and `run` read what the server holds
  and show what `run` would write, create and replace; `run` writes only when nothing
  stops it, and the dry run fails for the same reasons. Reference data (parties,
  commodities, accounts, counterparties, categories) is reused by name and created when
  missing; a party, commodity or account the server has with another kind stops it.
  - **Before the server's years** (e.g. 2025 under 2026): the server's opening balances
    on the day after the cutoff are replaced by the year's end, which must match them to
    the cent for every account and commodity. Their month is reopened for it if closed,
    then closed again.
  - **After them:** no opening balances are written; the server's balances on Jan 1 must
    be Firefly's, to the cent.
  - **Dates the server already holds data for** stop it unless `--overlap` allows them.
    Then each planned transaction already there (by Firefly ID; an opening balance by
    account and commodity) is skipped if unchanged; one that changed stops it, and so
    does anything else there that Firefly doesn't have.
  - **Duplicates, always:** a Firefly ID already on the server outside the year, a near
    duplicate (§6.3: same account, commodity and signed amount within 3 days, 5 for a
    transfer) among the year's transactions or the server's non-migrated ones around it,
    a bank row (§6.3) already recorded for another transaction, or a write into a closed
    month.
  - The statements of closed months after the year are compared before and after; one
    that changed fails the run.
- **Acceptance check** (`check`, `settlement`): per-account balances at every month end,
  category totals per month, and recomputed settlements vs. transfers actually made (§4.5).
  Done when it shows zero differences, or every difference is explained.
- **After the migration:** `settle` sets the migrated months' stipends and funders and
  closes them; `numbers` and `fingerprints` add the account numbers and the bank rows'
  IDs, so later imports resolve accounts and recognize rows already migrated (§6).
- **Firefly is never modified:** the tool's client refuses every request but GET.

## 12. Component: CLI

A thin client built on the generated SDK; it never imports server code. It reads the server
and a token file from `~/.config/exchequer/config.toml` (or `EXCHEQUER_URL` and
`EXCHEQUER_TOKEN_FILE`), and never prints the token.

- `login`: sign in through the web app (§10) and write the token file.
- `import <files…>`: upload, review each statement and row in the terminal, then commit
  (`--yes` takes every default; `--account` names the account a file doesn't).
- `rules show | edit | upload | log | diff | rollback | dry-run` (§8).
- `prices refresh` (§3.4).

Everything else (settlement, reports, balance checks, setup) is in the web app.

## 13. Component: web app

- A static build (SvelteKit) served by the API container at `/`, the same origin as
  `/api/v1`, so no CORS. It works under a path when a reverse proxy strips it.
- Sign-in is with a passkey (§10), or an API token; the session is kept in that browser
  until signing out. Setup → Security holds all of sign-in: it adds people, makes invite
  links, lists passkeys and tokens (created, last used, expiry), makes scripts' tokens,
  revokes any, and has the kill switch.
- Pages: **Accounts** (balances on a date, valued in USD, by owner and liquid,
  investment and loan, with a link to check each against the bank); **Transactions**
  (search, and editing with splits); **Imports** (upload, review and commit, §6);
  **Settlement** (each month's state across a year, a month's statement, its parameters,
  close and reopen); **Reports** (spending, balances, household pool); **Setup**
  (parties, accounts, commodities, prices, counterparties, categories, tags); and
  **Checks** (balance checks and the discrepancy finder, §7.1).
- Talks only through the TypeScript types generated from the spec.
- Looks and behaves as `docs/ux.md` says.

## 14. Component: deployment and operations

- Rootless podman quadlets on a private podman network: `exchequer-server.container` (the
  API and the web app), `exchequer-db.container` (PostgreSQL 17, no published port) and
  `exchequer-backup.container` with its timer (§14.2). The API publishes only to
  localhost, behind a reverse proxy. Examples live in `deploy/examples/`, and installing
  them is in `deploy/README.md`; each operator keeps their real units privately.
- Schema migrations run once at startup. `/healthz` (up and reaching the database) backs
  the container's health check; `/version` reports the release and the schema version.
  Logs go to stdout/journald.
- The deployment runs released images and updates through `podman auto-update`; nothing
  is built on the server. Releasing and updating are in [`deployment.md`](deployment.md).

### 14.1 Secrets

**No secret ever appears in a unit file, an env file or a repo.** All secrets are **podman
secrets**, generated on the host once and handed to containers as files under
`/run/secrets/`.

| Secret | Consumers |
|---|---|
| `exchequer_db_password` | db (sets it), api (connects), backup (dumps) |
| `exchequer_bootstrap_token` | api (the first user, §10) |
| `exchequer_remote_ai_key` | api (optional: remote AI, §8) |
| `exchequer_tiingo_api_key` | api (optional: the Tiingo price source, §3.4); each keyed price source has its own, `exchequer_<source>_api_key` |

```bash
openssl rand -base64 32 | tr -d '\n' | podman secret create exchequer_db_password -
openssl rand -base64 32 | tr -d '\n' | podman secret create exchequer_bootstrap_token -
podman secret create exchequer_remote_ai_key -   # pasted once, reads stdin
podman secret create exchequer_tiingo_api_key -
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

# exchequer-server.container
[Container]
Secret=exchequer_db_password
Secret=exchequer_bootstrap_token
Environment=DB_HOST=exchequer-db DB_USER=exchequer DB_NAME=exchequer
Environment=DB_PASSWORD_FILE=/run/secrets/exchequer_db_password
Environment=TZ=America/New_York
```

Server requirements that follow:

- For each secret setting `X`, the server reads `X_FILE`, and **refuses to start** if a secret
  arrives as a plain `X` environment variable.
- Secrets are never logged or echoed by `/healthz` or `/version`, including in DSNs.
- Rotation: `podman secret rm` + `create`, `ALTER ROLE … PASSWORD`, restart.

### 14.2 Backups

A host systemd timer runs a oneshot `pg_dump` container to a local backup drive. It's a
scheduled job, but it belongs to host operations, not to the app.

- `Persistent=true`: a run missed while the machine slept fires on wake.
- `pg_dump -Fc`; the filename includes the timestamp and the `sqlx` migration version, so a
  restore can check compatibility. Keep the last N dumps.
- Restore is a documented admin command, never an API endpoint, and is tested in CI by
  restoring into a scratch database.

There's no offsite copy yet (#38).

## 15. Stack

**Rust server, Python CLI and migration tool, TypeScript web app.** The OpenAPI spec is the
boundary, so each side uses the language that suits it.

**Server (Rust):** a Cargo workspace.

| Crate | Contents | Key dependencies |
|---|---|---|
| `domain` | Commodities, accounts, splits, invariants, derived kinds, settlement math (§3, §4), `AccountNumber` (§3.6). Pure logic: no I/O, no async | `rust_decimal`, `bigdecimal`, `time` |
| `store` | Postgres access, compile-time-checked queries, migrations | `sqlx` |
| `import` | Parsers, dedup and transfer matching (`matching`), staging (§6) | `ofx-rs`, `csv`, `strsim`, `suggest` |
| `suggest` | The suggestion interface every level implements (`Suggester`, local `Method`), the stack that combines the levels, the backtest (§8) | — |
| `heuristics` | Local AI methods (`lexical-knn`), text normalization, calibration; the only crate where float lints are allowed | `suggest` |
| `rules` | Embedded Rhai engine: compile, validate, run the stored script as a `suggest::Suggester` (§8) | `rhai` (`no_float`, `only_i64`, `sync`), `suggest` |
| `api` | HTTP handlers, auth middleware, OpenAPI generation | `axum`, `tokio`, `utoipa`, `utoipa-axum`, `serde`, `tracing` |
| `remote` | Remote AI: the decision model as a `suggest::Suggester`, its questions and what it's shown (§8) | `outbound`, `suggest`, `heuristics` (normalization) |
| `outbound` | Outbound clients (the decision model, price sources); the only place data leaves the server | `ureq` (blocking, rustls with ring, bundled roots) |
| `server` | The binary: configuration, wiring, startup migrations | — |

- **OpenAPI:** `utoipa-axum` registers each route together with its spec entry. The spec is
  committed, and CI fails on drift.
- **Auth:** SHA-256 token hashes (`sha2`); passkeys with `webauthn-rs` (MPL-2.0, used
  unmodified), whose OpenSSL is compiled in statically.
- **Database:** PostgreSQL 17 (`NUMERIC`, `pg_trgm`); `sqlx` migrations compiled into the
  binary.
- **Container:** multi-stage build (`Containerfile`): the web app's static build, then one
  static binary, in a minimal image. A second, small image carries the backup script.
- **Tests:** `proptest` for invariants and settlement math, `sqlx` test databases, synthetic
  bank-file fixtures, API contract tests against the committed spec.

**CLI (Python 3.13):** Typer, on an SDK generated from the spec (`openapi-python-client`),
amounts as `decimal.Decimal`. Its packages are attached to each GitHub Release.

**Firefly migration (Python):** reads Firefly's API, writes only through Exchequer's API
(§11).

**Web app (TypeScript):** SvelteKit with the static adapter, a client generated from the spec
(`openapi-typescript`, `openapi-fetch`), and exact decimal string arithmetic
(`src/lib/money.ts`) for the rare client-side math.

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
│   ├── requirements.md
│   ├── deployment.md              # merging, releasing, deploying
│   ├── ai.md                      # suggestions: rules, local and remote AI
│   ├── ux.md                      # how the web app looks and behaves
│   └── issues.md                  # how issues are posted
├── api/
│   └── openapi.json               # committed; generated from the server
├── server/                        # Cargo workspace
│   ├── Cargo.toml                 # [workspace] + shared lints (float bans)
│   ├── rust-toolchain.toml
│   ├── clippy.toml                # disallowed-types = f32, f64
│   ├── crates/
│   │   ├── domain/  store/  import/  suggest/  heuristics/  rules/
│   │   ├── remote/  outbound/  api/  server/
│   │   └── ofx-check/             # dev tool, not shipped
│   └── migrations/                # sqlx SQL migrations
├── python/                        # uv workspace, one lockfile
│   ├── pyproject.toml             # members = sdk, cli, firefly-migrate
│   ├── sdk/                       # generated Python SDK (committed)
│   ├── cli/                       # `exchequer`: Typer
│   └── firefly-migrate/           # one-time migration tool
├── web/                           # npm: SvelteKit + generated TS client (committed)
├── Containerfile                  # web build → server build → minimal image
├── justfile                       # every check, locally and in CI
├── deploy/                        # backup image and script, restore, version, tests
│   └── examples/                  # sample quadlets
└── .github/workflows/             # checks (path-filtered: server, python, web); release
```

### 16.3 Conventions

- **The spec is generated, committed and checked for drift;** both SDKs are regenerated from
  it, and CI fails on any diff.
- **Generated SDKs are committed,** so API changes show up as readable diffs.
- **One container serves the API and the web app** (`/` and `/api/v1`): one origin, one
  image, one quadlet.
- **Every change reaches `main` through a pull request,** and **every merged pull request
  is a release** (details in [`deployment.md`](deployment.md)).
- **One version for everything** (server, web, SDK, CLI), and it lives only in git tags:
  builds take it from `git describe` (`deploy/version`; `hatch-vcs` for Python), so
  nothing in the repo is edited to release. They're built from one spec and move together.
  A release publishes both images to GHCR, public since they hold no household data, and a
  GitHub Release with the Python packages. The deployment runs the newest release and
  updates through `podman auto-update`; nothing is built on the server.
- **Real bank files never enter the repo, not even anonymized.** Test files are
  **synthetic**: written in the tests for the fictional household, reproducing each
  format's quirks (SGML vs XML OFX, QBO headers, CSV layouts). Tests on real files read from a local path given at
  runtime and run only on the operator's machine.
- **Keeping the repo clean** relies on conventions (fictional examples, synthetic fixtures)
  and GitHub secret scanning with push protection.
- **Tooling:** Rust: `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, `cargo-deny`.
  Python: ruff, ty, pytest, under uv. Web: prettier, eslint, `svelte-check`, vitest. The
  top-level `justfile` runs them (`just check`, `just python-check`, `just web-check`),
  and CI runs the same recipes.

### 16.4 License

**BSD-2-Clause.** Consequently, **no GPL dependencies**: every dependency is MIT,
Apache-2.0 or BSD, and CI checks licenses (`cargo-deny` for Rust, plus a check over the Python
and npm lockfiles). GPL-licensed OFX libraries (e.g. Python's `ofxtools`) are never
imported; `ofx-check` compares against the output of an external program instead. ISC
(TLS: `ring`, `rustls-webpki`) is allowed alongside them. The exceptions, each for named
crates (`server/deny.toml`): two Rhai can't do without, `smartstring` (MPL-2.0, copyleft
per file, used unmodified) and `tiny-keccak` (CC0-1.0); `webpki-roots`
(CDLA-Permissive-2.0, root certificates as data) for remote AI's TLS; and passkeys'
`webauthn-rs` and its own crates (MPL-2.0, used unmodified).

## 17. Not built

This document describes what exists. The original build order is complete except for
these, each an issue:

- The discrepancy finder's AI explanation (§7.1): #34.
- Bills and recurrences (§9): #35; learned bills: #36.
- Encrypted offsite backups (§14.2): #38.

New work is proposed and tracked as GitHub issues ([`issues.md`](issues.md)); this
document changes with the pull request that builds it.

## 18. Decisions

| # | Topic | Decision | Where |
|---|---|---|---|
| 1 | Securities | One account per institution holds many commodities; per-security accounts are collapsed on migration | §3.3, §11 |
| 2 | Outbound calls | Only because of a request (prices, remote AI), possibly after it answers; never on a timer | §2, §3.4 |
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
| 23 | PII at runtime | `AccountNumber` masking, no bodies in logs; remote AI gets features, not identifiers | §3.6 |
| 24 | Name | Exchequer: originally the treasury of the royal household | — |
| 25 | Price sources | Tiingo first, ECB reference rates as the FX backup; pluggable and configured, so either can be replaced. One price before a chain | §3.4 |
| 26 | Releases | Every merged pull request is a release; `main` takes no direct pushes | §16.3, `deployment.md` |

### Open

None.

