# Exchequer — import

What import does, as seen from a client or a reviewer: given these files and these choices,
what ends up in the ledger. Why it works this way is in
[design §6](design.md#6-component-import-pipeline); suggestions are in [`ai.md`](ai.md).

> **How this doc is written.** Each rule appears once.
>
> **In:** behaviour visible at the API or the UI, as rules (*when X, Y*), one sentence each,
> present tense; the concepts, defined once in the table below, in design §6's terms, no
> synonyms; tables for anything that enumerates; what's refused, and when.
>
> **Out:** why (design §6: link it); endpoints, fields and parameters (the spec: name the
> endpoint at most); constant values (link the constant); step-by-step algorithms (the
> source: state the rule they guarantee); history ("now", "was", versions, "not built
> yet"); screen wording and layout (`ux.md`, the source); anything already in `design.md` or
> `ai.md` (link it); more than one example per rule.
>
> **Shape:** sections in pipeline order, numbered like design §6.x; each opens with a
> one-line job, then its rules. Budget: about 30 lines a section, 200 for the doc. Over
> budget means detail crept in: cut, don't make room.

## Concepts

| Term | Meaning |
|---|---|
| **Staged import** | One uploaded file on the server, waiting for commit: one or more statements |
| **Statement** | The rows of one account in a file, with that account (or none yet) |
| **Row** | One bank line, as the file had it, plus what matching and suggestions found |
| **Label** (of a row) | What matching found: `new`, `duplicate`, `possible_duplicate`, `transfer_pair`, `no_account` |
| **Choice** | What committing does with a row: add, add separately (a transfer pair only), don't add |
| **Default** | The choice a row takes until you make one, with a confidence |
| **Other side** | A row's counterparty, or one of your accounts when it's a transfer |
| **State** | A row's colour: `not_added`, `needs_you`, `to_review`, `ready` |
| **Reviewed** | Your tick on a row: you read what it shows and agree |
| **Readiness** | How close a staged import is to committing |
| **Import batch** | The transactions one commit made; undone as one |

## 6.3 Duplicates

Job: a bank row already in the ledger is never added twice.

- When a row's bank ID (or, without one, its content and its occurrence in the file) is
  already recorded for its account, and the row is unchanged since, it's a `duplicate`.
- When the ID is recorded but the row has changed since, it's a `possible_duplicate` of that
  transaction.
- When a ledger transaction in the same account has the same commodity and signed amount
  within [`NEAR_DAYS`](../server/crates/bank-import/src/params.rs), and no bank row of its
  own, the row is a `possible_duplicate` of it. Descriptions only rank candidates; a low
  likeness never clears one.
- Each ledger transaction matches at most one row, and each row at most one transaction.
- Uploading the same file again, or files whose dates overlap, adds nothing.
- Transactions entered another way (by hand, by a migration) can have their bank IDs
  recorded, so later files recognize them.
- A `duplicate` or `possible_duplicate` that you add anyway is recorded under the same ID
  with the next free suffix (`~2`, `~3`…).

## 6.4 Transfers

Job: money moving between two of your accounts is one transfer, never an expense plus an
income.

- A row's **potential transfer matches** are the rows of every staged import in another of
  your accounts, with the opposite amount in the same commodity, within
  [`TRANSFER_DAYS`](../server/crates/suggest/src/params.rs). Every suggestion level sees
  them.
- Two rows that are each other's potential transfer match are a `transfer_pair`, unless a
  counterparty (or none) you chose for either says otherwise. Suggestions don't prevent a
  pair.
- Pairs are one-to-one: rows you chose on both sides first, then the closest dates.
- When you chose a row as a transfer to an account, and an unpaired row there fits it, the
  two are a `transfer_pair` whose default keeps your choice (*add separately*), short of
  certain.
- A pair added as one transfer settles each row's other side as the other row's account.
- A row's choice never changes whether it's paired; pairs are worked out again across all
  staged imports whenever one changes (upload, a statement's account, a row, discard,
  commit, refresh).
- When the transfer is already in the ledger (a one-split transfer between this account and
  another of yours, same amount, same way, within `TRANSFER_DAYS`, holding no bank row of
  this account), the row is a `duplicate` of it if recorded the same day, else a
  `possible_duplicate`.
- A one-sided transaction already in the ledger is never turned into a transfer later.

## 6.6 Review

Job: every row shows what committing will do with it, and you change only what you disagree
with.

| Label | Found | Choices | Default |
|---|---|---|---|
| `new` | Nothing | add · don't add | add |
| `duplicate` | This bank row unchanged, or its transfer recorded the same day | add · don't add | don't add |
| `possible_duplicate` | A ledger transaction that may be it | add · don't add | don't add |
| `transfer_pair` | The other side, in another staged statement | add (one transfer) · add separately · don't add | add, unless your choices say otherwise |
| `no_account` | The statement has no account | none: the rows wait for it | — |

- A default's confidence is matching's ([`ai.md` → Matching](ai.md#matching)).
- A transfer pair's two rows share one choice.
- Undoing a choice goes back to the default. When a row's label, match or pair changes, it
  goes back to the new label's default, and its Reviewed tick clears.
- A row's **state**, checked in order: `not_added` when it's set to don't add; `needs_you`
  when a value it needs is missing; `to_review` when something it shows (a suggestion, a
  default) is short of [`CERTAIN`](../server/crates/suggest/src/params.rs) and it isn't
  ticked; else `ready`.
- A statement is `needs_you` without an account, `to_review` while its account is a
  suggestion short of certain, else `ready`.
- **Reviewed:** a tick makes what the row shows count as yours for its state, without making
  its labels your choices (a pair can still form). It holds while the row shows what you
  ticked, and goes by itself when that changes. A row that needs you or isn't added can't be
  ticked. Ticks can be taken back until commit.
- **Readiness** of a staged import: `needs_you` if a statement or a row is, else `to_review`
  if one is, else `ready`; with its rows by state and the transactions a commit would add.

## 6.7 Commit

Job: what review shows becomes ledger transactions, all at once or not at all.

- Commit takes what's on screen: your choices and labels, and defaults and suggestions where
  you made none.
- Commit is refused while anything is `needs_you`: a statement without an account, or an
  added row missing a label. With `to_review` left, clients ask once first.
- Before committing, every label and pair is worked out again; if one changed, nothing is
  committed and the client reviews again.
- Added rows become transactions in one import batch, one split each, or one per part for a
  row split into parts (parts add up to the row's amount, and a split row isn't matched as
  a transfer).
- A transfer pair added as one becomes one transfer, dated and described as the side the
  money left, holding both bank rows; the other file's row is then a `duplicate` of it.
- A `duplicate` or `possible_duplicate` that isn't added creates nothing: its bank row is
  recorded with the transaction it matched, so it never comes up again.
- Every bank row now in the ledger keeps its raw fields as the file had it; files are told
  apart by their bytes, so a copy under another name adds nothing. Uploaded files aren't
  kept.
- A closed month (design §4.4) or a broken ledger rule refuses the whole commit, naming the
  row.
- After a commit, every other staged import is worked out again, since the ledger changed.
- Undoing a batch deletes its transactions and their recorded bank rows. Every transaction
  has a batch, edits never change it, and one entered by hand is a batch of its own.
- **Committing several:** each import listed as `ready` is committed on its own, oldest
  first, checked again just before its turn; each comes back committed, or skipped with the
  reason (gone, not ready, changed since, refused).
- An OFX ledger balance is recorded as a balance check on commit (design §7.1).
