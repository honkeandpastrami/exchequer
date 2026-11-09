# UX principles

How the web app (and the CLI where it applies) behaves. A screen that can't follow these is
a reason to change this doc, not to make an exception.

## 1. The user decides

- What the app decides on its own is shown as such, says why, and can be overridden.
- Every choice can be undone where it was made.
- What the app proposed stays visible after it's overridden.
- A suggestion is the default value, never an extra option.

## 2. Nothing moves

- Interacting never shifts the layout: room for what may appear is kept from the start,
  and what floats (a bubble) covers, never pushes.
- Nothing is reordered or inserted above what the user is working on.

## 3. Every width works

- The app lives in a browser window that gets resized: every width looks deliberate, from a
  phone to a wide screen.
- A page is made of sections. Each has a wide and a narrow look and switches as a whole,
  never one piece at a time, and never flips back and forth near the switch.
- Text is never cut short: it wraps. When something doesn't fit, the layout switches;
  values never shrink. Badges give way before text does.

## 4. One vocabulary

- One word per concept and per action, on every screen, in the CLI and the docs; the terms
  are the design doc's.
- A button is a verb, plus an object when the verb alone is ambiguous. No word with two
  readings.
- A choice says what you end up with.

## 5. One look per thing

- Each kind of thing (an amount, a status, a suggestion, a transaction row, a field) looks
  the same everywhere, from one shared piece. A second way of showing it is a bug.
- A value is plain text; anything that isn't one (none, unknown) is a tag, so it can't be
  mistaken for data.
- A status is a symbol plus a word, never colour alone.
- A figure that adds up transactions links to them: the list built from the filter that
  computed it, which totals to it. A figure computed from other figures, or a setting,
  doesn't link, and neither does a chart.
- Only meaningful choices are offered.

## 6. Clear controls

- Every action is a control that says what it does; nothing is a hidden side effect of
  another part of the screen.
- A click area uses the room already there. Fonts stay uniform; no padding for its own sake.
- Hover is used wherever something can hover (a mouse, a pencil), and a tap shows the same.
- Works with the keyboard alone and in dark mode; focus is visible; every control has a
  label.

## 7. Edit in place

- Adding or changing happens where the item is: its form opens under it. There's no
  separate read-only view.
- A form changes nothing until **Save**; **Cancel** drops what was typed.
- One item per line, in forms and in rows.

## 8. Feedback

- An error appears next to its cause and says what to do; nothing typed is lost.
- Only what's hard to undo asks, once, saying what will happen.
- Long work shows progress, and the rest of the page stays usable.

## 9. Short text

- A label or a one-line hint. Explanations go in the docs.
- No paragraph at the top of a page, and no footnotes at the bottom. What the user needs
  goes where it applies: a label, a column header, a field hint.

## 10. Privacy

- Identifiers are masked by default; one remembered setting shows them, everywhere at once.
  It's a display choice only, decided in one place.

## 11. Dates

- One date system, from `web/src/lib/components/dates/`; a page never adds a date control of
  its own (lint enforces it).

  | What the page needs | Component |
  |---|---|
  | Its current day, month or year | `DateStepper` |
  | A run of months | `MonthRange` |
  | A day in a form or a filter | `DateField` |

- Every date shown has one format: `Aug 14, 2026`, `Aug 2026`, `2026`,
  `Sep 2025 – Aug 2026`.
