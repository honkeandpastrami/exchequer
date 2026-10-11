"""A fictional household (design §16.4) and Firefly data built for it."""

from __future__ import annotations

from datetime import date
from typing import Any

import pytest

from firefly_migrate import mapping

RAW: dict[str, Any] = {
    "firefly": {"api_base": "https://firefly.example.com/api/v1", "token_file": "/dev/null"},
    "exchequer": {"url": "http://127.0.0.1:7438", "token_file": "/dev/null"},
    "scope": {"year": 2026, "through": date(2026, 9, 6), "closed_through": "2026-08"},
    "parties": {"household": "Household", "people": ["Alex", "Sam"], "businesses": ["Studio"]},
    "commodities": {
        "USD": {"kind": "fiat", "precision": 2},
        "ACME": {"kind": "security", "precision": 3},
    },
    "accounts": [
        {"name": "Alex Checking", "firefly": [1], "kind": "checking", "owner": "Alex"},
        {"name": "Alex Credit", "firefly": [2], "kind": "credit_card", "owner": "Alex"},
        {"name": "Alex Brokerage", "firefly": [3, 4], "kind": "brokerage", "owner": "Alex"},
        {"name": "Joint Savings", "firefly": [5], "kind": "savings", "owner": "Household"},
        {"name": "Mortgage", "firefly": [6], "kind": "mortgage", "owner": "Household"},
        {"name": "Old Account", "firefly": [7], "kind": "other", "owner": "Sam", "archived": True},
    ],
    "purpose": {
        "default": "owner",
        "budgets": {"Shared": "Household", "Alex's": "Alex", "Studio": "Studio"},
        "tags": {"Shared": "Household"},
    },
    "counterparties": {"cash_account_by_destination": {"Alex Brokerage": "Broker Inc"}},
    "categories": {"drop": ["Card Payment"]},
    "tags": {"retire": ["Shared"], "import_batch_prefix": "batch "},
}


@pytest.fixture
def m() -> mapping.Mapping:
    return mapping.parse(RAW)


def account(fid: int, kind: str, name: str, currency: str = "USD", vb: str = "0") -> dict:
    return {
        "id": str(fid),
        "attributes": {
            "type": kind,
            "name": name,
            "currency_code": currency,
            "virtual_balance": vb,
        },
    }


ACCOUNTS = [
    account(1, "asset", "Alex Checking"),
    account(2, "asset", "Alex Credit", vb="2500.00"),
    account(3, "asset", "Brokerage ACME", currency="ACME"),
    account(4, "asset", "Brokerage USD"),
    account(5, "asset", "Joint Savings"),
    account(6, "liabilities", "Mortgage"),
    account(7, "asset", "Old Account"),
]

TYPES = {
    1: "Asset account",
    2: "Asset account",
    3: "Asset account",
    4: "Asset account",
    5: "Asset account",
    6: "Mortgage",
    7: "Asset account",
}


def journal(
    source: int | str,
    destination: int | str,
    amount: str,
    *,
    type: str = "withdrawal",
    currency: str = "USD",
    foreign: tuple[str, str] | None = None,
    budget: str | None = None,
    category: str | None = None,
    tags: list[str] | None = None,
    description: str = "something",
) -> dict:
    """`source`/`destination`: a Firefly account ID (owned), or
    "Expense:<name>", "Revenue:<name>", "Cash", "Initial"."""

    def side(x: int | str) -> tuple[str, str, str]:
        if isinstance(x, int):
            return str(x), TYPES[x], f"account {x}"
        if x == "Cash":
            return "900", "Cash account", "Cash account"
        if x == "Initial":
            return "901", "Initial balance account", "Initial balance"
        kind, name = x.split(":")
        return str(1000 + sum(map(ord, name))), f"{kind} account", name

    sid, stype, sname = side(source)
    did, dtype, dname = side(destination)
    return {
        "transaction_journal_id": "0",
        "type": type,
        "date": "",
        "order": 0,
        "description": description,
        "notes": None,
        "source_id": sid,
        "source_type": stype,
        "source_name": sname,
        "destination_id": did,
        "destination_type": dtype,
        "destination_name": dname,
        "amount": amount,
        "currency_code": currency,
        "foreign_amount": foreign[0] if foreign else None,
        "foreign_currency_code": foreign[1] if foreign else None,
        "budget_name": budget,
        "category_name": category,
        "tags": tags or [],
    }


def group(gid: int, day: str, *journals: dict, title: str | None = None) -> dict:
    for j in journals:
        j["date"] = f"{day}T00:00:00-07:00"
    return {
        "id": str(gid),
        "attributes": {"group_title": title, "transactions": list(journals)},
    }
