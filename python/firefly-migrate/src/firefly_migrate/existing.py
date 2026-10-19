"""What the target server already holds, and how a year's plan fits it
(design §11).

A migration writes one year. The server may be empty, or hold other years:
the year then goes before what it holds, whose first opening balances (the
day after the cutoff) the year replaces, or after it, whose balances on the
year's first day must be the year's opening balances. Dates the server
already holds data for are refused unless the operator allows the overlap;
then every transaction there must be one the plan has, unchanged.

`fit` is pure: it lists every reason not to write (`stops`) before anything
is written, and the writer refuses to start while there is one.
"""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Iterable
from dataclasses import dataclass, field, replace
from datetime import date, timedelta
from decimal import Decimal
from typing import Any
from uuid import UUID

from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.accounts import accounts_list as list_accounts
from exchequer_sdk.api.categories import categories_list as list_categories
from exchequer_sdk.api.commodities import commodities_list as list_commodities
from exchequer_sdk.api.counterparties import counterparties_list as list_counterparties
from exchequer_sdk.api.parties import parties_list as list_parties
from exchequer_sdk.api.settlements import settlements_history as get_history
from exchequer_sdk.api.transactions import transactions_imported_rows as imported_rows
from exchequer_sdk.api.transactions import transactions_list as list_transactions
from exchequer_sdk.models import Problem
from exchequer_sdk.types import Unset
from firefly_migrate.check import _all
from firefly_migrate.mapping import Mapping, account_key
from firefly_migrate.settle import months
from firefly_migrate.transform import (
    FIREFLY,
    OPENING_BALANCES,
    Leg,
    Plan,
    Split,
    Transaction,
    exact,
    normalize_name,
)

# Near duplicates (design §6.3): same account, commodity and signed amount,
# this many days apart at most; transfers, from either account's side.
NEAR_DAYS = 3
TRANSFER_DAYS = 5
# How far around the year bank rows already recorded are looked for.
ROW_DAYS = 7


@dataclass(frozen=True)
class Held:
    """A transaction on the server, in the plan's terms: names, not IDs."""

    id: UUID
    t: Transaction
    # (account, row ID) of the bank rows behind it; read only near the year.
    rows: frozenset[tuple[str, str]] = frozenset()

    @property
    def opening(self) -> bool:
        return is_opening(self.t)


@dataclass
class Ledger:
    """The server's data. Reference data by name: parties (name → ID, kind),
    commodities (code → ID, kind, precision), accounts (`Owner · Name` → ID,
    kind), counterparties (normalized name → ID), system counterparties and
    categories (name → ID)."""

    transactions: list[Held] = field(default_factory=list)
    parties: dict[str, tuple[UUID, str]] = field(default_factory=dict)
    commodities: dict[str, tuple[UUID, str, int]] = field(default_factory=dict)
    accounts: dict[str, tuple[UUID, str]] = field(default_factory=dict)
    counterparties: dict[str, UUID] = field(default_factory=dict)
    system: dict[str, UUID] = field(default_factory=dict)
    categories: dict[str, UUID] = field(default_factory=dict)
    closed: set[str] = field(default_factory=set)  # closed months, YYYY-MM


@dataclass
class Fit:
    writes: list[Transaction] = field(default_factory=list)
    # Planned transactions already on the server, unchanged.
    already: int = 0
    # Planned transactions given the labels Firefly lacks from the server's copy.
    labeled: int = 0
    # The server's opening balances that the year's end replaces.
    replace: list[Held] = field(default_factory=list)
    # A closed month the replaced opening balances are in: reopened for it.
    reopen: str | None = None
    # What the server lacks: kind ("accounts", ...) → names.
    create: dict[str, list[str]] = field(default_factory=lambda: defaultdict(list))
    stops: list[str] = field(default_factory=list)


# ---------------------------------------------------------------- Helpers ----


def is_opening(t: Transaction) -> bool:
    if len(t.splits) != 1:
        return False
    return OPENING_BALANCES in (t.splits[0].from_.system, t.splits[0].to.system)


def month(d: date) -> str:
    return d.isoformat()[:7]


Balances = dict[tuple[str, str], Decimal]


def balances(transactions: Iterable[Transaction]) -> Balances:
    """Net quantity per (account, commodity)."""
    out: Balances = defaultdict(Decimal)
    for t in transactions:
        for s in t.splits:
            if s.from_.account:
                out[(s.from_.account, s.from_.commodity)] -= s.from_.quantity
            if s.to.account:
                out[(s.to.account, s.to.commodity)] += s.to.quantity
    return out


def _differences(want: Balances, have: Balances) -> list[tuple[str, str, Decimal, Decimal]]:
    keys = sorted(k for k in set(want) | set(have) if want.get(k, 0) != have.get(k, 0))
    return [(a, c, want.get((a, c), Decimal(0)), have.get((a, c), Decimal(0))) for a, c in keys]


