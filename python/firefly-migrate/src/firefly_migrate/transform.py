"""Firefly data → an Exchequer ledger plan (design §11). Pure: no I/O.

The plan names everything (accounts, counterparties, parties, categories,
commodities) by name; the writer resolves names to IDs. Every judgment the
migration makes that the operator should confirm goes into the report.
"""

from __future__ import annotations

import re
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from datetime import date
from decimal import ROUND_HALF_EVEN, Decimal, InvalidOperation
from typing import Any

from firefly_migrate.mapping import Mapping

FIREFLY = "firefly"
OPENING_BALANCES = "opening_balances"
RECONCILIATION = "reconciliation"
CASH = "Cash"
# What Firefly calls an expense or revenue account it wasn't given a name for.
FIREFLY_NO_NAME = "(no name)"


# ------------------------------------------------------------------ Plan ----


@dataclass(frozen=True)
class Leg:
    """At most one of `account`, `counterparty` or `system` is set; none means
    outside the household, unknown who."""

    commodity: str
    quantity: Decimal
    account: str | None = None
    counterparty: str | None = None
    system: str | None = None  # OPENING_BALANCES or RECONCILIATION


@dataclass(frozen=True)
class Split:
    from_: Leg
    to: Leg
    purpose: str | None = None
    category: str | None = None
    description: str | None = None


@dataclass
class Transaction:
    date: date
    description: str
    splits: list[Split]
    notes: str | None = None
    tags: list[str] = field(default_factory=list)
    firefly_id: str | None = None  # the group ID; None for opening balances
    # The import tag it came in with in Firefly, if any: its import batch.
    import_batch: str | None = None
    # (account, row ID) for each bank row it was imported from: the journals'
    # `external_id`s, on the migrated accounts they touch (design §6.3).
    fingerprints: list[tuple[str, str]] = field(default_factory=list)


@dataclass(frozen=True)
class CreditLimit:
    commodity: str
    quantity: Decimal


@dataclass
class Report:
    """`conflicts` and `dropped`: what happened → Firefly transaction IDs."""

    rounded: list[str] = field(default_factory=list)
    conflicts: dict[str, list[str]] = field(default_factory=lambda: defaultdict(list))
    dropped: dict[str, list[str]] = field(default_factory=lambda: defaultdict(list))
    errors: list[str] = field(default_factory=list)
    skipped_after_cutoff: int = 0
    counts: Counter[str] = field(default_factory=Counter)


@dataclass
class Plan:
    year: int
    through: date
    accounts: dict[str, CreditLimit | None]  # migrated account name → credit limit
    # Migrated account name → the account numbers its Firefly accounts have.
    numbers: dict[str, tuple[str, ...]]
    # Archived accounts with no transaction in the year, left out with their
    # opening balances: name → commodity → balance at the start.
    skipped_accounts: dict[str, dict[str, Decimal]]
    counterparties: list[str]
    categories: list[str]
    transactions: list[Transaction]
    report: Report


# --------------------------------------------------------------- Helpers ----

# The longest row ID Exchequer records.
MAX_FINGERPRINT = 200

NUMBER = re.compile(r"^[0-9A-Za-z]{1,34}$")


def masked(number: str) -> str:
    return "…" + number[-4:]


def account_numbers(
    m: Mapping, accounts: list[dict[str, Any]], report: Report
) -> dict[str, tuple[str, ...]]:
    """Each mapped account's numbers, from its Firefly accounts' `account_number`,
    as Exchequer stores them (no spaces or dashes). A number on two mapped
    accounts is an error; one Exchequer wouldn't take is left out and counted.
    Reports name numbers only by their last four."""
    by_id = {int(a["id"]): a for a in accounts}
    out: dict[str, tuple[str, ...]] = {}
    owner: dict[str, str] = {}
    for a in m.accounts:
        found: list[str] = []
        for fid in a.firefly:
            raw = str((by_id.get(fid) or {}).get("attributes", {}).get("account_number") or "")
            n = re.sub(r"[\s-]", "", raw)
            if not n:
                continue
            if not NUMBER.match(n):
                report.counts["account numbers left out (not 1-34 letters and digits)"] += 1
                continue
            if owner.setdefault(n, a.key) != a.key:
                report.errors.append(
                    f"account number {masked(n)} is on both {owner[n]} and {a.key}"
                )
                continue
            if n not in found:
                found.append(n)
        out[a.key] = tuple(found)
    return out


