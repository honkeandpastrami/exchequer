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

Bills and tags are not suggested yet.

## The stack

Suggestions come from three levels, tried in order:

1. **Rules** — a Rhai script you write (below).
2. **Local AI** — similarity to your own labeled history, computed in the server
   (below).
3. **Remote AI** — a decision model on Azure AI Foundry. Optional. *Not built yet.*

A level runs while anything is still below the **confidence threshold, 0.9**. Labels
already settled — by an earlier level at 0.9 or more, or by you — are given to the next
level as facts, so it proposes the rest consistently with them. If no level reaches 0.9
for a label, the most confident suggestion any level made is kept, marked as unsure.

Suggestions are made when a file is uploaded: first the account of each statement that has
none, then every row's labels. Changing a statement's account suggests its rows' labels
again, with the new account as context. A label you set yourself is never overwritten: it's
given to every level as a fact.

Every level implements the same interface (`suggest::Suggester`): given rows, with the
labels already settled as facts, it proposes the others, each with a confidence and a
reason; given a statement's rows, it proposes an account. The stack only sees that
interface, so a level can be replaced without touching the rest.

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
        s.purpose(());              // none: the account's owner
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

A `file` has `name` (or `()`), `kind` (`"bank"`, `"credit_card"` or `()`) and `rows`.

**What `s` sets:** `s.counterparty("…")`, `s.category("…")` and `s.purpose("…")` (a
party), each by name or `()` for none, and `s.account("Owner · Name")`. The name alone is
enough for an account no other shares. If a setter is called twice, the last call wins.
Whatever is set is a suggestion at full confidence, by the rules level: its reason is
`rule v<version>:<line>`. Labels left unset go to local AI as usual, with the set ones as
facts.

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
  one of your accounts and the outside. It lists each one it would label differently from
  the ledger, or fails on. It stores nothing. In a dry run `row.raw` is empty, and
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
your accounts and the outside, which is what an imported row becomes. Adjustments, such as
opening balances, are left out. It runs in the server, in pure Rust, with no model files
and no training step to run: it's trained when needed, on the ledger as it is, and kept
until the ledger changes.

**Methods are interchangeable.** A local method is a `suggest::Method` in the
`heuristics` crate: it trains from the history into a suggester. The server runs one,
chosen by `EXCHEQUER_LOCAL_AI`:

| Value | Method |
|---|---|
| `lexical-knn-v1` (default) | Lexical nearest neighbors, below |
| `off` | No local AI |

A new method (or a new version of one) gets its own ID. Switching is a configuration
change, and its suggestions name it, so the two can be compared on what was committed
(`suggestion_outcome`) and measured on the same past by the backtest.

### `lexical-knn-v1`

1. **Normalization.** Bank texts are compared without what varies between rows of the
   same merchant: payment processors' prefixes (`SQ *`, `TST*`, …), card and reference
   numbers, store numbers, dates, digits and punctuation.
2. **Nearest neighbors.** The labeled transactions whose texts are most alike (character
   trigrams and words, TF-IDF, cosine similarity) vote on **the whole combination** of
   counterparty, category and purpose. Each vote is weighted by similarity and by context:
   the same account (else one with the same owner), the same direction, the same or a
   similar amount, and recency. A label's score is the weight agreeing with the winning
   combination, out of all of it plus a prior, so one lone neighbor never scores high.
3. **Alias memory.** Rows with exactly the same normalized text, on this account if it has
   any: how many of the latest five, in a row, had the same value.
4. **Facts.** Labels already settled for a row (by you, or by an earlier level) narrow the
   neighbors to transactions that agree with them; with a settled counterparty, all of its
   transactions are neighbors, however their text reads.
5. **The account.** For a statement without one, each row's neighbors vote for the
   accounts they're in (only accounts that aren't archived, and of the statement's kind
   when the file says: a credit card statement for a credit card).
6. **Calibration.** On training, the method suggests its latest 2,000 transactions again,
   each from only the ones before it, and each of the latest account-months as a
   statement, from the months before. Each score is then mapped to how often it was right
   (isotonic regression, smoothed so that thin evidence stays unsure). A confidence of 0.9
   means right about 9 times in 10 on this ledger's own data. With little history,
   nothing reaches 0.9.

Reasons read like *like 2026-03-12 “CORNER CAFE #” (6 of 7 similar)*, *the last 3 times
as “corner cafe”*, or, for an account, *12 of 15 rows look like its past transactions*.
The suggester's details record the data it learned from: a snapshot hash, the number of
transactions and the latest date.

### Backtest

`GET /suggesters/backtest?from=YYYY-MM&to=YYYY-MM[&method=…]` measures a method on the
ledger's own past. For each month, the method learns from everything before it, then
suggests labels for the month's transactions (their account known, nothing settled) and
an account for each account's transactions taken as one statement. Per label and for
accounts it reports how many were asked, guessed, right, confident, and right when
confident, plus guesses and right answers by confidence bin, which shows whether 0.9
really means 9 in 10. It reads only; nothing is stored. Any method can be named, so a
replacement can be compared with the current one before switching.

For trying a method on a copy of a ledger's history outside the server, the `heuristics`
crate has a dev tool reading the same history as a JSON file:
`cargo run --release -p heuristics --example backtest -- history.json 2026-01 2026-08`.

## What a suggestion records

Each suggestion keeps:

- the **value** (an account, counterparty, category or party — or "none");
- its **confidence**, stored per mille (an integer: 900 is 0.9);
- a short **reason** (e.g. the past transaction it resembles);
- **the exact system that made it**, a *suggester*: its kind (rule, local, remote), name
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
  *None* is a choice you can make explicitly: an unknown counterparty, no category, or
  the account's owner as the purpose.

## Commit

**Commit takes what's on screen:** your choices, and confident suggestions where you
made none. There's no separate step to accept them.

Commit is refused while information is missing:

- a statement has no account (neither its own nor a confident suggestion);
- a row that will become a new transaction has a label that is neither yours nor
  confidently suggested.

The web app disables *Commit* and lists the rows that need labels. The server enforces
the same rule (`409`, problem type `urn:exchequer:problem:labels-missing`, naming the
rows), so the command line leaves such an import staged, to finish in the web app.

On commit, every suggestion for a committed row, and every statement's suggested
account, is recorded with what was actually committed (`suggestion_outcome`). This
measures each suggester — how often a confident suggestion was kept, an old model
against its replacement — and doesn't depend on the ledger: it stays if the
transactions are later edited or undone.

## API

- `GET /imports/{id}`: each statement's `account_id` is the one the rows are checked
  against; `account_suggested` says it's a suggestion's; `suggested_account` is the
  suggestion, confident or not.
- `GET /imports/{id}/rows`: each row's `counterparty_id`, `category_id` and `purpose_id`
  are what a new transaction would get; `chosen` lists the labels you set;
  `suggestions` has at most one per label; `missing` lists, for a row that will be a new
  transaction, the labels commit still needs.
- `PATCH /imports/{id}/rows/{statement}/{position}`: setting a label (to a value or
  `null`) makes it yours; `"revert": ["category"]` hands it back to its suggestion.
- `PATCH /imports/{id}/statements/{position}` with an `account_id` makes it the
  statement's own; with `null`, it goes back to the suggestion, if any.
