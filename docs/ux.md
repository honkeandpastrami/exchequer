# UX guidelines

How the web app (and, where it applies, the CLI) looks and behaves. Every screen follows
these; a screen that can't is a reason to change this doc, not to make an exception.

## 1. The user decides

- Everything the app decides on its own (a suggestion, a duplicate, a transfer match, an
  account guess) is shown as such, says why, and can be overridden.
- Every choice can be undone from where it was made. No state is one-way: if a control can
  leave a state, the same control can return to it ("Not a transfer" → back to the match).
- What the app proposed stays visible after the user overrides it, so they can go back.

## 2. Nothing moves

- Interacting never shifts the layout. Space for what may appear (a badge, a hint, an
  error, a second line) is reserved up front; a control's width doesn't depend on its value.
- Nothing is inserted above what the user is working on. Lists don't reorder while being
  edited; new or changed items keep their place until the page is reloaded.

## 3. One vocabulary

- One word per concept, everywhere: screens, API docs, CLI, design doc. The terms are the
  design doc's: *counterparty*, *category*, *purpose* (shown as **For**), *owner*,
  *transfer*, *potential transfer match*, *duplicate*, *suggestion*, *staging*, *commit*.
- Buttons are a verb, plus an object when the verb alone is ambiguous: **Save**, **Cancel**,
  **Edit**, **Delete**, **Add**, **Discard**, **Commit import**. The same action has the
  same word on every screen. No words with two readings ("Back the suggestion").
- A choice says what you end up with: a row left out of an import is **Do not import**,
  never "skip"; a transfer match is **Merge into one transfer** or **Keep separate
  transactions**.
- In progress: the verb with an ellipsis (**Saving…**), the control disabled.

## 4. Values, none and unknown

- A real value (a name, an amount) is plain text. Anything that isn't a value is shown as
  a tag: a small pill with a border and lowercase text, never plain text, so it can't be
  mistaken for data even at a glance or in a dense table:
  - **none**: deliberately empty; a valid answer. A neutral tag: `none`.
  - **unknown**: not known yet; needs attention. A warning-colored tag: `unknown`.
- The two tags are one shared component, the same on every screen, in every field
  (counterparty, category, For, account) and in dropdowns.
- An option is offered only if it's a meaningful choice. The default that empty already
  means isn't a list item.

## 5. Suggestions

- A suggestion shows its source by color (rules, local AI, remote AI; `tokens.css`) and its
  confidence by fill: solid when confident, outlined when not. A user's own choice has no
  color.
- Hovering or tapping it says why (the rule, the similar transactions, the model and its
  probability).
- Accepting a suggestion and choosing a value yourself are the same control (the field's
  dropdown); there's no separate "accept" step.

## 6. Looks

- Components use only the tokens in `web/src/lib/theme/tokens.css`: no literal colors,
  sizes or fonts. Native controls (`select`, `input`, `button`) look like the rest of the
  page, in light and dark.
- Shared things come from shared components: amounts (`Amount`), suggestions
  (`SuggestionBadge`), none and unknown (`NoValue`), names of legs (`LegName`), failures
  (`Failure`), statuses (`Mark`). A second way of showing the same thing is a bug.
- A status is a symbol plus a word, in its colour, never a dot or colour alone:
  - ✓ ok, green: settled, ready, matches;
  - ⚠ warning, amber: it would go through but you should look (confident suggestions, a
    month closed with a difference);
  - ✗ error, red: it needs you first;
  - ○ open, muted: not done yet;
  - ↺ in progress, accent: paying down a past difference.
- Amounts: tabular figures, right-aligned in columns, sign and commodity always shown,
  never rounded for display beyond the commodity's precision. Dates in one format
  (`dates.ts`).

## 7. Feedback

- An error appears next to what caused it, in words that say what to do. A failed request
  never loses what the user typed.
- Deleting or discarding asks once, naming what will be lost; anything else that can be
  undone doesn't ask.
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
  big enough for a finger.
- At phone width the page never scrolls sideways; wide tables scroll inside themselves.
- Works with the keyboard alone, and in dark mode.
- Focus is always visible; every control has a label.
- Short text: a label or a one-line hint, not a paragraph. Explanations go in the docs.
- No footnotes. A page doesn't end with notes on how it works, what it leaves out or why;
  if the user needs to know something, the screen shows it where it applies (a label, a
  column, a disabled control), and otherwise it belongs in the docs.
