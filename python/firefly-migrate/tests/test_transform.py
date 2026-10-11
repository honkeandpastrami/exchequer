"""Firefly → Exchequer plan, on the fictional household's Firefly data."""

from datetime import date
from decimal import Decimal

from firefly_migrate import transform
from tests.conftest import ACCOUNTS, group, journal


def plan(m, *groups):
    return transform.plan(m, ACCOUNTS, list(groups))


def only(p):
    """The one migrated transaction, ignoring opening balances."""
    [t] = [t for t in p.transactions if t.firefly_id]
    return t


def test_expense_purpose_from_budget_tag_or_owner(m):
    p = plan(
        m,
        group(10, "2026-03-01", journal(2, "Expense:Grocer", "80.00", budget="Shared")),
        group(11, "2026-03-02", journal(2, "Expense:Grocer", "5.00", tags=["Shared"])),
        group(12, "2026-03-03", journal(2, "Expense:Books", "9.00")),
    )
    purposes = {t.firefly_id: t.splits[0].purpose for t in p.transactions}
    assert purposes == {"10": "Household", "11": "Household", "12": "Alex"}
    t = p.transactions[0]
    assert t.splits[0].from_.account == "Alex Credit"
    assert t.splits[0].to.counterparty == "Grocer"
    assert t.splits[0].from_.quantity == Decimal("80.00")
    assert p.report.counts["purpose from owner"] == 1


def test_conflicting_labels_use_the_budget_and_are_reported(m):
    p = plan(
        m,
        group(
            10, "2026-03-01", journal(2, "Expense:Fabric", "30", budget="Studio", tags=["Shared"])
        ),
    )
    assert only(p).splits[0].purpose == "Studio"
    [(what, ids)] = p.report.conflicts.items()
    assert "Studio used" in what
    assert ids == ["10"]


def test_retired_and_import_tags_are_not_carried(m):
    j = journal(2, "Expense:Grocer", "1", tags=["Shared", "batch 2026-01-01", "Trip"])
    assert only(plan(m, group(10, "2026-03-01", j))).tags == ["Trip"]


def test_brokerage_accounts_collapse_and_a_sale_is_a_conversion(m):
    sale = journal(3, 4, "10", type="transfer", currency="ACME", foreign=("1500.00", "USD"))
    split = only(plan(m, group(10, "2026-03-01", sale))).splits[0]
    assert (split.from_.account, split.from_.commodity, split.from_.quantity) == (
        "Alex Brokerage",
        "ACME",
        Decimal("10"),
    )
    assert (split.to.account, split.to.commodity, split.to.quantity) == (
        "Alex Brokerage",
        "USD",
        Decimal("1500.00"),
    )
    assert split.purpose is None
    assert split.category is None


def test_liquidation_defaults_to_household(m):
    j = journal(4, 1, "1000.00", type="transfer", category="Card Payment")
    split = only(plan(m, group(10, "2026-03-01", j))).splits[0]
    assert split.purpose == "Household"
    assert split.category is None  # transfer-only categories don't migrate
    assert plan(m, group(10, "2026-03-01", j)).report.counts["liquidations"] == 1


def test_payment_into_a_liability_becomes_a_transfer(m):
    j = journal(5, 6, "2000.00", budget="Shared", category="Mortgage")
    p = plan(m, group(10, "2026-03-01", j))
    split = only(p).splits[0]
    assert (split.from_.account, split.to.account) == ("Joint Savings", "Mortgage")
    assert (split.purpose, split.category) == (None, None)
    assert set(p.report.dropped) == {
        "budget 'Shared' on a transfer",
        "category 'Mortgage' on a transfer",
    }


def test_dividends_from_the_cash_account_come_from_the_institution(m):
    j = journal("Cash", 4, "12.34", type="deposit")
    split = only(plan(m, group(10, "2026-03-01", j))).splits[0]
    assert split.from_.counterparty == "Broker Inc"
    assert split.purpose is None  # investment activity has none


def test_float_damage_is_rounded_and_reported(m):
    p = plan(m, group(10, "2026-03-01", journal(1, "Expense:X", "1234.5600000000001")))
    assert only(p).splits[0].from_.quantity == Decimal("1234.56")
    assert p.report.rounded == [
        "Firefly transaction 10 (2026-03-01, 'something'): 1234.5600000000001 → 1234.56"
    ]


def test_opening_balances_sum_everything_before_the_year(m):
    p = plan(
        m,
        group(1, "2024-05-01", journal("Initial", 1, "100.00", type="opening balance")),
        group(2, "2025-06-01", journal(1, "Expense:X", "30.555")),  # rounded quietly
        group(3, "2025-07-01", journal(2, "Expense:X", "40.00")),
        group(4, "2026-01-15", journal(1, "Expense:X", "1.00")),
    )
    openings = [t for t in p.transactions if not t.firefly_id]
    assert all(t.date == date(2026, 1, 1) for t in openings)
    legs = {
        (
            t.splits[0].from_.account or t.splits[0].from_.system,
            t.splits[0].to.account or t.splits[0].to.system,
            t.splits[0].to.quantity,
        )
        for t in openings
    }
    assert legs == {
        ("opening_balances", "Alex Checking", Decimal("69.44")),
        ("Alex Credit", "opening_balances", Decimal("40.00")),
    }
    assert p.report.rounded == []
    assert p.report.counts["rounded before 2026"] == 1


def test_after_the_cutoff_is_skipped_and_counted(m):
    p = plan(m, group(10, "2026-09-07", journal(1, "Expense:X", "1")))
    assert [t for t in p.transactions if t.firefly_id] == []
    assert p.report.skipped_after_cutoff == 1


def test_split_groups_keep_their_splits(m):
    g = group(
        10,
        "2026-03-01",
        journal(2, "Expense:Store", "120", category="Groceries", description="food"),
        journal(2, "Expense:Store", "45", category="Home", description="supplies"),
        title="Store run",
    )
    t = only(plan(m, g))
    assert t.description == "Store run"
    assert [(s.category, s.description) for s in t.splits] == [
        ("Groceries", "food"),
        ("Home", "supplies"),
    ]


def test_counterparties_merge_by_normalized_name(m):
    p = plan(
        m,
        group(10, "2026-03-01", journal(2, "Expense:Big Store", "1")),
        group(11, "2026-03-02", journal(2, "Expense:Big Store", "1")),
        group(12, "2026-03-03", journal("Revenue:big  store", 2, "1", type="deposit")),
    )
    assert p.counterparties == ["Big Store"]


def test_unmapped_owned_accounts_are_errors(m):
    j = journal(1, "Expense:X", "1")
    j["destination_type"] = "Asset account"
    j["destination_id"] = "99"
    p = plan(m, group(10, "2026-03-01", j))
    assert len(p.report.errors) == 1
    assert "not in the mapping" in p.report.errors[0]


def test_credit_limits_come_from_the_virtual_balance(m):
    p = plan(m)
    assert p.accounts["Alex Credit"] == transform.CreditLimit("USD", Decimal("2500.00"))
    assert p.accounts["Alex Checking"] is None