def normalize_name(name: str) -> str:
    """The key counterparties are merged by: case and spacing don't matter."""
    return re.sub(r"\s+", " ", name).strip().casefold()


def exact(value: Any, what: str) -> Decimal:
    try:
        return Decimal(str(value))
    except InvalidOperation as e:
        raise ValueError(f"{what}: {value!r} is not a decimal") from e


def fit(amount: Decimal, precision: int, what: str, report: Report) -> Decimal:
    """Rounds float damage to the commodity's precision, and reports it (§3.5)."""
    quantum = Decimal(1).scaleb(-precision)
    rounded = amount.quantize(quantum, rounding=ROUND_HALF_EVEN)
    if rounded != amount:
        report.rounded.append(f"{what}: {amount} → {rounded}")
    return rounded


@dataclass
class _Journal:
    group_id: str
    journal_id: str
    group_title: str | None
    type: str
    date: date
    description: str
    notes: str | None
    source_id: int
    source_type: str
    source_name: str
    destination_id: int
    destination_type: str
    destination_name: str
    amount: Decimal
    currency: str
    foreign_amount: Decimal | None
    foreign_currency: str | None
    budget: str | None
    category: str | None
    tags: list[str]
    external_id: str | None = None


def _journals(groups: list[dict[str, Any]]) -> list[list[_Journal]]:
    out = []
    for g in groups:
        attrs = g["attributes"]
        journals = []
        for t in sorted(attrs["transactions"], key=lambda t: int(t.get("order") or 0)):
            where = f"Firefly transaction {g['id']}"
            foreign = t.get("foreign_amount")
            journals.append(
                _Journal(
                    group_id=str(g["id"]),
                    journal_id=str(t["transaction_journal_id"]),
                    group_title=attrs.get("group_title") or None,
                    type=t["type"],
                    date=date.fromisoformat(t["date"][:10]),
                    description=t["description"],
                    notes=t.get("notes") or None,
                    source_id=int(t["source_id"]),
                    source_type=t["source_type"],
                    source_name=t["source_name"],
                    destination_id=int(t["destination_id"]),
                    destination_type=t["destination_type"],
                    destination_name=t["destination_name"],
                    amount=exact(t["amount"], where),
                    currency=t["currency_code"],
                    foreign_amount=exact(foreign, where) if foreign not in (None, "") else None,
                    foreign_currency=t.get("foreign_currency_code") or None,
                    budget=t.get("budget_name") or None,
                    category=t.get("category_name") or None,
                    tags=list(t.get("tags") or []),
                    external_id=str(t.get("external_id") or "").strip() or None,
                )
            )
        out.append(journals)
    return out


# -------------------------------------------------------------- Transform ----


