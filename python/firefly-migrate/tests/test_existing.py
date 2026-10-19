"""How a year's plan fits a server that already holds data (design §11)."""

import copy
from dataclasses import replace
from datetime import date
from decimal import Decimal
from itertools import count
from uuid import UUID

from firefly_migrate import mapping, transform
from firefly_migrate.existing import Held, Ledger, fit, is_opening
from firefly_migrate.transform import OPENING_BALANCES, Leg, Split, Transaction
from tests.conftest import ACCOUNTS, RAW, group, journal

ids = count(1)


def year(y: int, through: date):
    raw = copy.deepcopy(RAW)
    raw["scope"] = {"year": y, "through": through}
    return mapping.parse(raw)


M2025 = year(2025, date(2025, 12, 31))
GROUPS = [
    group(1, "2024-06-01", journal("Initial", 1, "1000.00", type="deposit", category=None)),
    group(10, "2025-03-01", journal(2, "Expense:Grocer", "80.00")),
    group(11, "2025-03-02", journal(1, 2, "500.00", type="transfer", external_id="R11")),
    group(12, "2025-12-30", journal("Revenue:Employer", 1, "2000.00", type="deposit")),
]


def plan(m=M2025, groups=GROUPS):
    return transform.plan(m, ACCOUNTS, groups)


def held(t: Transaction, **changes) -> Held:
    rows = changes.pop("rows", frozenset())
    return Held(UUID(int=next(ids)), replace(t, **changes), rows)


def ledger(m, *transactions: Held, closed=()) -> Ledger:
    """A server with the mapping's reference data and these transactions."""
    kinds = {m.household: "household", **dict.fromkeys(m.people, "person")}
    kinds.update(dict.fromkeys(m.businesses, "business"))
    return Ledger(
        transactions=list(transactions),
        parties={n: (UUID(int=next(ids)), k) for n, k in kinds.items()},
        commodities={
            c: (UUID(int=next(ids)), s.kind, s.precision) for c, s in m.commodities.items()
        },
        accounts={a.key: (UUID(int=next(ids)), a.kind) for a in m.accounts},
        counterparties={"grocer": UUID(int=next(ids)), "employer": UUID(int=next(ids))},
        categories={"Groceries": UUID(int=next(ids))},
        closed=set(closed),
    )


def opening(account: str, quantity: str, day: date) -> Transaction:
    system = Leg("USD", Decimal(quantity), system=OPENING_BALANCES)
    mine = Leg("USD", Decimal(quantity), account=account)
    return Transaction(day, "Opening balance", [Split(from_=system, to=mine)])


def next_year(*extra: Held) -> list[Held]:
    """2026 on the server: opening balances where 2025 ends, and one more."""
    jan1 = date(2026, 1, 1)
    later = Transaction(
        date(2026, 2, 1),
        "Coffee",
        [Split(Leg("USD", Decimal("4.00"), account="Alex · Checking"), Leg("USD", Decimal(4)))],
        firefly_id="99",
    )
    return [
        held(opening("Alex · Checking", "2500.00", jan1)),
        held(opening("Alex · Credit", "420.00", jan1)),
        held(later),
        *extra,
    ]


def test_an_empty_server_takes_everything():
    p = plan()
    f = fit(M2025, p, Ledger(), overlap=False)
    assert not f.stops
    assert f.writes == p.transactions
    assert f.create["accounts"] == list(p.accounts)
    assert f.create["parties"] == ["Household", "Alex", "Sam", "Studio"]
    assert not f.replace


def test_a_year_before_the_server_replaces_its_first_opening_balances():
    p = plan()
    lg = ledger(M2025, *next_year(), closed={"2026-01"})
    f = fit(M2025, p, lg, overlap=False)
    assert not f.stops, f.stops
    assert f.writes == p.transactions
    assert sorted(str(h.t.splits[0].to.account) for h in f.replace) == [
        "Alex · Checking",
        "Alex · Credit",
    ]
    assert f.reopen == "2026-01"


def test_the_year_must_end_where_the_server_starts_to_the_cent():
    lg = ledger(M2025, *next_year())
    lg.transactions[1] = held(opening("Alex · Credit", "420.01", date(2026, 1, 1)))
    f = fit(M2025, plan(), lg, overlap=False)
    assert f.stops == [
        "Alex · Credit USD: the year ends at 420.00, "
        "the server's opening balance on 2026-01-01 is 420.01"
    ]


def test_later_data_without_opening_balances_stops_it():
    lg = ledger(M2025, *next_year()[2:])
    f = fit(M2025, plan(), lg, overlap=False)
    assert len(f.stops) == 2
    assert all("has no opening balance on 2026-01-01 to replace" in s for s in f.stops)


def test_a_year_already_before_the_server_fits_again_unchanged():
    p = plan()
    done = [held(t) for t in p.transactions]
    lg = ledger(M2025, *done, *next_year()[2:])
    f = fit(M2025, p, lg, overlap=True)
    assert not f.stops, f.stops
    assert (f.already, f.writes, f.replace) == (len(p.transactions), [], [])


