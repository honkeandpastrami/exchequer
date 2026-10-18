"""The operator's mapping file (design §11): TOML, hand-edited, never committed.

`load` reads and validates it; every problem is reported at once, so one run
fixes them all.
"""

from __future__ import annotations

import re
import tomllib
from collections.abc import Sequence
from dataclasses import dataclass, field
from datetime import date
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

ACCOUNT_KINDS = frozenset(
    {"checking", "savings", "credit_card", "brokerage", "mortgage", "cash", "other"}
)
COMMODITY_KINDS = frozenset({"fiat", "security", "fund", "crypto", "other"})


class MappingError(ValueError):
    def __init__(self, problems: list[str]) -> None:
        self.problems = problems
        super().__init__("invalid mapping:\n" + "\n".join(f"  - {p}" for p in problems))


@dataclass(frozen=True)
class Commodity:
    code: str
    kind: str
    precision: int


@dataclass(frozen=True)
class Account:
    name: str
    firefly: tuple[int, ...]
    kind: str
    owner: str
    # None: the server's default (brokerages and mortgages aren't liquid,
    # everything else is). A mortgage never is.
    liquid: bool | None = None
    archived: bool = False
    # A non-liquid account's categories for money into it (an expense) and
    # out of it (income), e.g. Invest and Divest (design §3.2).
    category_in: str | None = None
    category_out: str | None = None

    @property
    def key(self) -> str:
        """How the tool tells accounts apart: names are unique per owner."""
        return account_key(self.owner, self.name)

    @property
    def is_liquid(self) -> bool:
        if self.kind == "mortgage":
            return False
        return self.liquid if self.liquid is not None else self.kind != "brokerage"


def account_key(owner: str, name: str) -> str:
    return f"{owner} · {name}"


def find_account(accounts: Sequence[Account], ref: str) -> Account | str:
    """An account named in the mapping: `Owner · Name`, or just the name when
    only one owner has an account by that name. A string says what's wrong."""
    matches = [a for a in accounts if ref in (a.key, a.name)]
    if len({a.key for a in matches}) == 1:
        return matches[0]
    if not matches:
        return "is not an account"
    return f"is ambiguous: write it as one of {sorted(a.key for a in matches)}"


MONTH = re.compile(r"^[0-9]{4}-(0[1-9]|1[0-2])$")


@dataclass(frozen=True)
class Settlement:
    """The settlement pass (design §4.3, §4.5): parameters carried forward
    from the month they're given for, and payments that settle a month other
    than the default (the one before they're made)."""

    # person -> {month "YYYY-MM": stipend}
    stipends: dict[str, dict[str, Decimal]]
    # business -> {month: funder}
    funders: dict[str, dict[str, str]]
    # Firefly transaction group ID -> month it settles
    settles: dict[str, str]


@dataclass(frozen=True)
class Mapping:
    firefly_api_base: str
    firefly_token_file: Path
    request_timeout: int
    exchequer_url: str
    exchequer_token_file: Path
    year: int
    through: date
    closed_through: str | None
    household: str
    people: tuple[str, ...]
    businesses: tuple[str, ...]
    commodities: dict[str, Commodity]
    accounts: tuple[Account, ...]
    budgets: dict[str, str]
    purpose_tags: dict[str, str]
    cash_account_by_destination: dict[str, str]
    drop_categories: frozenset[str]
    retire_tags: frozenset[str]
    import_batch_prefix: str | None
    settlement: Settlement | None = None
    by_firefly_id: dict[int, Account] = field(init=False, repr=False)

    def __post_init__(self) -> None:
        index = {fid: a for a in self.accounts for fid in a.firefly}
        object.__setattr__(self, "by_firefly_id", index)

    @property
    def parties(self) -> tuple[str, ...]:
        return (self.household, *self.people, *self.businesses)

    @property
    def start(self) -> date:
        return date(self.year, 1, 1)


def read_token(path: Path) -> str:
    """A token file holds the token alone; surrounding whitespace is ignored."""
    token = path.expanduser().read_text().strip()
    if not token:
        raise ValueError(f"{path} is empty")
    return token


