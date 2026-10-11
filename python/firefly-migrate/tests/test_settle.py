"""The settlement pass's pure parts: schedules carried forward, the mapping's
`[settlement]` section, and the implied-stipend report."""

import copy
from decimal import Decimal

import pytest

from firefly_migrate import mapping
from firefly_migrate.settle import Row, carried, month_params, months, render

from .conftest import RAW

SETTLEMENT = {
    "currency": "USD",
    "stipends": {"Alex": {"2026-01": "500.00", "2026-06": "600.00"}, "Sam": {"2026-03": "0"}},
    "funders": {"Studio": {"2026-01": "Household", "2026-07": "Sam"}},
    "settles": {"1234": "2026-03"},
}


def with_settlement(section: dict) -> dict:
    raw = copy.deepcopy(RAW)
    raw["settlement"] = section
    return raw


def test_months_and_carrying_forward():
    assert months("2025-11", "2026-02") == ["2025-11", "2025-12", "2026-01", "2026-02"]
    assert months("2026-03", "2026-02") == []
    schedule = {"2026-01": "a", "2026-06": "b"}
    assert carried(schedule, "2025-12") is None
    assert carried(schedule, "2026-05") == "a"
    assert carried(schedule, "2026-09") == "b"


def test_month_params():
    m = mapping.parse(with_settlement(SETTLEMENT))
    assert m.settlement is not None
    assert m.settlement.settles == {"1234": "2026-03"}
    feb = month_params(m, "2026-02")
    assert feb.stipends == {("Alex", Decimal("500.00"))}
    assert feb.funders == {("Studio", "Household")}
    july = month_params(m, "2026-07")
    assert july.stipends == {("Alex", Decimal("600.00")), ("Sam", Decimal("0"))}
    assert july.funders == {("Studio", "Sam")}


@pytest.mark.parametrize(
    ("change", "problem"),
    [
        ({"stipends": {"Alex": {"2026-01": 500.0}}}, "as a string"),
        ({"stipends": {"Alex": {"2026-01": "-1"}}}, "as a string"),
        ({"stipends": {"Studio": {"2026-01": "1"}}}, "not a person"),
        ({"stipends": {"Alex": {"2026-13": "1"}}}, "not a month"),
        ({"funders": {"Studio": {"2026-01": "Studio"}}}, "not the household or a person"),
        ({"funders": {"Alex": {"2026-01": "Household"}}}, "not a business"),
        ({"settles": {"abc": "2026-03"}}, "not a Firefly transaction ID"),
        ({"settles": {"1234": "March"}}, "not a month"),
        ({"currency": "EUR"}, "not a commodity"),
    ],
)
def test_bad_settlement_sections(change, problem):
    with pytest.raises(mapping.MappingError) as e:
        mapping.parse(with_settlement({**SETTLEMENT, **change}))
    assert any(problem in p for p in e.value.problems), e.value.problems


def test_closed_through_must_be_a_month():
    raw = copy.deepcopy(RAW)
    raw["scope"]["closed_through"] = "August"
    with pytest.raises(mapping.MappingError):
        mapping.parse(raw)


def test_report_flags_closed_months_only():
    def row(month: str, implied: str) -> Row:
        z = Decimal(0)
        return Row(month, "Alex", z, z, Decimal("500"), Decimal(implied), z)

    out = render([row("2026-07", "500"), row("2026-08", "450"), row("2026-09", "10")], "2026-08")
    assert "| 2026-08 | Alex | 0 | 0 | 500 | 450 | -50 ⚠ | 0 |" in out
    assert "| 2026-09 | Alex | 0 | 0 | 500 | 10 | -490 | 0 |" in out
    assert out.endswith("Months to look at: 1")
