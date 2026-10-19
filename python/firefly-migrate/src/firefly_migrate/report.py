"""The migration report, as Markdown (design §11)."""

from __future__ import annotations

from firefly_migrate.existing import Fit, is_opening
from firefly_migrate.transform import Plan

EXAMPLES = 5


def render(p: Plan) -> str:
    r = p.report
    out = [f"# Firefly migration plan: {p.year}, through {p.through}", ""]
    out.append(
        f"- {r.counts['transactions']} transactions, {r.counts['opening balances']} opening "
        f"balances, {len(p.counterparties)} counterparties, {len(p.categories)} categories"
    )
    out.append(f"- skipped after the cutoff: {r.skipped_after_cutoff} transaction groups")
    for key, n in sorted(r.counts.items()):
        if key not in ("transactions", "opening balances") and n:
            out.append(f"- {key}: {n}")
    limits = {a: c for a, c in p.accounts.items() if c}
    if limits:
        out.append(
            "- credit limits: "
            + ", ".join(f"{a} {c.quantity} {c.commodity}" for a, c in limits.items())
        )

    with_numbers = sum(1 for ns in p.numbers.values() if ns)
    out.append(f"- accounts with account numbers: {with_numbers}")

    n = len(p.skipped_accounts)
    out += ["", f"## Archived accounts not migrated: no transactions in {p.year} ({n})", ""]
    for name, balances in sorted(p.skipped_accounts.items()):
        held = ", ".join(f"{q} {c}" for c, q in sorted(balances.items()))
        out.append(f"- {name}" + (f" (balance at the start: {held})" if held else ""))
    if not p.skipped_accounts:
        out.append("None.")

    missing = [t for t in p.transactions if t.missing]
    out += ["", f"## Labels Firefly lacks ({len(missing)}): from the server's copy, if any", ""]
    out += [f"- {why}" for t in missing for _, _, why in t.missing] or ["None."]
    out += ["", f"## Errors ({len(r.errors)})", ""]
    out += [f"- {e}" for e in r.errors] or ["None."]
    out += ["", f"## Rounded float damage ({len(r.rounded)}): confirm each", ""]
    out += [f"- {x}" for x in r.rounded] or ["None."]
    for title, groups in (("Label conflicts", r.conflicts), ("Dropped labels", r.dropped)):
        total = sum(len(v) for v in groups.values())
        out += ["", f"## {title} ({total})", ""]
        for what, ids in sorted(groups.items(), key=lambda kv: -len(kv[1])):
            shown = ", ".join(ids[:EXAMPLES]) + (" …" if len(ids) > EXAMPLES else "")
            out.append(f"- {len(ids)}× {what} (Firefly {shown})")
        if not groups:
            out.append("None.")
    return "\n".join(out)


def render_fit(f: Fit) -> str:
    """How the plan fits the server: what `run` would write, create and
    replace there, and anything that stops it."""
    openings = sum(1 for t in f.writes if is_opening(t))
    out = ["", "## On the server", ""]
    out.append(f"- to write: {len(f.writes) - openings} transactions, {openings} opening balances")
    out.append(f"- already there, unchanged: {f.already}")
    out.append(f"- labels Firefly lacks, taken from the server's copy: {f.labeled} transactions")
    for kind in ("parties", "commodities", "accounts", "categories"):
        if f.create.get(kind):
            out.append(f"- {kind} to create: {', '.join(f.create[kind])}")
    if f.create.get("counterparties"):
        out.append(f"- counterparties to create: {len(f.create['counterparties'])}")
    if f.replace:
        day = f.replace[0].t.date
        out.append(f"- opening balances on {day} to replace with the year's end: {len(f.replace)}")
        if f.reopen:
            out.append(f"  ({f.reopen} is closed: reopened for it, then closed again)")
    out += ["", f"## Stops ({len(f.stops)})", ""]
    out += [f"- {s}" if not s.startswith("  ") else f"  - {s.strip()}" for s in f.stops] or [
        "None."
    ]
    return "\n".join(out)
