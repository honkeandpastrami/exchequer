# UX guidelines

How the web app (and, where it applies, the CLI) looks and behaves. Every screen follows
these; a screen that can't is a reason to change this doc, not to make an exception.

## 1. The user decides

- Everything the app decides on its own (a suggestion, a duplicate, a transfer pair, an
  account guess) is shown as such, says why, and can be overridden.
- Every choice can be undone from where it was made. No state is one-way: if a control can
  leave a state, the same control can return to it ("Not a transfer" → back to the match).
- What the app proposed stays visible after the user overrides it, so they can go back.

## 2. Nothing moves

- Interacting never shifts the layout. Space for what may appear (a badge, a hint, an
  error, a second line) is reserved up front; a control's width doesn't depend on its value.
- Nothing is cut short with an ellipsis: text wraps. Where something's smallest width
  doesn't fit, the layout switches; values never shrink. Every width has a layout.
- **Text wins over badges.** Every value shows its badge, 100% included, while there's
  room; where a badge would cost a value its room, the badge goes, never the text.
- Nothing is inserted above what the user is working on. Lists don't reorder while being
  edited; new or changed items keep their place until the page is reloaded.

## 3. One vocabulary

- One word per concept, everywhere: screens, API docs, CLI, design doc. The terms are the
  design doc's: *counterparty*, *category*, *purpose* (shown as **For**), *owner*,
  *transfer*, *potential transfer match*, *transfer pair*, *duplicate*, *possible
  duplicate*, *suggestion*, *staging*, *commit*.
- Buttons are a verb, plus an object when the verb alone is ambiguous: **Save**, **Cancel**,
  **Edit**, **Delete**, **Add**, **Discard**, **Commit import**. The same action has the
  same word on every screen. No words with two readings ("Back the suggestion").
- A choice says what you end up with: a staged row is **Add** or **Don't add**, never
  "skip" or "match"; a transfer pair is **Add as one transfer**, **Add separately** or
  **Don't add**.
- In progress: the verb with an ellipsis (**Saving…**), the control disabled.

## 4. Values, none and unknown

- A real value (a name, an amount) is plain text. Anything that isn't a value is shown as
  a tag: a small pill with a border and lowercase text, never plain text, so it can't be
  mistaken for data even at a glance or in a dense table:
  - **none**: deliberately empty; a valid answer. A neutral tag: `none`.
  - **unknown**: not set, an outside party included. A dark grey tag: `unknown`, never
    plain "Unknown", which could be a company's name. Whether it needs you is said by
    its row's colour, not by the tag.
- The two tags are one shared component, the same on every screen, in every field
  (counterparty, category, For, account) and in dropdowns.
- An option is offered only if it's a meaningful choice. The default that empty already
  means isn't a list item.

## 5. Suggestions

- A suggestion shows its source by color (rules, matching, local AI, remote AI;
  `tokens.css`) and its confidence by fill: solid when confident, outlined when not. A
  user's own choice has no color.
- Hovering or tapping it says why (the rule, the similar transactions, the model and its
  probability): on a staged row, tapping a badge opens the row's **Details** at that
  field, where its reason sits under it in full.
- A suggestion is the default: the value shown, the option chosen. It's never an extra
  option, and nothing says "(suggested)". Choosing a value yourself is the field's own
  control; agreeing with all of a bank row's suggestions at once is its **Reviewed** tick
  (§10).

## 6. Looks

- Components use only the tokens in `web/src/lib/theme/tokens.css`: no literal colors,
  sizes or fonts. Native controls (`select`, `input`, `button`) look like the rest of the
  page, in light and dark.
- Shared things come from shared components: amounts (`Amount`), suggestions
  (`SuggestionBadge`), none and unknown (`NoValue`), names of legs (`LegName`), failures
  (`Failure`), statuses (`Mark`), a row's form (`RowForm`), a form's field (`Field`), a
  transaction's splits (`SplitLines`), tags (`Tags`), what a commit did
  (`CommittedImport`). A second way of showing the same thing is a bug.
- A status is a symbol plus a word, in its colour, never a dot or colour alone:
  - ✓ ok, green: settled, ready, matches;
  - ⚠ warning, amber: it would go through but you should look (confident suggestions, a
    month closed with a difference);
  - ✗ error, red: it needs you first;
  - ○ open, muted: not done yet;
  - ↺ in progress, accent: paying down a past difference;
  - ⊘ off, smoke: not added (a staged row left out), out of the way.
