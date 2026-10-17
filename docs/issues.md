# Posting issues

One issue, one problem. The title says what's wrong or wanted, in plain words.

## Type: what kind of issue

Every issue has exactly one type.

| Type | When |
|---|---|
| **Bug** | Something behaves differently from the design or from what's obviously intended |
| **Feature** | Something new, or a change to how something works |
| **Task** | Work with no visible behavior change: cleanup, docs, deployment, tests |

The type replaces the old `bug` and `enhancement` labels; don't use those.

## Labels: where it is

One or more area labels (a staging screen that suggests wrong labels is `data import`
and `AI`, plus `web UX` if it's about the screen):

| Label | Area |
|---|---|
| `web UX` | The web app |
| `CLI/TUI` | The command-line tools |
| `API` | The server's API |
| `data import` | Bank files, staging, duplicates, transfers, commit; migration from Firefly |
| `AI` | Suggestions: rules, local AI, remote AI, backtests |
| `tools/deployment` | Containers, releases, backups, CI, dev tooling |
| `documentation` | The docs: design, requirements, guides |

And, when it applies, how it was closed: `duplicate` (link the original), `wontfix`,
`invalid`. `question` marks an issue waiting for an answer.

## Body

- **Bug:** what you did, what happened, what you expected. The version (`/version`) and
  the screen or endpoint.
- **Feature:** what you want to do and why; not how to build it.
- **Task:** what's done when it's done.

## Never real data

No real data in an issue: no names of people, banks, merchants or accounts, no amounts,
dates, bank texts, file names or screenshots from the live ledger. Describe the shape
instead ("a card payment and the checking withdrawal that pays it"), or use made-up values.