def _content(t: Transaction) -> tuple[Any, ...]:
    """What makes two transactions the same, as compared: everything but IDs,
    import batches and settlement links. Counterparties by normalized name."""

    def leg(x: Leg) -> tuple[Any, ...]:
        cp = normalize_name(x.counterparty) if x.counterparty else None
        return (x.commodity, x.quantity, x.account, cp, x.system)

    return (
        t.date,
        t.description,
        t.notes or None,
        tuple(sorted(t.tags)),
        tuple(
            (leg(s.from_), leg(s.to), s.purpose, s.category, s.description or None)
            for s in t.splits
        ),
    )


def difference(planned: Transaction, held: Transaction) -> str | None:
    """The first thing that differs, in words, or None."""
    a, b = _content(planned), _content(held)
    if a == b:
        return None
    names = ("date", "description", "notes", "tags", "splits")
    what = next(n for n, x, y in zip(names, a, b, strict=True) if x != y)
    return f"its {what} differ"


def describe(t: Transaction) -> str:
    first = t.splits[0]
    leg = first.from_ if first.from_.account else first.to
    amount = f"{leg.quantity} {leg.commodity}" + (f" on {leg.account}" if leg.account else "")
    who = f"Firefly transaction {t.firefly_id}" if t.firefly_id else "a transaction"
    return f"{who} ({t.date}, {t.description!r}, {amount})"


def _signed_legs(t: Transaction) -> Iterable[tuple[str, str, Decimal, bool]]:
    """(account, commodity, signed quantity, transfer?) of each owned leg."""
    for s in t.splits:
        transfer = bool(s.from_.account and s.to.account)
        if s.from_.account:
            yield s.from_.account, s.from_.commodity, -s.from_.quantity, transfer
        if s.to.account:
            yield s.to.account, s.to.commodity, s.to.quantity, transfer


def near(a: Transaction, b: Transaction) -> bool:
    """Possibly the same transaction twice (design §6.3)."""
    for acct, comm, qty, transfer in _signed_legs(a):
        for acct2, comm2, qty2, transfer2 in _signed_legs(b):
            if (acct, comm, qty) != (acct2, comm2, qty2):
                continue
            days = TRANSFER_DAYS if transfer and transfer2 else NEAR_DAYS
            if abs((a.date - b.date).days) <= days:
                return True
    return False


# -------------------------------------------------------------------- Fit ----