- A staged row, a statement or an import shows its colour as a strip down its left edge,
  and its symbol before its label: ✗ needs you, ⚠ to review, ✓ ready, ⊘ not added.
- Amounts: tabular figures, right-aligned in columns, sign and commodity always shown,
  never rounded for display beyond the commodity's precision. Dates in one format
  (`dates.ts`).

## 7. Feedback

- An error appears next to what caused it, in words that say what to do. A failed request
  never loses what the user typed.
- Deleting or discarding asks once, naming what will be lost; anything else that can be
  undone doesn't ask, but committing an import with rows to review asks once, saying how
  many will be committed as shown.
- Long work (an upload, remote AI) shows progress, and the rest of the page stays usable.

## 8. Privacy

- Account numbers (and other identifiers) are masked to their last four digits by default.
  Once signed in, the user can choose to show them in full: one setting, remembered in
  that browser, applied everywhere at once (#16).
- What's masked is decided in one place and applied by the shared components, not per
  screen. Masked or not, a number looks the same way on every screen.
- Masking is a display choice in the web app only: what the server sends elsewhere (logs,
  remote AI) is never affected by it.

## 9. Everywhere

- The web app is for laptops and phones alike: every screen and every task works on both,
  not a reduced version on one. On a laptop: mouse and keyboard, wide screens. On a phone:
  touch, narrow screens, no hover (anything shown on hover is also shown on tap), targets
  big enough for a finger: every control at least `--target` tall (`tokens.css`), the
  whole line of a checkbox or radio its target, its words included, and the boxes drawn
  larger than the browser's.
- At phone width the page never scrolls sideways. Transactions, imports and an import's
  rows turn each row into a block: what it is and its amount first, then the rest, its
  actions with it. Other wide tables scroll inside themselves.
- Works with the keyboard alone, and in dark mode.
- Focus is always visible; every control has a label.
- Short text: a label or a one-line hint, not a paragraph. Explanations go in the docs.
- No footnotes. A page doesn't end with notes on how it works, what it leaves out or why;
  if the user needs to know something, the screen shows it where it applies (a label, a
  column, a disabled control), and otherwise it belongs in the docs.

## 10. Rows and forms

- Adding or changing something happens where it is, never on another page. Opening a row
  (or its **Details**) opens its form directly under it; **Add** opens the same form,
  blank, at the top of the list. There's no separate read-only view: facts about the item
  (its ID, where it came from) sit at the bottom of its form.
- A form changes nothing until **Save**; **Cancel** closes it and drops what was typed. A
  saved item keeps its place in the list until the page is reloaded (§2). The exception
  says so where it is: an account's numbers are saved as each is added or removed.
- A row's list shows what it is and a way to open it (**Edit**, or **Details** for an
  import's row); **Delete** is in its form, with the other actions on the whole item.
- **One item per line.** A form's fields and a row's values are never packed side by side.
  Names sit in a column on wide screens, above their control on narrow ones (`Field`);
  each control uses the rest of the width, its badge at the end, and the line under it
  (where its value came from) wraps.
- A transaction's splits come in that order: from, to, amount, category, For, note. With
  two or more, each is a numbered block ending in **Remove split**. A field a split can't
  have stays in its place, disabled. **Add split** copies the split above. Rarer fields
  (another commodity received, a settled month) open under **More fields**.
- Tags are chips: typing offers the existing tags, Enter adds one, × removes it (`Tags`).
- A split's from and to are one text box each: as you type it offers the accounts and
  counterparties that match, alike. A name that's neither is a new counterparty, said so
  in the list; **Unknown** is someone outside the household, unknown who.
- A dropdown lists each option once.
- A transaction has an address (`/transactions/<id>`): the list showing only it, its form
  open.
- A bank row in import review shows what you agree to, never the why: its label, then its
  values one per line with their badges. Its choices are a radio group, one per line, the
  same in every layout. Wide, the date, description, amount and choices share the first
  line and the values run under them from the date's edge to the choices; narrow, it's
  one stacked block.
- Its **Details** open the same form as a transaction's, on every row. Its account's side
  is fixed, the amount is the bank's until it's split (then the parts must add up to it,
  shown with `Mark`), and under each suggested field its reason. Saving makes everything
  the form shows yours, so the row is ready. Staging has no **More fields**.
- Outside the form and applied at once, the row's **Reviewed** tick, on rows to review
  (and rows you ticked): you agree with what it shows. It clears itself when the row
  changes. The colour counts at the top of the review are its filters; **Tick all
  reviewed** ticks the rows to review the filter shows.