class _Builder:
    def __init__(self, m: Mapping, report: Report) -> None:
        self.m = m
        self.report = report
        # Normalized name → spellings seen, to pick the most common one.
        self.counterparty_spellings: dict[str, Counter[str]] = defaultdict(Counter)
        self.categories: set[str] = set()
        # Before the migrated year only the sums matter: notes are counted,
        # not listed. Errors are always listed (they'd change the balances).
        self.quiet = False

    def note(self, kind: str, message: str, group_id: str | None = None) -> None:
        if self.quiet:
            self.report.counts[f"{kind} before {self.m.year}"] += 1
        elif group_id is None:
            getattr(self.report, kind).append(message)
        else:
            seen = getattr(self.report, kind)[message]
            if group_id not in seen:
                seen.append(group_id)

    def fit(self, amount: Decimal, commodity: str, where: str) -> Decimal:
        quantum = Decimal(1).scaleb(-self.m.commodities[commodity].precision)
        rounded = amount.quantize(quantum, rounding=ROUND_HALF_EVEN)
        if rounded != amount:
            self.note("rounded", f"{where}: {amount} → {rounded}")
        return rounded

    def counterparty(self, name: str) -> str:
        key = normalize_name(name)
        self.counterparty_spellings[key][name.strip()] += 1
        return key  # resolved to the chosen spelling at the end

    def side(self, fid: int, ftype: str, fname: str, other_fid: int, where: str) -> dict[str, str]:
        account = self.m.by_firefly_id.get(fid)
        if account is not None:
            return {"account": account.key}
        match ftype:
            case "Initial balance account":
                return {"system": OPENING_BALANCES}
            case "Reconciliation account":
                return {"system": RECONCILIATION}
            case "Cash account":
                other = self.m.by_firefly_id.get(other_fid)
                institution = self.m.cash_account_by_destination.get(other.key) if other else None
                return {"counterparty": self.counterparty(institution or CASH)}
            case "Expense account" | "Revenue account" if fname.strip() == FIREFLY_NO_NAME:
                if not self.quiet:
                    self.report.counts["legs with no counterparty (Firefly's (no name))"] += 1
                return {}
            case "Expense account" | "Revenue account":
                return {"counterparty": self.counterparty(fname)}
            case _:
                raise _Skip(f"{where}: {ftype} {fname!r} (Firefly {fid}) is not in the mapping")

    def purpose(self, j: _Journal, where: str) -> str:
        """Who the split is for, from its budget or purpose tags: there's no
        default (design §4.1), so with neither it's an error."""
        labels: dict[str, str] = {}
        if j.budget is not None:
            if j.budget not in self.m.budgets:
                raise _Skip(f"{where}: budget {j.budget!r} is not in the mapping")
            labels[f"budget {j.budget!r}"] = self.m.budgets[j.budget]
        for tag in j.tags:
            if tag in self.m.purpose_tags:
                labels[f"tag {tag!r}"] = self.m.purpose_tags[tag]
        parties = set(labels.values())
        if len(parties) > 1:
            said = ", ".join(f"{k} → {v}" for k, v in sorted(labels.items()))
            chosen = labels.get(f"budget {j.budget!r}")
            if chosen is None:
                raise _Skip(f"{where}: {said}: no budget to settle who it's for")
            self.note("conflicts", f"{said}: {chosen} used", j.group_id)
            return chosen
        if not parties:
            raise _Skip(f"{where}: no budget or purpose tag says who it's for")
        return parties.pop()

    def split(self, j: _Journal) -> Split:
        where = f"Firefly transaction {j.group_id} ({j.date}, {j.description!r})"
        src = self.side(j.source_id, j.source_type, j.source_name, j.destination_id, where)
        dst = self.side(
            j.destination_id, j.destination_type, j.destination_name, j.source_id, where
        )
        if j.amount < 0:
            raise _Skip(f"{where}: negative amount {j.amount}")

        # Firefly's amount is in the source's currency; a foreign amount, when
        # there is one, is what arrived on the other side.
        from_commodity, from_qty = j.currency, j.amount
        to_commodity, to_qty = j.currency, j.amount
        if j.foreign_amount is not None and j.foreign_currency not in (None, j.currency):
            to_commodity, to_qty = j.foreign_currency, j.foreign_amount
        for code in (from_commodity, to_commodity):
            if code not in self.m.commodities:
                raise _Skip(f"{where}: currency {code} is not in the mapping")
        from_qty = self.fit(from_qty, from_commodity, where)
        to_qty = (
            from_qty if to_commodity == from_commodity else self.fit(to_qty, to_commodity, where)
        )

        from_leg = Leg(commodity=from_commodity, quantity=from_qty, **src)
        to_leg = Leg(commodity=to_commodity, quantity=to_qty, **dst)
        if from_qty == 0:
            raise _Skip(f"{where}: zero amount")

        # The kind, as Exchequer derives it (design §3.2), decides whether a
        # purpose and a category are allowed.
        accounts = {a.key: a for a in self.m.accounts}
        a_from = accounts.get(from_leg.account or "")
        a_to = accounts.get(to_leg.account or "")
        if a_from and a_to:
            if a_from is a_to and from_commodity == to_commodity:
                raise _Skip(f"{where}: moves {from_commodity} from {a_from.key} into itself")
            # Into or out of an investment or a loan: an expense or income.
            crossing = a_from.is_liquid != a_to.is_liquid
            transfer = from_commodity == to_commodity and not crossing
            income_or_expense = False
            owned = None
        else:
            crossing = transfer = False
            # With a system counterparty it's an adjustment: no purpose or
            # category (design §3.2).
            income_or_expense = "system" not in src and "system" not in dst
            owned = a_from or a_to

        purpose = category = None
        if crossing:
            purpose = self.purpose(j, where)
            category = j.category
            if not self.quiet:
                self.report.counts["into or out of investments and loans"] += 1
        elif income_or_expense and owned is not None:
            category = j.category
            if owned.is_liquid:
                purpose = self.purpose(j, where)
            elif j.budget or any(t in self.m.purpose_tags for t in j.tags):
                self.note("dropped", f"purpose label on investment account {owned.key}", j.group_id)
        elif j.budget and j.budget in self.m.budgets:
            kind = "transfer" if transfer else "conversion"
            self.note("dropped", f"budget {j.budget!r} on a {kind}", j.group_id)

        if category in self.m.drop_categories:
            if income_or_expense or crossing:
                self.note(
                    "dropped",
                    f"transfer-only category {category!r} on an income/expense",
                    j.group_id,
                )
            category = None
        kept_kind = income_or_expense or crossing
        if j.category and j.category not in self.m.drop_categories and not kept_kind:
            kind = "transfer" if transfer else "conversion"
            self.note("dropped", f"category {j.category!r} on a {kind}", j.group_id)
        if category is not None:
            self.categories.add(category)

        return Split(from_=from_leg, to=to_leg, purpose=purpose, category=category)

    def transaction(self, journals: list[_Journal]) -> Transaction:
        first = journals[0]
        splits = []
        for j in journals:
            split = self.split(j)
            if len(journals) > 1:
                split = Split(
                    from_=split.from_,
                    to=split.to,
                    purpose=split.purpose,
                    category=split.category,
                    description=j.description,
                )
            splits.append(split)
        prefix = self.m.import_batch_prefix
        batches = sorted({t for j in journals for t in j.tags if prefix and t.startswith(prefix)})
        tags = sorted(
            {
                t
                for j in journals
                for t in j.tags
                if t not in self.m.retire_tags and t not in batches
            }
        )
        notes = "\n\n".join(dict.fromkeys(j.notes for j in journals if j.notes)) or None
        fingerprints: list[tuple[str, str]] = []
        for j in journals:
            if not j.external_id:
                continue
            if len(j.external_id) > MAX_FINGERPRINT:
                self.report.counts["external IDs left out (over 200 characters)"] += 1
                continue
            for fid in (j.source_id, j.destination_id):
                account = self.m.by_firefly_id.get(fid)
                pair = (account.key, j.external_id) if account else None
                if pair and pair not in fingerprints:
                    fingerprints.append(pair)
        return Transaction(
            date=first.date,
            description=(first.group_title if len(journals) > 1 else None) or first.description,
            splits=splits,
            notes=notes,
            tags=tags,
            firefly_id=first.group_id,
            import_batch=batches[0] if batches else None,
            fingerprints=fingerprints,
        )