def fit(m: Mapping, plan: Plan, ledger: Ledger, *, overlap: bool) -> Fit:
    out = Fit()
    start, through = m.start, plan.through
    day_after = through + timedelta(days=1)
    _reference(m, plan, ledger, out)

    before = [h for h in ledger.transactions if h.t.date < start]
    within = [h for h in ledger.transactions if start <= h.t.date <= through]
    later = [h for h in ledger.transactions if h.t.date > through]

    if within and not overlap:
        out.stops.append(
            f"the server already has {len(within)} transactions from {start} to {through}; "
            "migrating into dates it holds data for needs --overlap"
        )
        per: dict[str, list[date]] = defaultdict(list)
        for h in within:
            for acct in {x for x, *_ in _signed_legs(h.t)} or {"(no account)"}:
                per[acct].append(h.t.date)
        for acct, days in sorted(per.items()):
            out.stops.append(f"  {acct}: {len(days)} from {min(days)} to {max(days)}")
        return out

    by_ref: dict[str, list[Held]] = defaultdict(list)
    for h in ledger.transactions:
        if h.t.firefly_id:
            by_ref[h.t.firefly_id].append(h)

    planned = []
    # Missing labels the server can't give: a stop each, but their amounts
    # still count, and their copies on the server are theirs.
    unlabeled: list[Transaction] = []
    matched: set[UUID] = set()
    for t in plan.transactions:
        copies = by_ref.get(t.firefly_id or "", [])
        if not t.missing:
            planned.append(t)
        elif (filled := _labels_from(t, copies, out)) is not None:
            planned.append(filled)
        else:
            matched.update(h.id for h in copies)
            unlabeled += [] if copies else [t]
    if before:
        # Earlier years are there: their end is the year's opening balances.
        want = balances(t for t in planned if is_opening(t))
        have = balances(h.t for h in before)
        for acct, comm, w, h in _differences(want, have):
            out.stops.append(
                f"{acct} {comm} on {start}: the server has {h}, Firefly's opening balance is {w}"
            )
        planned = [t for t in planned if not is_opening(t)]

    # Planned transactions already there, by Firefly ID (opening balances,
    # which have none, by account and commodity).
    for t in planned:
        if t.firefly_id:
            found = by_ref.get(t.firefly_id, [])
        else:
            key = next(iter(balances([t])))
            found = [h for h in within if h.opening and next(iter(balances([h.t]))) == key]
        if not found:
            out.writes.append(t)
            continue
        matched.update(h.id for h in found)
        h = found[0]
        if len(found) > 1:
            out.stops.append(f"{describe(t)} is on the server {len(found)} times")
        elif not start <= h.t.date <= through:
            out.stops.append(f"{describe(t)} is already on the server, dated {h.t.date}")
        elif (diff := difference(t, h.t)) is not None:
            out.stops.append(f"{describe(t)} is on the server, changed: {diff}")
        else:
            out.already += 1

    # Anything else in the year, and anything not migrated from Firefly
    # around it, may be a transaction the year brings again.
    margin = timedelta(days=TRANSFER_DAYS)
    others = [
        h
        for h in ledger.transactions
        if h.id not in matched
        and not h.opening
        and start - margin <= h.t.date <= through + margin
        and (start <= h.t.date <= through or not h.t.firefly_id)
    ]
    paired: set[UUID] = set()
    for t in out.writes:
        for h in others:
            if near(t, h.t):
                paired.add(h.id)
                out.stops.append(f"{describe(t)} may already be on the server as {describe(h.t)}")
    for h in within:
        if h.id not in matched and h.id not in paired:
            out.stops.append(f"on the server, not in Firefly's {m.year}: {describe(h.t)}")

    rows = {row: h for h in ledger.transactions if h.id not in matched for row in h.rows}
    for t in out.writes:
        for row in t.fingerprints:
            if row in rows:
                out.stops.append(
                    f"a bank row of {describe(t)} is already on the server's "
                    f"{describe(rows[row].t)}"
                )

    closed = sorted({month(t.date) for t in out.writes} & ledger.closed)
    if closed:
        out.stops.append(f"the year writes into closed months: {', '.join(closed)}")

    if later:
        # What's after the year starts from opening balances on the day
        # after it: the year's end replaces them, to the cent. Where there
        # are none (already replaced, by an earlier run), the year must not
        # change what's after it.
        out.replace = [h for h in later if h.opening and h.t.date == day_after]
        have = balances(h.t for h in out.replace)
        kept = [h.t for h in [*before, *within] if h.id in matched or h.t.date < start]
        end = balances([*kept, *out.writes, *unlabeled])
        added = balances(out.writes)
        for acct, comm in sorted(set(end) | set(have) | set(added)):
            key = (acct, comm)
            if key in have and end[key] != have[key]:
                out.stops.append(
                    f"{acct} {comm}: the year ends at {end[key]}, the server's opening balance "
                    f"on {day_after} is {have[key]}"
                )
            elif key not in have and added.get(key, 0) != 0:
                out.stops.append(
                    f"{acct} {comm}: the year adds {added[key]}, and the server's data after it "
                    f"has no opening balance on {day_after} to replace"
                )
        if out.replace and month(day_after) in ledger.closed:
            out.reopen = month(day_after)
    return out


def _labels_from(t: Transaction, copies: list[Held], out: Fit) -> Transaction | None:
    """`t` with the labels Firefly lacks taken from the server's copy of it,
    or None, with the reasons in `out.stops`, if there's no copy to take
    them from. Everything else is compared afterwards, as for any other."""
    held = copies[0].t if len(copies) == 1 else None
    if held is None or len(held.splits) != len(t.splits):
        out.stops += [why for _, _, why in t.missing]
        return None
    splits = list(t.splits)
    for i, label, why in t.missing:
        value = getattr(held.splits[i], label)
        if value is None:
            out.stops.append(f"{why}, and the server's copy has none either")
            return None
        splits[i] = replace(splits[i], **{label: value})
    out.labeled += 1
    return replace(t, splits=splits, missing=[])


def _reference(m: Mapping, plan: Plan, ledger: Ledger, out: Fit) -> None:
    for name, kind in [
        (m.household, "household"),
        *((p, "person") for p in m.people),
        *((b, "business") for b in m.businesses),
    ]:
        if name not in ledger.parties:
            out.create["parties"].append(name)
        elif ledger.parties[name][1] != kind:
            have = ledger.parties[name][1]
            out.stops.append(f"party {name} is a {have} on the server, not a {kind}")
    for code, spec in m.commodities.items():
        if code not in ledger.commodities:
            out.create["commodities"].append(code)
        elif ledger.commodities[code][1:] != (spec.kind, spec.precision):
            _, kind, precision = ledger.commodities[code]
            out.stops.append(
                f"commodity {code} is a {kind} with precision {precision} on the server, "
                f"not a {spec.kind} with precision {spec.precision}"
            )
    for a in m.accounts:
        if a.key not in plan.accounts:
            continue
        if a.key not in ledger.accounts:
            out.create["accounts"].append(a.key)
        elif ledger.accounts[a.key][1] != a.kind:
            out.stops.append(
                f"account {a.key} is a {ledger.accounts[a.key][1]} account on the server, "
                f"not a {a.kind} one"
            )
    out.create["counterparties"] += [
        n for n in plan.counterparties if normalize_name(n) not in ledger.counterparties
    ]
    out.create["categories"] += [n for n in plan.categories if n not in ledger.categories]


