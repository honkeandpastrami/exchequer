"""The migration report, as Markdown (design §11)."""

from __future__ import annotations

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
