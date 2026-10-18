# Exchequer — suggestions and AI

How Exchequer suggests what an imported bank row is, and how you review those
suggestions before anything enters the ledger. The design and its reasons are in
[`design.md` §8](design.md#8-component-classification-rules-and-ai); this doc describes
how it operates. It grows with the code: each part below says whether it's built yet.

## What gets suggested

When a bank file is staged (design §6), suggestions are made in two passes:

1. **The statement's account,** when the file doesn't settle it: a CSV names no account,
   and an OFX file may carry a number no account has yet.
2. **Each row's labels** — **counterparty**, **purpose** (who the money was for) and
   **category** — for every row that will become a new transaction. The three are
   suggested together, as one proposal per row, since they depend on each other: the
   counterparty mostly decides the category, and the two together with the account
   mostly decide the purpose. Each label still gets its own confidence.
3. **Each row's decision** — what committing does with it: add it as new, or keep it with
   what's already there. Matching suggests it (see [Matching](#matching)).

Bills and tags are not suggested yet.

## Transfers

A card payment shows up twice: leaving checking, and arriving on the card. It's one
transfer, never an expense plus an income.

- **The other side.** The counterparty label is really a row's *other side*: a
  counterparty, or **one of your accounts** — the row is then a transfer. Every level can
  suggest an account there, and you can choose one in review. A transfer has no category
  and no purpose (design §3.2), so once its other side is settled as an account, those
  are settled as none and not asked. Between a liquid and a non-liquid account it's an
  expense or income (design §3.2), which keeps them.
- **Potential transfer matches.** Each row comes with the rows of every staged import
  (this file's other statements, other files) that could be the other side of it: in
  another of your accounts, with the opposite amount in the same currency, dated within 5
  days. Every level sees them (`row.transfer_candidates` in rules); they're evidence, not a
  verdict.
- **Transfer match.** When a row is settled as a transfer to an account (yours, or a
  confident suggestion) and one of its potential transfer matches is in that account and
  open (or settled as the transfer back), the two are a transfer match: committed as one
  transfer, by whichever file is committed first, with both bank rows. The pairing is
  logic (`import::matching`): one-to-one, rows agreeing on both sides first, then the
  closest dates; merging them is matching's suggestion, as sure as the dates are close. A counterparty you chose, or a decision you made on a row, rules
  it out; *Keep separate transactions* splits a pair.
- **Transfer duplicate.** When the transfer is already in the ledger (committed from the
  other account), its other side is a transfer duplicate: kept with that transfer,
  nothing new created, unless you add it as new.
- One-sided transactions already in the ledger are never turned into transfers
  afterwards: a transfer is recorded as one when its first side is committed.

## Matching

Every row's decision is a suggestion, made by **matching**: logic, like the rules, and a
level of its own (kind `match`). It suggests the decision a row's status calls for, as
sure as what it found:

| Row | Suggests | Confidence |
|---|---|---|
| New: nothing like it | *Add as new* | 1.0 |
| Duplicate: the same bank row, imported before | *Already there* | 1.0 |
| Transfer duplicate or transfer match | *Already there* / *Merge into one transfer* | 1.0, less 0.03 a day apart |
| Near duplicate: the same amount a few days apart | *Already there* | the descriptions' likeness, less 0.06 a day apart; **never above 0.89** |

A near duplicate may be another, identical purchase, so it's never confident: it always
waits for your decision. Choosing a decision, even the suggested one, makes it yours. A
transfer match's two sides share one decision. Calibrating matching is a new version of
it (`matching` v1 now).

## The stack

Suggestions come from three levels, tried in order:

1. **Rules** — a Rhai script you write (below).
2. **Local AI** — similarity to your own labeled history, computed in the server
   (below).
3. **Remote AI** — a decision model on Azure AI Foundry. Optional (below).

A level runs while anything is still below the **confidence threshold, 0.9**. Labels
already settled — by an earlier level at 0.9 or more, or by you — are given to the next
level as facts, so it proposes the rest consistently with them. If no level reaches 0.9
for a label, the most confident suggestion any level made is kept, marked as unsure.

Suggestions are made when a file is uploaded: first the account of each statement that has
none, then every row's labels. Changing a statement's account suggests its rows' labels
again, with the new account as context. The whole stack runs again, for the labels that
aren't yours or confidently suggested, on every other staged import whenever a file is
uploaded or committed (its rows may be their potential transfer matches, and the ledger
the levels learn from may have changed), and on one import on request (`POST
/imports/{id}/refresh`, **Refresh suggestions** in review). A label you set yourself is
never overwritten: it's given to every level as a fact; so is a confident suggestion, when
suggesting again.

Every level implements the same interface (`suggest::Suggester`): given rows, with the
labels already settled as facts, it proposes the others, each with a confidence and a
reason; given a statement's rows, it proposes an account. The stack only sees that
interface, so a level can be replaced without touching the rest.

Each level's tunable parameters (thresholds, weights, how much it's shown) are
constants in its crate's `params` module: `suggest::params` (shared: the
threshold, the transfer window), `rules::params`, `heuristics::params` (local AI),
`remote::params`, `import::params` (matching), plus `api::params` (backtest and
dry-run sizes).

