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

1. **Rules** — a Rhai script you write. *Not built yet.*
2. **Local AI** — similarity to your own labeled history, computed in the server. *Not
   built yet.*
3. **Remote AI** — a decision model on Azure AI Foundry. Optional. *Not built yet.*

A level runs while anything is still below the **confidence threshold, 0.9**. Labels
already settled — by an earlier level at 0.9 or more, or by you — are given to the next
level as facts, so it proposes the rest consistently with them. If no level reaches 0.9
for a label, the most confident suggestion any level made is kept, marked as unsure.

Changing a statement's account suggests its rows' labels again. A label you set yourself
is never overwritten.

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
