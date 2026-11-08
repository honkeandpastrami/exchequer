# Exchequer — suggestions and AI

What suggestions do, as seen from a client or a reviewer: given these rows, this ledger and
these choices, what each row is shown as and why. Why it works this way is in
[design §8](design.md#8-component-classification-rules-and-ai); what matching finds and how
review and commit treat a row are in [`import.md`](import.md).

> **How this doc is written.** Each rule appears once.
>
> **In:** behaviour visible at the API, the UI or a rules script, as rules (*when X, Y*), one
> sentence each, present tense; the concepts, defined once in the table below, in design
> §8's terms, no synonyms; tables for anything that enumerates; what's refused, and when.
>
> **Out:** why (design §8: link it); endpoints, fields and parameters (the spec: name the
> endpoint at most); constant values (link the constant); step-by-step algorithms (the
> source: state the rule they guarantee); history ("now", "was", versions, "not built
> yet"); screen wording and layout (`ux.md`, the source); anything already in `design.md` or
> `import.md` (link it); more than one example per rule.
>
> **Shape:** sections in design §8's order: what's suggested, transfers, matching, the stack,
> what a suggestion records, review and commit, then one section per level. Each opens with
> a one-line job, then its rules. Budget: about 20 lines a section, 40 for a level's
> section with its tables, 250 for the doc. Over budget means detail crept in: cut, don't
> make room.

## Concepts

| Term | Meaning |
|---|---|
| **Label** (to suggest) | A statement's account, or a row's counterparty, purpose or category |
| **Other side** | A row's counterparty label: a counterparty, none, or one of your accounts (a transfer) |
| **Potential transfer match** | A staged row that could be a row's other side ([`import.md` §6.4](import.md#64-transfers)) |
| **Suggestion** | A proposed value for a label, with a confidence and a reason, made by a suggester |
| **Suggester** | The exact system that made a suggestion: its level, name, version and what identifies it |
| **Level** | Where a suggestion comes from: `rule`, `local`, `remote`, or `match` (matching) |
| **Confident** | At or above [`CONFIDENT`](../server/crates/suggest/src/params.rs) |
| **Certain** | At [`CERTAIN`](../server/crates/suggest/src/params.rs): short of it, a row or statement is `to_review` |
| **Fact** | A label already settled for a row, by you or by a confident suggestion, given to a level |

## What's suggested

Job: every label commit needs has a value before you look at it.

- A statement's account is suggested when the file doesn't settle it: a CSV names no
  account, and an OFX file may carry a number no account has yet.
- Each row that may be added gets its counterparty, purpose and category as one proposal,
  each label with its own confidence.
- Each row's choice is suggested by [matching](#matching).
- Bills and tags aren't suggested.

## Transfers

Job: a level can say a row is a transfer, the same way it says anything else.

- Every level may suggest one of your accounts as a row's other side, and every level sees
  the row's potential transfer matches.
- Once a row's other side is settled as an account, its category and purpose are settled as
  none and not asked (design §3.2); between a liquid and a non-liquid account, they're
  asked as usual.
- Whether two staged rows are one transfer is matching's, not a suggestion
  ([`import.md` §6.4](import.md#64-transfers)): suggestions never prevent a pair, and a pair
  added as one overrides what was suggested for either side.

## Matching

Job: each row's default choice is a suggestion, as sure as what matching found.

| Row | Suggests | Confidence |
|---|---|---|
| `new` | add | [`SURE`](../server/crates/bank-import/src/params.rs) |
| `duplicate`: the bank row, unchanged | don't add | [`SURE`](../server/crates/bank-import/src/params.rs) |
| `possible_duplicate`: the bank row, changed since | don't add | [`CHANGED`](../server/crates/bank-import/src/params.rs) |
| `transfer_pair`, or a recorded transfer | add (one transfer) · don't add | [`SURE`](../server/crates/bank-import/src/params.rs), less [`TRANSFER_DAY`](../server/crates/bank-import/src/params.rs) a day apart |
| `possible_duplicate`: the same amount days apart | don't add | the descriptions' likeness, less [`NEAR_DAY`](../server/crates/bank-import/src/params.rs) a day apart, at most [`NEAR_MOST`](../server/crates/bank-import/src/params.rs) |
| `transfer_pair` your choices disagree with | add separately | [`DISAGREES`](../server/crates/bank-import/src/params.rs) |

- Matching's suggestions name it as their suggester, like any level's.
- A new calibration of matching is a new version of its suggester
  ([`MATCHER`](../server/crates/bank-import/src/params.rs)).

## The stack

Job: each label gets the most confident value the levels can give, asking the costliest last.

- The levels run in order, rules, local AI, remote AI, each only while some label is short
  of confident; matching runs on its own.
- Each level gets the labels already settled as facts, and proposes the rest consistently
  with them.
- When no level is confident about a label, the most confident suggestion any level made is
  kept, as unsure.
- A label you set is never overwritten, and a confident suggestion stays when suggesting
  again.
- On upload, statements' accounts are suggested first, then every row's labels.
- Changing a statement's account suggests its rows' labels again, with the new account as
  context.
- Uploading or committing a file suggests again on every other staged import; refreshing an
  import suggests again on it.
- Each level's parameters are constants in its crate's `params` module; the shared ones
  (the threshold, the transfer window) are in [`suggest::params`](../server/crates/suggest/src/params.rs).

## What a suggestion records

Job: every suggestion says which system made it, so models can be measured and compared.

- A suggestion keeps its value (or none), its confidence, a short reason, and its
  suggester.
- A suggester identifies the exact system: the rules' version, the data a local method
  learned from, the remote model as the service reports it and the questions' version.
- On commit, every suggestion for a committed row (its choice included), and every
  statement's suggested account, is recorded with what was committed.
- That record stays when the transactions are later edited or undone.

## Review and commit

Job: you see which values are suggestions, and change only what you disagree with.

- Every suggested value shows its level and whether it's confident, and on request its
  suggester and reason; values you set show none of that.
- A suggestion is the value shown, never an extra option to pick.
- A confident statement account counts as the account: the rows are matched against it. An
  unsure one is shown, but the rows wait until an account is picked or the suggestion is
  ticked reviewed.
- Picking or confirming a statement's account remembers the file's account number for it.
- Setting a label makes it yours; reverting it hands it back to its suggestion. Saving a
  row's form makes every label it shows yours.
- No counterparty is a choice you can make; a category and a purpose always name one
  (design §4.1).
- A row split into parts has no suggestions of its own: the parts' labels are all yours.
- Commit takes suggestions where you made no choice, with no step to accept them; states,
  ticks and refusals are in [`import.md`](import.md#66-review).

## Rules

Job: the household writes, in a [Rhai](https://rhai.rs) script, what local AI gets wrong.

- The script starts empty, and may define `fn classify(row, s)`, run for each row, and
  `fn account(file, s)`, run for each statement without an account.
- Whatever it sets is a suggestion at [`CONFIDENCE`](../server/crates/suggest-rules/src/params.rs),
  whose reason names the script's version and the line; a setter called twice keeps the last
  call.
- An upload is refused, with each error's line, for syntax errors, undefined variables, an
  entry point with other parameters, or a name that doesn't exist; names must be written
  out in setters and in comparisons of `row.account`, `row.account_name` or `row.owner`.
- A valid upload is active at once; when a name it uses is deleted later, rules sit out
  until the script is fixed.
- A script can't reach files, the network or other processes; a call that runs too long
  gives that row no rule suggestions.
- Every upload is a new version, with who, when and a note; none is edited or deleted, and
  an edit based on an older version is refused.
- A dry run lists the ledger transactions in a date range that the script would label
  differently, or fails on, and stores nothing; in it, `row.raw` is empty and `account`
  doesn't run. `exchequer rules` edits, versions and dry-runs the script.

| A `row` has | |
|---|---|
| `description` | The bank's text, as is |
| `amount` | Positive into the account; compared with `amount("12.50")` or a whole number, never a float |
| `date`, `year`, `month`, `day`, `weekday` | `"2026-03-02"`; numbers, weekday 1 (Monday) to 7 |
| `account`, `account_name`, `owner` | `"Owner · Name"`, the name, the owner; `()` without an account |
| `raw` | The file's own fields, by name, as text |
| `transfer_candidates` | Its potential transfer matches, closest first: `account`, `account_name`, `owner`, `date`, `days` |

A `file` has `name` (or `()`), `kind` (`"bank"`, `"credit_card"` or `()`) and `rows`.

| `s` sets | |
|---|---|
| `s.counterparty("…")` | The counterparty by name, or `()` for none |
| `s.category("…")`, `s.purpose("…")` | A category, a party, by name |
| `s.transfer("Owner · Name")` | The other side as one of your accounts |
| `s.account("Owner · Name")` | The statement's account; the name alone when no other account shares it |

## Local AI

Job: suggest from the ledger's own past, with a confidence that means what it says.

- Local AI learns from every transaction with one split between one of your accounts and
  the outside, and every one-split transfer between two of yours, from each side;
  adjustments are left out.
- It's trained on the ledger as it is when needed, and kept until the ledger changes.
- The server runs one method, chosen by `EXCHEQUER_LOCAL_AI` (the newest by default,
  [`config.rs`](../server/crates/server/src/config.rs)), or none (`off`).
- A method's suggestions name it and its version, so methods can be compared on what was
  committed and on the same past.
- A confidence of *p* means right about a fraction *p* of the time on this ledger's
  history; with little history, nothing is confident.
- Facts narrow what it learns from to transactions that agree with them.
- For a statement's account, it votes only with accounts that aren't archived and fit the
  file's kind; transfers don't vote.
- Its reasons name what it resembles: a past transaction, the last times the same text was
  seen, or how many rows look like an account's.

**Backtest:** for each month of a range, a method learns from everything before it and
suggests the month's labels and accounts; it reports per label how often guesses and
confident guesses were right, and by confidence, whether *p* holds. It stores nothing, any
method can be named, and with remote AI configured it can include remote AI (billed, for at
most [`BACKTEST_REMOTE_MONTHS`](../server/crates/api/src/params.rs)). Outside the server,
the [backtest example](../server/crates/suggest-local-ai/examples/backtest.rs) runs the same
on a history file.

## Remote AI

Job: a decision model on Azure AI Foundry answers what the other levels left unsure.

- Remote AI is optional: configured, it's asked last, only about labels still short of
  confident, with the others as facts; without it, or when it fails, everything else works.
- A row is one request, with a question per open label; a statement without an account is
  one request.
- Each answer is one of the options with a probability, which is the confidence; an answer
  that isn't an option is dropped.
- *Cannot tell* and *none of these* give no suggestion, and a new counterparty is never
  suggested.
- Options and their descriptions are computed from the ledger on each request; the wording
  is in [templates](../server/crates/suggest-remote-ai/resources/).
- It's shown the row, its account, the facts and the most similar past transactions, never
  an identifier (design §19.2).
- A request the service refuses makes remote AI step aside for that row; any other failure
  stops it for the rest of that upload. Either is logged without the request.
- A new wording is a new questions version, measured with the backtest before use.
- A data-zone deployment keeps the data in that zone.

| Question | Options |
|---|---|
| `counterparty` | Its potential transfer matches' accounts, then the other sides of similar past transactions, at most [`CANDIDATES`](../server/crates/suggest-remote-ai/src/params.rs); *none of these*; *cannot tell* |
| `category` | Every category; *cannot tell* |
| `purpose` | Every party; *cannot tell* |
| `account` | Every account that isn't archived and fits the file's kind; *none of these*; *cannot tell* |

| Setting (all of the first three, or none) | |
|---|---|
| `EXCHEQUER_REMOTE_AI_ENDPOINT` | The Foundry resource's endpoint |
| `EXCHEQUER_REMOTE_AI_DEPLOYMENT` | The model's deployment name |
| `REMOTE_AI_KEY_FILE` | A file holding the resource's key (podman secret `exchequer_remote_ai_key`) |
| `EXCHEQUER_REMOTE_AI_CONCURRENCY` | Requests at once ([`config.rs`](../server/crates/server/src/config.rs)) |