class _Skip(Exception):
    """A Firefly transaction that can't be migrated; reported as an error."""


def plan(
    m: Mapping,
    accounts: list[dict[str, Any]],
    groups: list[dict[str, Any]],
) -> Plan:
    """`groups`: every Firefly transaction group up to `m.through`, from the
    first one ever, so opening balances can be summed exactly."""
    report = Report()
    b = _Builder(m, report)

    # Every mapped Firefly account must exist, as an owned account.
    by_id = {int(a["id"]): a for a in accounts}
    limits: dict[str, CreditLimit | None] = {a.key: None for a in m.accounts}
    for a in m.accounts:
        for fid in a.firefly:
            ff = by_id.get(fid)
            if ff is None:
                report.errors.append(f"account {a.key}: Firefly account {fid} doesn't exist")
                continue
            attrs = ff["attributes"]
            if attrs["type"] not in ("asset", "liabilities", "liability"):
                report.errors.append(
                    f"account {a.key}: Firefly account {fid} is a {attrs['type']} account"
                )
            vb = attrs.get("virtual_balance")
            if a.kind == "credit_card" and vb not in (None, "") and exact(vb, a.key) > 0:
                code = attrs["currency_code"]
                if code in m.commodities:
                    limits[a.key] = CreditLimit(
                        code, fit(exact(vb, a.key), m.commodities[code].precision, a.key, report)
                    )

    opening: dict[tuple[str, str], Decimal] = defaultdict(Decimal)
    transactions: list[Transaction] = []
    for journals in _journals(groups):
        day = journals[0].date
        if day > m.through:
            report.skipped_after_cutoff += 1
            continue
        b.quiet = day < m.start
        try:
            t = b.transaction(journals)
        except _Skip as e:
            report.errors.append(str(e))
            continue
        if day < m.start:
            for s in t.splits:
                if s.from_.account:
                    opening[(s.from_.account, s.from_.commodity)] -= s.from_.quantity
                if s.to.account:
                    opening[(s.to.account, s.to.commodity)] += s.to.quantity
        else:
            transactions.append(t)
    report.counts["transactions"] = len(transactions)

    # Archived accounts unused in the year aren't migrated; they're listed in
    # the report instead, opening balance and all. Accounts still in use are
    # always migrated, active this year or not.
    active = {
        leg.account
        for t in transactions
        for sp in t.splits
        for leg in (sp.from_, sp.to)
        if leg.account
    }
    skipped: dict[str, dict[str, Decimal]] = {
        a.key: {} for a in m.accounts if a.archived and a.key not in active
    }
    for (account, commodity), balance in opening.items():
        if account in skipped and balance != 0:
            skipped[account][commodity] = balance
    limits = {name: limit for name, limit in limits.items() if name not in skipped}
    numbers = {
        name: n for name, n in account_numbers(m, accounts, report).items() if name in limits
    }

    # One opening-balance transaction per account and commodity (§11).
    openings = []
    for (account, commodity), balance in sorted(opening.items()):
        if balance == 0 or account in skipped:
            continue
        system = Leg(commodity=commodity, quantity=abs(balance), system=OPENING_BALANCES)
        mine = Leg(commodity=commodity, quantity=abs(balance), account=account)
        split = Split(from_=system, to=mine) if balance > 0 else Split(from_=mine, to=system)
        openings.append(Transaction(date=m.start, description="Opening balance", splits=[split]))
    report.counts["opening balances"] = len(openings)

    spellings = {key: c.most_common(1)[0][0] for key, c in b.counterparty_spellings.items()}
    merged = sum(1 for c in b.counterparty_spellings.values() if len(c) > 1)
    report.counts["counterparties merged from several spellings"] = merged

    def resolve(leg: Leg) -> Leg:
        if leg.counterparty is None:
            return leg
        return Leg(leg.commodity, leg.quantity, counterparty=spellings[leg.counterparty])

    used: set[str] = set()
    final = []
    for t in [*openings, *transactions]:
        splits = []
        for s in t.splits:
            f, to = resolve(s.from_), resolve(s.to)
            used.update(x for x in (f.counterparty, to.counterparty) if x)
            splits.append(Split(f, to, s.purpose, s.category, s.description))
        t.splits = splits
        final.append(t)

    report.counts["bank row IDs (Firefly external IDs) on migrated accounts"] = sum(
        len(t.fingerprints) for t in final
    )
    used_categories = {s.category for t in final for s in t.splits if s.category}
    return Plan(
        year=m.year,
        through=m.through,
        accounts=limits,
        numbers=numbers,
        skipped_accounts=skipped,
        counterparties=sorted(used),
        categories=sorted(used_categories),
        transactions=final,
        report=report,
    )
