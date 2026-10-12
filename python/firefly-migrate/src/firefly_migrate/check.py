"""The acceptance check (design §11): Firefly vs. what Exchequer now holds.

Balances per account and commodity at each month end, and net totals per
category and month. Exchequer's side is read back through its API and summed
here, so the check covers the write as well as the transform. The settlement
comparison (§4.5) arrives with settlement itself (build step 4).
"""

from __future__ import annotations

import calendar
from collections import defaultdict
from collections.abc import Collection
from dataclasses import dataclass
from datetime import date
from decimal import Decimal
from typing import Any

from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.accounts import accounts_list as list_accounts
from exchequer_sdk.api.categories import categories_list as list_categories
from exchequer_sdk.api.commodities import commodities_list as list_commodities
from exchequer_sdk.api.parties import parties_list as list_parties
from exchequer_sdk.api.transactions import transactions_list as list_transactions
from exchequer_sdk.models import Problem
from exchequer_sdk.types import UNSET
from firefly_migrate.firefly import FireflyReader
from firefly_migrate.mapping import Mapping, account_key
from firefly_migrate.transform import exact


@dataclass(frozen=True)
class Difference:
    what: str
    firefly: Decimal
    exchequer: Decimal

    def __str__(self) -> str:
        delta = self.exchequer - self.firefly
        return f"{self.what}: Firefly {self.firefly}, Exchequer {self.exchequer} (Δ {delta})"


def check_dates(m: Mapping) -> list[date]:
    """Each month end of the migrated year up to the cutoff, and the cutoff."""
    out = []
    for month in range(1, m.through.month):
        out.append(date(m.year, month, calendar.monthrange(m.year, month)[1]))
    out.append(m.through)
    return out


def _all(fetch: Any) -> list[Any]:
    items, cursor = [], UNSET
    while True:
        page = fetch(cursor)
        if page is None or isinstance(page, Problem):
            raise RuntimeError("couldn't read from Exchequer")
        items += page.items
        if not page.next_cursor:
            return items
        cursor = page.next_cursor


def accounts_by_key(client: AuthenticatedClient) -> dict[str, Any]:
    """Exchequer's accounts by `Owner · Name`."""
    parties = {
        str(p.id): p.name
        for p in _all(lambda c: list_parties.sync(client=client, limit=500, cursor=c))
    }
    return {
        account_key(parties[str(a.owner_id)], a.name): a
        for a in _all(lambda c: list_accounts.sync(client=client, limit=500, cursor=c))
    }


def exchequer_view(
    client: AuthenticatedClient, dates: list[date]
) -> tuple[dict[tuple[str, str, date], Decimal], dict[tuple[str, str], Decimal]]:
    """Balances by (account, commodity, date) and net category totals by
    (category, YYYY-MM): expenses negative, income positive."""
    parties = {
        str(p.id): p.name
        for p in _all(lambda c: list_parties.sync(client=client, limit=500, cursor=c))
    }
    accounts = {
        str(a.id): account_key(parties[str(a.owner_id)], a.name)
        for a in _all(lambda c: list_accounts.sync(client=client, limit=500, cursor=c))
    }
    commodities = {
        str(x.id): x.code
        for x in _all(lambda c: list_commodities.sync(client=client, limit=500, cursor=c))
    }
    categories = {
        str(x.id): x.name
        for x in _all(lambda c: list_categories.sync(client=client, limit=500, cursor=c))
    }
    txs = _all(lambda c: list_transactions.sync(client=client, to=dates[-1], limit=500, cursor=c))

    balances: dict[tuple[str, str, date], Decimal] = defaultdict(Decimal)
    totals: dict[tuple[str, str], Decimal] = defaultdict(Decimal)
    for t in txs:
        for s in t.splits:
            moves = []
            if s.from_.account_id:
                moves.append(
                    (s.from_.account_id, s.from_.commodity_id, -exact(s.from_.quantity, ""))
                )
            if s.to.account_id:
                moves.append((s.to.account_id, s.to.commodity_id, exact(s.to.quantity, "")))
            for account, commodity, qty in moves:
                key = (accounts[str(account)], commodities[str(commodity)])
                for d in dates:
                    if t.date <= d:
                        balances[(*key, d)] += qty
            category = s.category_id
            if category:
                kind = s.kind.value
                sign = -1 if kind == "expense" else 1
                leg = s.from_ if kind == "expense" else s.to
                code = commodities[str(leg.commodity_id)]
                month = t.date.strftime("%Y-%m")
                totals[(f"{categories[str(category)]} [{code}]", month)] += sign * exact(
                    leg.quantity, ""
                )
    return balances, totals


def firefly_balances(
    ff: FireflyReader, m: Mapping, dates: list[date], migrated: Collection[str]
) -> dict[tuple[str, str, date], Decimal]:
    """Firefly's own balances, for the `migrated` accounts. Firefly includes a
    credit card's virtual balance (its limit) in what it reports, so that is
    subtracted (§11)."""
    out: dict[tuple[str, str, date], Decimal] = defaultdict(Decimal)
    for a in m.accounts:
        if a.key not in migrated:
            continue
        for fid in a.firefly:
            for d in dates:
                attrs = ff.account_on(fid, d)["attributes"]
                balance = exact(attrs["current_balance"], a.key)
                virtual = attrs.get("virtual_balance")
                if virtual not in (None, ""):
                    balance -= exact(virtual, a.key)
                out[(a.key, attrs["currency_code"], d)] += balance
    return out


def firefly_category_totals(
    m: Mapping, groups: list[dict[str, Any]]
) -> dict[tuple[str, str], Decimal]:
    """Net category totals of Firefly's withdrawals and deposits in scope."""
    totals: dict[tuple[str, str], Decimal] = defaultdict(Decimal)
    for g in groups:
        for t in g["attributes"]["transactions"]:
            day = date.fromisoformat(t["date"][:10])
            category = t.get("category_name")
            if not (m.start <= day <= m.through) or not category:
                continue
            if category in m.drop_categories or t["type"] not in ("withdrawal", "deposit"):
                continue
            # Between two owned accounts it's a transfer in Exchequer, with no
            # category (e.g. a withdrawal into a liability): listed in the report.
            owned = m.by_firefly_id
            if int(t["source_id"]) in owned and int(t["destination_id"]) in owned:
                continue
            sign = -1 if t["type"] == "withdrawal" else 1
            key = (f"{category} [{t['currency_code']}]", day.strftime("%Y-%m"))
            totals[key] += sign * exact(t["amount"], g["id"])
    return totals


def compare(
    firefly: dict[Any, Decimal], exchequer: dict[Any, Decimal], precision: int = 3
) -> list[Difference]:
    diffs = []
    for key in sorted(set(firefly) | set(exchequer), key=str):
        f = firefly.get(key, Decimal(0)).quantize(Decimal(1).scaleb(-precision))
        e = exchequer.get(key, Decimal(0)).quantize(Decimal(1).scaleb(-precision))
        if f != e:
            diffs.append(Difference(" / ".join(str(k) for k in key), f, e))
    return diffs
