"""The settlement pass (design §4.5, §11): an additive pass over a migrated
ledger, through the API only.

It links the settlement payments that settle a
month other than the default, sets each month's stipends and funders,
closes the months already closed in the old system, and reports the implied
stipend per person per month: the acceptance test for the settlement.

Re-running it is safe: anything already as the mapping says is left alone,
and a closed month is reopened only to change it, then closed again.
"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from decimal import Decimal
from typing import Any
from uuid import UUID

from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.parties import parties_list as list_parties
from exchequer_sdk.api.settlements import settlements_close as close_month
from exchequer_sdk.api.settlements import settlements_get as get_settlement
from exchequer_sdk.api.settlements import settlements_history as get_history
from exchequer_sdk.api.settlements import settlements_put as put_settlement
from exchequer_sdk.api.settlements import settlements_reopen as reopen_month
from exchequer_sdk.api.transactions import transactions_get as get_transaction
from exchequer_sdk.api.transactions import transactions_list as list_transactions
from exchequer_sdk.api.transactions import transactions_replace as replace_transaction
from exchequer_sdk.models import (
    Funder,
    Kind,
    NewSplit,
    NewTransaction,
    PartyKind,
    Problem,
    SettlementParams,
    Split,
    Stipend,
    Transaction,
)
from exchequer_sdk.types import Response
from firefly_migrate.mapping import Mapping
from firefly_migrate.transform import FIREFLY


class SettleError(RuntimeError):
    pass


def months(first: str, last: str) -> list[str]:
    """Every month from `first` to `last`, inclusive (`YYYY-MM`)."""
    y, m = map(int, first.split("-"))
    out = []
    while f"{y:04d}-{m:02d}" <= last:
        out.append(f"{y:04d}-{m:02d}")
        y, m = (y + 1, 1) if m == 12 else (y, m + 1)
    return out


def carried(schedule: dict[str, Any], month: str) -> Any | None:
    """The value given for the latest month up to `month`, if any."""
    given = [m for m in schedule if m <= month]
    return schedule[max(given)] if given else None


@dataclass(frozen=True)
class Params:
    stipends: frozenset[tuple[str, Decimal]]
    funders: frozenset[tuple[str, str]]


def month_params(m: Mapping, month: str) -> Params:
    """A month's parameters by party name, from the mapping's schedules."""
    s = m.settlement
    if s is None:
        return Params(frozenset(), frozenset())
    stipends = {(p, carried(sched, month)) for p, sched in s.stipends.items()}
    funders = {(b, carried(sched, month)) for b, sched in s.funders.items()}
    return Params(
        frozenset((p, a) for p, a in stipends if a is not None),
        frozenset((b, f) for b, f in funders if f is not None),
    )


def _ok(response: Response[Any], what: str) -> Any:
    parsed = response.parsed
    if response.status_code >= 300 or parsed is None or isinstance(parsed, Problem):
        detail = parsed.detail if isinstance(parsed, Problem) else response.content[:300]
        raise SettleError(f"{what}: HTTP {response.status_code}: {detail}")
    return parsed


def _etag(response: Response[Any]) -> str:
    return response.headers.get("etag", '"0"')


def _new_split(s: Split, settles: str | None) -> NewSplit:
    return NewSplit(
        from_=s.from_,
        to=s.to,
        category_id=s.category_id,
        description=s.description,
        purpose_id=s.purpose_id,
        settles=settles if settles is not None else s.settles,
    )


@dataclass
class Outcome:
    linked: list[str] = field(default_factory=list)
    params_set: list[str] = field(default_factory=list)
    closed: list[str] = field(default_factory=list)


class Settler:
    def __init__(self, client: AuthenticatedClient, m: Mapping) -> None:
        if m.settlement is None:
            raise SettleError("the mapping has no [settlement] section")
        self.client = client
        self.m = m
        self.s = m.settlement
        parties = _ok(list_parties.sync_detailed(client=client, limit=500), "list parties")
        self.party_ids: dict[str, UUID] = {p.name: UUID(str(p.id)) for p in parties.items}
        self.household = next(p for p in parties.items if p.kind == PartyKind.HOUSEHOLD)
        missing = [n for n in m.parties if n not in self.party_ids]
        if missing:
            raise SettleError(f"parties not on the server: {', '.join(missing)}")
        self.reopened: set[str] = set()

    # -- each step ----------------------------------------------------------

    def _settlement(self, month: str) -> tuple[Any, str]:
        r = get_settlement.sync_detailed(month, client=self.client)
        return _ok(r, f"settlement {month}"), _etag(r)

    def _reopen(self, month: str) -> None:
        settlement, etag = self._settlement(month)
        if settlement.closed:
            _ok(
                reopen_month.sync_detailed(month, client=self.client, if_match=etag),
                f"reopen {month}",
            )
            self.reopened.add(month)

    def _find(self, firefly_id: str) -> Transaction:
        page = _ok(
            list_transactions.sync_detailed(
                client=self.client, external_source=FIREFLY, external_ref=firefly_id
            ),
            f"find Firefly transaction {firefly_id}",
        )
        if not page.items:
            raise SettleError(f"Firefly transaction {firefly_id} isn't on the server")
        return page.items[0]

    def link(self, firefly_id: str, month: str) -> bool:
        """Marks the payment(s) in a migrated transaction as settling `month`."""
        t = self._find(firefly_id)
        payments = [i for i, s in enumerate(t.splits) if s.kind == Kind.TRANSFER]
        if not payments:
            raise SettleError(f"Firefly transaction {firefly_id} has no transfer to link")
        if all(t.splits[i].settles == month for i in payments):
            return False
        self._reopen(t.date.isoformat()[:7])
        r = get_transaction.sync_detailed(UUID(str(t.id)), client=self.client)
        _ok(r, f"Firefly transaction {firefly_id}")
        body = NewTransaction(
            date=t.date,
            description=t.description,
            splits=[
                _new_split(s, month if i in payments else None) for i, s in enumerate(t.splits)
            ],
            notes=t.notes,
            tags=t.tags,
            external_refs=t.external_refs,
        )
        _ok(
            replace_transaction.sync_detailed(
                UUID(str(t.id)), client=self.client, body=body, if_match=_etag(r)
            ),
            f"link Firefly transaction {firefly_id} to {month}",
        )
        return True

    def _wanted(self, month: str) -> tuple[list[Stipend], list[Funder]]:
        p = month_params(self.m, month)
        stipends = [
            Stipend(party_id=self.party_ids[name], amount=str(a)) for name, a in sorted(p.stipends)
        ]
        funders = [
            Funder(business_id=self.party_ids[b], funder_id=self.party_ids[f])
            for b, f in sorted(p.funders)
        ]
        return stipends, funders

    def set_params(self, month: str) -> bool:
        stipends, funders = self._wanted(month)
        settlement, etag = self._settlement(month)
        same = (
            settlement.params_from == month
            and {(str(x.party_id), Decimal(x.amount)) for x in settlement.stipends}
            == {(str(x.party_id), Decimal(x.amount)) for x in stipends}
            and {(str(x.business_id), str(x.funder_id)) for x in settlement.funders}
            == {(str(x.business_id), str(x.funder_id)) for x in funders}
        )
        if same:
            return False
        if settlement.closed:
            self._reopen(month)
            settlement, etag = self._settlement(month)
        body = SettlementParams(stipends=stipends, funders=funders)
        _ok(
            put_settlement.sync_detailed(month, client=self.client, body=body, if_match=etag),
            f"set {month}'s stipends and funders",
        )
        return True

    def close(self, month: str) -> bool:
        settlement, etag = self._settlement(month)
        if settlement.closed:
            return False
        _ok(close_month.sync_detailed(month, client=self.client, if_match=etag), f"close {month}")
        self.reopened.discard(month)
        return True

    # -- the pass -----------------------------------------------------------

    def run(self, progress: Callable[[str], None] | None = None) -> Outcome:
        say = progress or (lambda _: None)
        out = Outcome()
        for firefly_id, month in sorted(self.s.settles.items()):
            if self.link(firefly_id, month):
                out.linked.append(firefly_id)
                say(f"linked Firefly transaction {firefly_id} to {month}")
        last = self.m.closed_through or self.m.through.isoformat()[:7]
        for month in months(f"{self.m.year}-01", last):
            if self.set_params(month):
                out.params_set.append(month)
                say(f"set {month}'s stipends and funders")
        if self.m.closed_through:
            for month in months(f"{self.m.year}-01", self.m.closed_through):
                if self.close(month):
                    out.closed.append(month)
                    say(f"closed {month}")
        # Months reopened only to change them are closed again.
        for month in sorted(self.reopened):
            self.close(month)
        return out


# -- the §4.5 report ---------------------------------------------------------


@dataclass(frozen=True)
class Row:
    month: str
    party: str
    carried: Decimal
    claims: Decimal
    paid: Decimal
    stipend: Decimal
    implied: Decimal
    outstanding: Decimal


def history_rows(client: AuthenticatedClient, first: str, last: str) -> list[Row]:
    """Each person's edge for each month: what the statement computes."""
    parties = _ok(list_parties.sync_detailed(client=client, limit=500), "list parties")
    names = {str(p.id): p.name for p in parties.items}
    kinds = {str(p.id): p.kind for p in parties.items}
    h = _ok(get_history.sync_detailed(client=client, from_=first, to=last), "settlement history")
    rows = []
    for st in h.months:
        for e in st.edges:
            if kinds[str(e.child_id)] != PartyKind.PERSON:
                continue
            rows.append(
                Row(
                    month=st.month,
                    party=names[str(e.child_id)],
                    carried=Decimal(e.carried),
                    claims=Decimal(e.claims),
                    paid=Decimal(e.paid),
                    stipend=Decimal(e.stipend),
                    implied=Decimal(e.implied_stipend),
                    outstanding=Decimal(e.outstanding),
                )
            )
    return rows


