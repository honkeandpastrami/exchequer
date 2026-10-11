# Exchequer — requirements

Exchequer is a self-hosted personal and household accounting app. It is built for households
that bank in the US, where banks offer no usable APIs and export only CSV and OFX/QFX/QBO
files. It also replaces Firefly III for people who need the features below.

## What to keep from Firefly III

- The separation between asset accounts and the transactions that move value between them.
- Classification by category, source/destination and tags.
- Rich reports.
- Every figure is computed on the fly: editing one transaction updates every balance and
  report that depends on it.
- A solid relational database underneath.
- A proper REST API.

## What to fix

- **No browser lock-in.** The server is a REST API; any client that speaks it is
  first-class (the Transmission model). A web client is welcome but not the only option.
- **Simple deployment.** A small server plus a database, running as user-level containers
  (podman), without a large dependency stack.
- **US bank exports are the primary input.** Import poor-quality CSV and OFX/QFX/QBO files
  directly. Parsing, account detection and duplicate detection belong to the server, not to
  a separate pre-processing tool.
- **No scheduled jobs.** The server may sleep most of the time. It does nothing unless a
  request arrives: no cron, no timers, no background workers. Work that needs the outside
  world (e.g. exchange rates) happens on demand.
- **Multi-commodity accounts.** One account holds several currencies and securities (a
  brokerage with several funds is one account, not one account per security). Selling a
  security for cash and converting currencies are ordinary transactions. Values can be
  converted to any currency on demand for display.

## The core use case: household settlement

- Accounts are either personal (owned by one person, or by a business) or shared (owned by
  the household).
- Each month, the household reviews all income and expenses and labels each one by
  **purpose**: personal, household, or business.
- Household income pays household expenses and a monthly **stipend** to each person.
  Personal expenses come out of each person's stipend.
- The monthly result is a set of transfers: each person (and each business) either deposits
  into shared savings or draws from it.
- **Owners** (account → person, household or business) are set once. **Purposes** are the
  manual part, so the app must make them fast: simple rules written as code, plus
  suggestions learned from history and from an LLM.

## Other requirements

- **AI assistance where it is cheap and useful:** auto-categorization, auto-destination,
  account detection for an uploaded file, and explaining balance discrepancies. The app must
  work fully without it.
- **Exact arithmetic.** No floating point for money, quantities, prices or rates, anywhere.
- **Bills that start and stop.** Recurring expectations have a lifecycle and can be learned
  from history.
- **One-time migration from Firefly III,** at least one calendar year of history.
- **Authentication:** it runs on a private network behind a VPN, but still needs real auth:
  at minimum static tokens, preferably sign-in through a reputable identity provider.
- **Privacy:** household data never leaves the server except where a feature needs it
  (e.g. an LLM call), and then only the minimum, never into logs.