def load(path: Path) -> Mapping:
    with path.open("rb") as f:
        raw = tomllib.load(f)
    return parse(raw)


def parse(raw: dict[str, Any]) -> Mapping:
    problems: list[str] = []

    def need(table: dict[str, Any], key: str, where: str) -> Any:
        if key not in table:
            problems.append(f"{where}.{key} is missing")
            return None
        return table[key]

    firefly = raw.get("firefly", {})
    exchequer = raw.get("exchequer", {})
    scope = raw.get("scope", {})
    parties = raw.get("parties", {})
    purpose = raw.get("purpose", {})

    year = need(scope, "year", "scope")
    through = need(scope, "through", "scope")
    if isinstance(year, int) and isinstance(through, date) and through.year != year:
        problems.append(f"scope.through ({through}) is not in scope.year ({year})")
    if through is not None and not isinstance(through, date):
        problems.append("scope.through must be a date, e.g. 2026-09-06")

    closed_through = scope.get("closed_through")
    if closed_through is not None and not (
        isinstance(closed_through, str) and MONTH.match(closed_through)
    ):
        problems.append('scope.closed_through must be a month, e.g. "2026-08"')

    household = need(parties, "household", "parties")
    people = tuple(parties.get("people", []))
    businesses = tuple(parties.get("businesses", []))
    known = {household, *people, *businesses}
    if len(known) != 1 + len(people) + len(businesses):
        problems.append("parties: names must be unique")

    commodities: dict[str, Commodity] = {}
    for code, spec in raw.get("commodities", {}).items():
        kind, precision = spec.get("kind"), spec.get("precision")
        if kind not in COMMODITY_KINDS:
            problems.append(f"commodities.{code}: kind must be one of {sorted(COMMODITY_KINDS)}")
        if not isinstance(precision, int) or not 0 <= precision <= 18:
            problems.append(f"commodities.{code}: precision must be 0-18")
        commodities[code] = Commodity(code, kind, precision)

    accounts: list[Account] = []
    seen_ids: dict[int, str] = {}
    for i, a in enumerate(raw.get("accounts", [])):
        name = a.get("name") or f"accounts[{i}]"
        ids = tuple(a.get("firefly", []))
        if not ids or not all(isinstance(x, int) for x in ids):
            problems.append(f"account {name}: firefly must list Firefly account IDs")
        for fid in ids:
            if fid in seen_ids:
                problems.append(f"Firefly account {fid} is mapped twice ({seen_ids[fid]}, {name})")
            seen_ids[fid] = name
        if a.get("kind") not in ACCOUNT_KINDS:
            problems.append(f"account {name}: kind must be one of {sorted(ACCOUNT_KINDS)}")
        if a.get("owner") not in known:
            problems.append(f"account {name}: owner {a.get('owner')!r} is not a party")
        if a.get("kind") == "mortgage" and a.get("liquid"):
            problems.append(f"account {name}: a mortgage is never liquid")
        account = Account(
            name=name,
            firefly=ids,
            kind=a.get("kind", ""),
            owner=a.get("owner", ""),
            liquid=a.get("liquid"),
            archived=bool(a.get("archived", False)),
            category_in=a.get("category_in"),
            category_out=a.get("category_out"),
        )
        for key in ("category_in", "category_out"):
            value = a.get(key)
            if value is not None and (not isinstance(value, str) or not value):
                problems.append(f"account {name}: {key} must be a category name")
            elif value is not None and account.is_liquid:
                problems.append(f"account {name}: only a non-liquid account takes {key}")
        accounts.append(account)
    keys = [a.key for a in accounts]
    for k in sorted({k for k in keys if keys.count(k) > 1}):
        problems.append(f"accounts: {k} appears twice (names are unique per owner)")

    budgets = dict(purpose.get("budgets", {}))
    purpose_tags = dict(purpose.get("tags", {}))
    for label, party in [*budgets.items(), *purpose_tags.items()]:
        if party not in known:
            problems.append(f"purpose: {label!r} maps to {party!r}, which is not a party")
    if purpose.get("default", "owner") != "owner":
        problems.append('purpose.default: only "owner" is supported')

    cash: dict[str, str] = {}
    raw_cash = raw.get("counterparties", {}).get("cash_account_by_destination", {})
    for dest, institution in dict(raw_cash).items():
        found = find_account(accounts, dest)
        if isinstance(found, str):
            problems.append(f"counterparties.cash_account_by_destination: {dest!r} {found}")
        else:
            cash[found.key] = institution

    settlement = _settlement(raw.get("settlement"), set(people), set(businesses), household)
    if isinstance(settlement, list):
        problems += settlement
        settlement = None

    if problems:
        raise MappingError(problems)

    return Mapping(
        firefly_api_base=firefly.get("api_base", ""),
        firefly_token_file=Path(firefly.get("token_file", "")),
        request_timeout=int(firefly.get("request_timeout", 120)),
        exchequer_url=exchequer.get("url", ""),
        exchequer_token_file=Path(exchequer.get("token_file", "")),
        year=year,
        through=through,
        closed_through=closed_through,
        household=household,
        people=people,
        businesses=businesses,
        commodities=commodities,
        accounts=tuple(accounts),
        budgets=budgets,
        purpose_tags=purpose_tags,
        cash_account_by_destination=cash,
        drop_categories=frozenset(raw.get("categories", {}).get("drop", [])),
        retire_tags=frozenset(raw.get("tags", {}).get("retire", [])),
        import_batch_prefix=raw.get("tags", {}).get("import_batch_prefix"),
        settlement=settlement,
    )