## Rules

Rules are one [Rhai](https://rhai.rs) script, stored in the database. Rhai is a small
scripting language for embedding in Rust, with Rust-like syntax. If/else is the rule
language: there's no rule table and no trigger/action DSL. The script starts empty. Write
rules where local AI is measurably wrong in a way a rule fixes, typically a purpose that
depends on the account, the amount or the date. The backtest and the dry run show where.

```rust
// Coffee on the household card is the household's, whoever buys it.
fn classify(row, s) {
    let d = row.description.to_upper();
    if d.contains("CORNER CAFE") && row.account == "Household · Joint Card" {
        s.counterparty("Corner Cafe");
        s.category("Dining");
        s.purpose("Household");
    } else if d.starts_with("ACME PAYROLL") && row.amount > amount("1000") {
        s.counterparty("Acme");
        s.category("Salary");
        s.purpose("Alex");
    }
}

// Files from the card's site are named card-2026-03.csv.
fn account(file, s) {
    if file.name != () && file.name.starts_with("card-") {
        s.account("Household · Joint Card");
    }
}
```

**Entry points,** both optional:

- `fn classify(row, s)` runs for each bank row being imported.
- `fn account(file, s)` runs for each statement whose file doesn't say which account it is
  for.

**What a row has:**

| Field | |
|---|---|
| `description` | The bank's text, as is |
| `amount` | An amount: positive into the account. Compare it with `amount("12.50")` or a whole number (`row.amount < 0`); `abs()`, `+` and `-` work. There are no floats. |
| `date`, `year`, `month`, `day`, `weekday` | `"2026-03-02"`; numbers; weekday 1 (Monday) to 7 |
| `account`, `account_name`, `owner` | `"Owner · Name"`, the name alone, the owner's name; `()` when the statement has no account yet |
| `raw` | The bank file's own fields, by name, as text (e.g. `row.raw.Memo`) |
| `transfer_candidates` | Its potential transfer matches, closest first, each with `account` (`"Owner · Name"`), `account_name`, `owner`, `date` and `days` apart |

A `file` has `name` (or `()`), `kind` (`"bank"`, `"credit_card"` or `()`) and `rows`.

**What `s` sets:** `s.counterparty("…")`, by name or `()` for none; `s.category("…")` and
`s.purpose("…")`, a category and a party by name (never none: design §4.1);
`s.transfer("Owner · Name")`, the other side as one of your accounts (a transfer); and
`s.account("Owner · Name")`. The name alone is
enough for an account no other shares. If a setter is called twice, the last call wins.
Whatever is set is a suggestion at full confidence, by the rules level: its reason is
`rule v<version>:<line>`. Labels left unset go to local AI as usual, with the set ones as
facts.

For example, a card payment, when the card's side of it is staged:

```rust
fn classify(row, s) {
    for c in row.transfer_candidates {
        if c.account == "Household · Joint Card" && row.description.contains("PAYMENT") {
            s.transfer("Household · Joint Card");
        }
    }
}
```

**Validation.** `PUT /rules` refuses a script with each error and its line (`422`,
`rules-invalid`, `errors: [{line, column, message}]`), and it never becomes active:

- syntax errors, undefined variables, an entry point with other parameters;
- **every name it uses:** in setters, and in comparisons of `row.account`,
  `row.account_name` or `row.owner` with text. Setters take names written out
  (`s.category("Dining")`, not a variable), so they can all be checked.

A valid script is active at once. If a name it uses is deleted later, the script stops
validating, and rules sit out (local AI still suggests) until it's fixed.

**Sandboxed.** A script can't reach files, the network or other processes, and has no
`eval`. A call that runs too long (a loop) is stopped: that row simply gets no rule
suggestions.

**Versions.** Every upload is a new version, and none is ever edited or deleted. Each
records who uploaded it, when, and an optional note. The newest is active.

- `GET /rules`: the active script (version 0, empty, when there's none). Its ETag goes in
  `If-Match` for the next `PUT /rules` (`{ "source", "note" }`), so two edits can't
  overwrite each other.
- `GET /rules/versions`, `GET /rules/versions/{n}`.
- `POST /rules/dry-run?from=&to=` with `{}` (the active script) or `{ "source": … }` (a
  candidate) runs `classify` over the ledger's transactions in the range, those between
  one of your accounts and the outside, and transfers (once from each side, with the
  other side as a potential transfer match). It lists each one it would label differently
  from the ledger, or fails on. It stores nothing. In a dry run `row.raw` is empty, and
  `account` doesn't run.

On the command line:

```sh
exchequer rules edit -m "why"     # $EDITOR on the active script; a refused one reopens at its error
exchequer rules upload FILE -m "why"
exchequer rules show [N]          # a version's script
exchequer rules log               # versions, newest first
exchequer rules diff 3 [4]        # what changed (against the active one by default)
exchequer rules rollback 3        # v3's script, as a new version
exchequer rules dry-run --month 2026-09 [--file candidate.rhai]
```

## Local AI

Local AI learns from the ledger itself: every transaction with one split between one of
your accounts and the outside, which is what an imported row becomes, and every one-split
transfer between two of your accounts, once from each side (its other side is the other
account). Adjustments, such as opening balances, are left out. It runs in the server, in pure Rust, with no model files
and no training step to run: it's trained when needed, on the ledger as it is, and kept
until the ledger changes.

**Methods are interchangeable.** A local method is a `suggest::Method` in the
`heuristics` crate: it trains from the history into a suggester. The server runs one,
chosen by `EXCHEQUER_LOCAL_AI`:

| Value | Method |
|---|---|
| `lexical-knn-v2` (default) | Lexical nearest neighbors, below |
| `off` | No local AI |

A new method (or a new version of one) gets its own ID. Switching is a configuration
change, and its suggestions name it, so the two can be compared on what was committed
(`suggestion_outcome`) and measured on the same past by the backtest.

### `lexical-knn-v2`

Version 2 learns transfers (version 1 didn't see them).

1. **Normalization.** Bank texts are compared without what varies between rows of the
   same merchant: payment processors' prefixes (`SQ *`, `TST*`, …), card and reference
   numbers, store numbers, dates, digits and punctuation.
2. **Nearest neighbors.** The labeled transactions whose texts are most alike (character
   trigrams and words, TF-IDF, cosine similarity) vote on **the whole combination** of
   counterparty, category and purpose. Each vote is weighted by similarity and by context:
   the same account (else one with the same owner), the same direction, the same or a
   similar amount, and recency; a past transfer to an account where the row has a
   potential transfer match counts double. A label's score is the weight agreeing with the winning
   combination, out of all of it plus a prior, so one lone neighbor never scores high.
3. **Alias memory.** Rows with exactly the same normalized text, on this account if it has
   any: how many of the latest five, in a row, had the same value.
4. **Facts.** Labels already settled for a row (by you, or by an earlier level) narrow the
   neighbors to transactions that agree with them; with a settled counterparty, all of its
   transactions are neighbors, however their text reads.
5. **The account.** For a statement without one, each row's neighbors vote for the
   accounts they're in (only accounts that aren't archived, and of the statement's kind
   when the file says: a credit card statement for a credit card). Transfers don't vote:
   a transfer's text is one bank's, not necessarily this account's.
6. **Calibration.** On training, the method suggests its latest 2,000 transactions again,
   each from only the ones before it, and each of the latest account-months as a
   statement, from the months before. Each score is then mapped to how often it was right
   (isotonic regression, smoothed so that thin evidence stays unsure). A confidence of 0.9
   means right about 9 times in 10 on this ledger's own data. With little history,
   nothing reaches 0.9. A transfer's potential transfer matches are worked out from the
   history as if its rows had been staged together; its category and purpose aren't
   asked.

Reasons read like *like 2026-03-12 “CORNER CAFE #” (6 of 7 similar)*, *the last 3 times
as “corner cafe”*, or, for an account, *12 of 15 rows look like its past transactions*.
The suggester's details record the data it learned from: a snapshot hash, the number of
transactions and the latest date.

### Backtest

`GET /suggesters/backtest?from=YYYY-MM&to=YYYY-MM[&method=…][&remote=true]` measures a
method on the ledger's own past. For each month, the method learns from everything before it, then
suggests labels for the month's transactions (their account known, nothing settled, the
month's other transactions as potential transfer matches) and an account for each
account's transactions taken as one statement. A transfer is scored on its other side
only. Per label and for
accounts it reports how many were asked, guessed, right, confident, and right when
confident, plus guesses and right answers by confidence bin, which shows whether 0.9
really means 9 in 10. Each score also has the usual measures (samples, coverage,
precision, recall, F1, accuracy, and their macro means over values), over all guesses and
over confident ones only, and the response totals them over all months. It reads only; nothing is stored. Any method can be named, so a
replacement can be compared with the current one before switching.

With `remote=true` (when remote AI is configured, and for at most 3 months), remote AI is
asked too, as on upload: shown only what's before the month, about what the method left
unsure. The scores are then the final suggestions', and `levels` says, per level, how
many of them it made and how right they were. Its requests are real ones, and billed.

For trying a method on a copy of a ledger's history outside the server, the `heuristics`
crate has a dev tool reading the same history as a JSON file:
`cargo run --release -p heuristics --example backtest -- history.json 2026-01 2026-08`
(`BACKTEST_DUMP=out.json` also writes each row's guesses).

## Remote AI

Optional: a **decision model** deployed on Azure AI Foundry, such as Microsoft-Decision-1.
A decision model generates no text: it's given a `state` (JSON) and named multiple-choice
questions, and answers each with one of the options and a probability for every option.
That fits the problem: pick a label among the ledger's own values, with a confidence the
0.9 threshold can use directly.

It's the last level: asked only about what rules and local AI left unsure, with what they
settled as facts. Without it configured, or when the service fails, everything else works
as before.

**The questions** (version `q2`; `q1` didn't offer transfers). One request per row, with a
question per open label, sharing one state; one request per statement without an account.

| Question | Options |
|---|---|
| `counterparty` | The row's other side: the accounts of its potential transfer matches (a transfer), then the other sides of its similar past transactions and of those with the same text, at most 10 — never the whole list — plus *none of these* and *cannot tell* |
| `category` | Every category, plus *cannot tell* |
| `purpose` | Every party, plus *cannot tell* |
| `account` | Every account that isn't archived and fits the file's kind, plus *none of these* and *cannot tell* |

Each option is a name and a description computed from the data on each request (a
counterparty's recent bank text, usual category, purpose and amount, and how often it
appears; a category's usual counterparties, amounts and purposes; a party's kind and usual
categories; an account's kind, owner, last four digits, recent bank texts and latest
transaction), so nothing goes stale. Options are shuffled (order sways the model), the same
way for the same row. *Cannot tell* and *none of these* give no suggestion, so the model is
never forced to choose; a new counterparty is never suggested, since it can't be offered.
An answer that isn't one of the options is dropped.

The wording (each question's instructions, how descriptions are phrased, the names of the
fixed options) is in Jinja templates in
[`server/crates/remote/resources/`](../server/crates/remote/resources/), built into the
server; the code only computes what fills them.

**What it's shown,** per row:

- the transaction: its bank text normalized, date and weekday, amount and direction, the
  bank's own fields saying what kind of transaction it is (a `Type` or `Category` column,
  a merchant category code), whether the same amount recurred in recent months, and the
  accounts where the same amount moved the other way (its potential transfer matches);
- its account: name, kind, owner, and its purposes and top categories over six months;
- the labels already settled, as facts;
- the 10 most similar past transactions, from local AI, with their labels.

For a statement: its file name, the last four digits of the number it names, its kind,
dates and a few rows.

**Never an identifier.** Bank texts and file names are normalized, so runs of five or more
digits (card, reference and trace numbers) are replaced with `#`; account numbers go only
as their last four digits; no other bank field is sent. A request the service refuses (`400`/`422`, e.g. a
content filter) makes remote AI step aside for that row: it's logged (the status and the
service's error code, never the request), and the import goes on. Any other failure (a bad
key, a wrong endpoint, the service unreachable, still busy after retries) stops remote AI
for the rest of that upload, logged once.

**Suggestions** name the model as the service reports it and the questions' version
(`microsoft-decision-1 q2`); the details record the provider and deployment. The reason is
the model and its probability (*microsoft-decision-1: 0.94*). A new wording is a new
version: measure it with the backtest before using it.

**Configuration** (all three, or none):

| Setting | |
|---|---|
| `EXCHEQUER_REMOTE_AI_ENDPOINT` | The Foundry resource's endpoint, `https://<resource>.services.ai.azure.com` |
| `EXCHEQUER_REMOTE_AI_DEPLOYMENT` | The model's deployment name |
| `REMOTE_AI_KEY_FILE` | A file holding the resource's key: a podman secret (`exchequer_remote_ai_key`) |
| `EXCHEQUER_REMOTE_AI_CONCURRENCY` | Requests at once, default 4 (1–32) |

Requests go to `<endpoint>/providers/microsoft/v1/systemone` over HTTPS, with the key in
the `api-key` header, 30 seconds each at most; a busy service (`429`, `5xx`) is retried
twice, after what `Retry-After` says (at most 10 seconds). Remote AI runs while a file is uploaded (or a statement's
account is changed), so a large statement takes a while: one request per unsure row. A
deployment type that keeps inference within a data zone, rather than a global one, keeps
the data in that zone.

## What a suggestion records

Each suggestion keeps:

- the **value** (an account, counterparty, category or party — or "none");
- its **confidence**, stored per mille (an integer: 900 is 0.9);
- a short **reason** (e.g. the past transaction it resembles);
- **the exact system that made it**, a *suggester*: its kind (rule, local, remote, match), name
  and version, plus whatever else identifies it — the rules' version, the data a local
  model learned from, the remote model's version as the service reports it and the
  version of the questions asked. When a model is upgraded or replaced, every suggestion
  still says which one made it.

## Review

In the web app's import review, every suggested value carries a badge: one color per
level, **solid when confident** (0.9 or more), **outlined when not**. Your own choices
carry none, so "the AI said so" and "I said so" always look different. Hovering a badge
shows the suggester and the reason.

- **A statement's account.** A confident suggestion counts as the account: the rows are
  checked against it (duplicates, transfer matches). *Accept* makes it the statement's
  own, and then offers to remember the file's number for it. An unsure suggestion is
  shown with *Use it*; the rows wait until an account is picked.
- **A row's labels.** Each new row shows its counterparty, category and purpose, with
  their badges. In its details, you can change any of them (the value becomes yours),
  accept an unsure suggestion (it becomes yours too), or go back to the suggestion.
  *None* is a choice you can make explicitly for the counterparty: an unknown one. A
  category always names something and a purpose someone (design §4.1). The counterparty can be one of your accounts
  (typed by its `Owner · Name`): the row is then a transfer, and its category and purpose
  show *Not for a transfer*.
- **Transfers.** A transfer match names its other side (account, date, text, and file if
  it's another), linked to that row, and committing makes the two one transfer (*Merge
  into one transfer*); *Keep separate transactions* splits them. A transfer duplicate names the transfer it's the other side of.
- **A row's decision.** *On commit* shows matching's suggestion with its badge and reason.
  A confident one is selected; an unsure one (every near duplicate) asks *Choose…*, with
  the suggestion marked. Picking any option makes it yours.

## Commit

**Commit takes what's on screen:** your choices, and confident suggestions where you
made none. There's no separate step to accept them.

Commit is refused while information is missing:

- a statement has no account (neither its own nor a confident suggestion);
- a row that will become a new transaction has a label that is neither yours nor
  confidently suggested (a row split into parts has none of its own: the parts' labels
  are all yours, and its suggestions don't apply);
- a row's decision is neither yours nor confidently suggested.

The web app marks such an import *Needs work*, disables *Commit* and links the rows that
need labels or a decision. The server enforces the same rule (`409`, problem types
`urn:exchequer:problem:labels-missing` and `…:decisions-missing`, naming the rows), so
the command line leaves such an import staged, to finish in the web app.

On commit, every suggestion for a committed row (but a split one's labels), its decision included, and every
statement's suggested account, is recorded with what was actually committed (`suggestion_outcome`). This
measures each suggester — how often a confident suggestion was kept, an old model
against its replacement — and doesn't depend on the ledger: it stays if the
transactions are later edited or undone.

## API

- `GET /imports/{id}`: each statement's `account_id` is the one the rows are checked
  against; `account_suggested` says it's a suggestion's; `suggested_account` is the
  suggestion, confident or not.
- `GET /imports/{id}/rows`: each row's `counterparty_id` (or `transfer_account_id`, for
  a transfer), `category_id` and `purpose_id` are what a new transaction would get;
  `chosen` lists the labels you set; `suggestions` has at most one per label (a
  suggested transfer has `transfer_account_id`); `missing` lists, for a row that will be
  a new transaction, the labels commit still needs; `not_applicable` the labels a
  transfer doesn't have; `transfer_match` is a transfer match's other side.
- `PATCH /imports/{id}/rows/{statement}/{position}`: setting a label (to a value or
  `null`) makes it yours; `transfer_account_id` sets the other side to one of your
  accounts (the counterparty label, yours); `"revert": ["category"]` hands it back to its
  suggestion. `"decision": "new"` on a transfer match keeps its two sides separate.
  `"splits": [...]` splits a row added as new into parts (two or more, adding up to its
  amount), each with its own other side, category, purpose and note; `null` unsplits it.
- `PATCH /imports/{id}/statements/{position}` with an `account_id` makes it the
  statement's own; with `null`, it goes back to the suggestion, if any.