def render(rows: list[Row], closed_through: str | None) -> str:
    """The implied stipend next to the stipend set (design §4.5). A difference
    in a settled month is either new (a label or payment link to look at, or a
    past mistake now carried forward) or a catch-up that pays down what earlier
    months carried. Months after the last closed one aren't settled yet, so
    they're shown but not marked."""
    lines = [
        "## Settlement check (implied stipends)",
        "",
        "Implied stipend = claims - paid; outstanding = carried + implied - stipend.",
        "A difference marked ⚠ is new: a label or a payment link to look at, or a past",
        "mistake carried forward. One marked ↺ pays down what earlier months carried.",
        "",
        "| Month | Person | Carried | Claims | Paid | Stipend | Implied | Difference "
        "| Outstanding |",
        "|---|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    flagged = 0
    for r in rows:
        diff = r.implied - r.stipend
        settled = closed_through is not None and r.month <= closed_through
        mark = ""
        if settled and diff != 0:
            if abs(r.outstanding) < abs(r.carried):
                mark = " ↺"
            else:
                mark = " ⚠"
                flagged += 1
        lines.append(
            f"| {r.month} | {r.party} | {r.carried} | {r.claims} | {r.paid} | {r.stipend} "
            f"| {r.implied} | {diff}{mark} | {r.outstanding} |"
        )
    lines += ["", f"Months to look at: {flagged}"]
    return "\n".join(lines)