def _settlement(
    raw: dict[str, Any] | None, people: set[str], businesses: set[str], household: str
) -> Settlement | list[str] | None:
    """The `[settlement]` section, or its problems."""
    if raw is None:
        return None
    problems: list[str] = []
    if "currency" in raw:
        problems.append("settlement.currency: remove it, settlement is always in USD")

    def months(table: Any, where: str) -> dict[str, Any]:
        if not isinstance(table, dict):
            problems.append(f"{where} must be a table of months")
            return {}
        for m in table:
            if not MONTH.match(m):
                problems.append(f"{where}: {m!r} is not a month (YYYY-MM)")
        return dict(table)

    stipends: dict[str, dict[str, Decimal]] = {}
    for person, table in raw.get("stipends", {}).items():
        if person not in people:
            problems.append(f"settlement.stipends: {person!r} is not a person")
        stipends[person] = {}
        for m, amount in months(table, f"settlement.stipends.{person}").items():
            # Strings only: TOML numbers with decimals are floats.
            try:
                value = Decimal(amount) if isinstance(amount, str) else None
            except InvalidOperation:
                value = None
            if value is None or value < 0:
                problems.append(
                    f'settlement.stipends.{person}.{m}: give the amount as a string, e.g. "650.00"'
                )
                continue
            stipends[person][m] = value

    funders: dict[str, dict[str, str]] = {}
    for business, table in raw.get("funders", {}).items():
        if business not in businesses:
            problems.append(f"settlement.funders: {business!r} is not a business")
        funders[business] = months(table, f"settlement.funders.{business}")
        for m, funder in funders[business].items():
            if funder != household and funder not in people:
                problems.append(
                    f"settlement.funders.{business}.{m}: {funder!r} "
                    "is not the household or a person"
                )

    settles = raw.get("settles", {})
    if not isinstance(settles, dict):
        problems.append("settlement.settles must map Firefly transaction IDs to months")
        settles = {}
    for ref, m in settles.items():
        if not str(ref).isdigit():
            problems.append(f"settlement.settles: {ref!r} is not a Firefly transaction ID")
        if not isinstance(m, str) or not MONTH.match(m):
            problems.append(f"settlement.settles.{ref}: {m!r} is not a month (YYYY-MM)")

    if problems:
        return problems
    return Settlement(
        stipends=stipends,
        funders=funders,
        settles={str(k): str(v) for k, v in settles.items()},
    )