# ------------------------------------------------------------------- Read ----


class ReadError(RuntimeError):
    pass


def read(client: AuthenticatedClient, m: Mapping, through: date) -> Ledger:
    """Everything `fit` needs from the server; only reads."""
    lg = Ledger()

    def every(fetch: Any) -> list[Any]:
        try:
            return _all(fetch)
        except RuntimeError as e:
            raise ReadError("couldn't read the target server (is the token right?)") from e

    parties = every(lambda c: list_parties.sync(client=client, limit=500, cursor=c))
    names = {str(p.id): p.name for p in parties}
    lg.parties = {p.name: (UUID(str(p.id)), p.kind.value) for p in parties}
    commodities = every(lambda c: list_commodities.sync(client=client, limit=500, cursor=c))
    codes = {str(x.id): x.code for x in commodities}
    lg.commodities = {x.code: (UUID(str(x.id)), x.kind.value, x.precision) for x in commodities}
    accounts = every(lambda c: list_accounts.sync(client=client, limit=500, cursor=c))
    keys = {str(a.id): account_key(names[str(a.owner_id)], a.name) for a in accounts}
    lg.accounts = {keys[str(a.id)]: (UUID(str(a.id)), a.kind.value) for a in accounts}
    counterparties = every(lambda c: list_counterparties.sync(client=client, limit=500, cursor=c))
    cps: dict[str, Leg] = {}
    for x in counterparties:
        if isinstance(x.system, str):
            lg.system[x.system] = UUID(str(x.id))
            cps[str(x.id)] = Leg("", Decimal(0), system=x.system)
        else:
            lg.counterparties[normalize_name(x.name)] = UUID(str(x.id))
            cps[str(x.id)] = Leg("", Decimal(0), counterparty=x.name)
    categories = every(lambda c: list_categories.sync(client=client, limit=500, cursor=c))
    category_names = {str(x.id): x.name for x in categories}
    lg.categories = {x.name: UUID(str(x.id)) for x in categories}

    def opt(value: Any) -> Any:
        return None if isinstance(value, Unset) or value in (None, "") else value

    def leg(x: Any) -> Leg:
        quantity = exact(x.quantity, "a server leg")
        commodity = codes[str(x.commodity_id)]
        if opt(x.account_id):
            return Leg(commodity, quantity, account=keys[str(x.account_id)])
        if opt(x.counterparty_id):
            cp = cps[str(x.counterparty_id)]
            return Leg(commodity, quantity, counterparty=cp.counterparty, system=cp.system)
        return Leg(commodity, quantity)

    near_year = (m.start - timedelta(days=ROW_DAYS), through + timedelta(days=ROW_DAYS))
    for x in every(lambda c: list_transactions.sync(client=client, limit=500, cursor=c)):
        refs = [r.ref for r in x.external_refs if r.source == FIREFLY]
        t = Transaction(
            date=x.date,
            description=x.description,
            splits=[
                Split(
                    from_=leg(s.from_),
                    to=leg(s.to),
                    purpose=names[str(s.purpose_id)] if opt(s.purpose_id) else None,
                    category=category_names[str(s.category_id)] if opt(s.category_id) else None,
                    description=opt(s.description),
                )
                for s in x.splits
            ],
            notes=opt(x.notes),
            tags=list(x.tags),
            firefly_id=refs[0] if refs else None,
        )
        rows: frozenset[tuple[str, str]] = frozenset()
        if near_year[0] <= x.date <= near_year[1]:
            r = imported_rows.sync(UUID(str(x.id)), client=client)
            if r is None or isinstance(r, Problem):
                raise ReadError(f"couldn't read the bank rows of the server's transaction {x.id}")
            rows = frozenset((keys[str(i.account_id)], i.fingerprint) for i in r)
        lg.transactions.append(Held(UUID(str(x.id)), t, rows))

    if not lg.transactions:
        return lg  # nothing to settle, so nothing closed
    last = max([through + timedelta(days=1), *(h.t.date for h in lg.transactions)])
    span = months(f"{m.year}-01", month(last))
    for i in range(0, len(span), 120):  # the server's most per request
        h = get_history.sync(client=client, from_=span[i], to=span[min(i + 119, len(span) - 1)])
        if h is None or isinstance(h, Problem):
            raise ReadError("couldn't read the server's settlement history")
        lg.closed |= {st.month for st in h.months if st.closed}
    return lg