def test_a_year_after_the_server_starts_from_its_balances():
    m2026 = year(2026, date(2026, 3, 31))
    groups = [*GROUPS, group(20, "2026-02-01", journal(1, "Expense:Grocer", "4.00"))]
    p = plan(m2026, groups)
    earlier = [held(t) for t in plan().transactions]  # 2025, as migrated before
    f = fit(m2026, p, ledger(m2026, *earlier), overlap=False)
    assert not f.stops, f.stops
    assert [t.firefly_id for t in f.writes] == ["20"]  # no opening balances

    earlier.pop()  # 2025's last deposit missing: the balances don't match
    f = fit(m2026, p, ledger(m2026, *earlier), overlap=False)
    assert f.stops == [
        "Alex · Checking USD on 2026-01-01: the server has 500.00, "
        "Firefly's opening balance is 2500.00"
    ]


def test_data_in_the_year_needs_overlap():
    p = plan()
    [grocer] = [t for t in p.transactions if t.firefly_id == "10"]
    lg = ledger(M2025, held(grocer), *next_year())
    f = fit(M2025, p, lg, overlap=False)
    assert f.stops[0].startswith("the server already has 1 transactions from 2025-01-01")
    assert f.stops[1:] == ["  Alex · Credit: 1 from 2025-03-01 to 2025-03-01"]
    assert not f.writes


def test_with_overlap_only_unchanged_transactions_are_skipped():
    p = plan()
    by_id = {t.firefly_id: t for t in p.transactions}
    lg = ledger(M2025, held(by_id["10"]), *next_year())
    f = fit(M2025, p, lg, overlap=True)
    assert not f.stops, f.stops
    assert f.already == 1
    assert "10" not in {t.firefly_id for t in f.writes}

    lg = ledger(M2025, held(by_id["10"], description="Grocer (edited)"), *next_year())
    f = fit(M2025, p, lg, overlap=True)
    assert f.stops == [
        "Firefly transaction 10 (2025-03-01, 'something', 80.00 USD on Alex · Credit) "
        "is on the server, changed: its description differ"
    ]


def test_with_overlap_near_duplicates_and_strangers_stop_it():
    p = plan()
    by_id = {t.firefly_id: t for t in p.transactions}
    typed = held(by_id["10"], date=date(2025, 3, 3), description="GROCER #12", firefly_id=None)
    stranger = held(by_id["10"], date=date(2025, 7, 1), firefly_id=None)
    f = fit(M2025, p, ledger(M2025, typed, stranger, *next_year()), overlap=True)
    assert f.stops == [
        "Firefly transaction 10 (2025-03-01, 'something', 80.00 USD on Alex · Credit) may "
        "already be on the server as a transaction (2025-03-03, 'GROCER #12', 80.00 USD on "
        "Alex · Credit)",
        "on the server, not in Firefly's 2025: "
        "a transaction (2025-07-01, 'something', 80.00 USD on Alex · Credit)",
    ]


def test_a_near_duplicate_just_after_the_year_stops_it():
    p = plan()
    by_id = {t.firefly_id: t for t in p.transactions}
    # Imported from a bank file, not migrated: no Firefly ID.
    imported = held(by_id["12"], date=date(2026, 1, 2), firefly_id=None)
    f = fit(M2025, p, ledger(M2025, *next_year(imported)), overlap=False)
    assert any("Firefly transaction 12" in s and "may already be" in s for s in f.stops)


def test_a_bank_row_already_on_the_server_stops_it():
    p = plan()
    imported = held(next_year()[2].t, firefly_id=None, rows=frozenset({("Alex · Checking", "R11")}))
    f = fit(M2025, p, ledger(M2025, *next_year(imported)), overlap=False)
    assert any(s.startswith("a bank row of Firefly transaction 11") for s in f.stops)


def test_closed_months_and_mismatched_reference_data_stop_it():
    lg = ledger(M2025, *next_year(), closed={"2025-03", "2026-01"})
    lg.accounts["Alex · Credit"] = (lg.accounts["Alex · Credit"][0], "checking")
    f = fit(M2025, plan(), lg, overlap=False)
    assert f.stops == [
        "account Alex · Credit is a checking account on the server, not a credit_card one",
        "the year writes into closed months: 2025-03",
    ]


def test_opening_balances_are_found_by_account_and_commodity():
    p = plan()
    [first] = [t for t in p.transactions if is_opening(t)]
    lg = ledger(M2025, held(first), *next_year())
    f = fit(M2025, p, lg, overlap=True)
    assert not f.stops, f.stops
    assert f.already == 1
    assert not [t for t in f.writes if is_opening(t)]


def test_labels_firefly_lacks_come_from_the_servers_copy():
    groups = [*GROUPS, group(14, "2025-05-01", journal(2, "Expense:Grocer", "7.00", budget=None))]
    p = plan(M2025, groups)
    [unlabeled] = [t for t in p.transactions if t.missing]
    f = fit(M2025, p, Ledger(), overlap=False)
    assert f.stops == [unlabeled.missing[0][2]]  # no copy: as good as an error

    later = next_year()
    later[1] = held(opening("Alex · Credit", "413.00", date(2026, 1, 1)))  # 7.00 less
    copy_ = held(unlabeled, splits=[replace(unlabeled.splits[0], purpose="Alex")], missing=[])
    f = fit(M2025, p, ledger(M2025, copy_, *later), overlap=True)
    assert not f.stops, f.stops
    assert (f.labeled, f.already) == (1, 1)

    copy_ = held(unlabeled, missing=[])  # the server's copy has no purpose either
    f = fit(M2025, p, ledger(M2025, copy_, *later), overlap=True)
    assert f.stops == [f"{unlabeled.missing[0][2]}, and the server's copy has none either"]
