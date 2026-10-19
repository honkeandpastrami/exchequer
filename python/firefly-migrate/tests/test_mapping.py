"""The mapping file: parsed, and every problem reported at once."""

import copy

import pytest

from firefly_migrate import mapping
from tests.conftest import RAW


def test_parses_the_example(m):
    assert m.parties == ("Household", "Alex", "Sam", "Studio")
    assert m.by_firefly_id[4].key == "Alex · Brokerage"
    assert not m.by_firefly_id[3].is_liquid
    assert m.by_firefly_id[1].is_liquid
    assert m.by_firefly_id[7].archived


def test_reports_every_problem():
    raw = copy.deepcopy(RAW)
    raw["accounts"][1]["firefly"] = [1]  # already Alex · Checking's
    raw["accounts"][2]["owner"] = "Nobody"
    raw["purpose"]["budgets"]["Ghost"] = "Ghost"
    raw["scope"]["through"] = raw["scope"]["through"].replace(year=2025)
    raw["accounts"][0]["category_in"] = "Savings"  # Checking is liquid
    raw["accounts"][4]["category_out"] = ""
    with pytest.raises(mapping.MappingError) as e:
        mapping.parse(raw)
    problems = "\n".join(e.value.problems)
    assert "mapped twice" in problems
    assert "'Nobody' is not a party" in problems
    assert "'Ghost' maps to 'Ghost'" in problems
    assert "is not in scope.year" in problems
    assert "Checking: only a non-liquid account takes category_in" in problems
    assert "Mortgage: category_out must be a category name" in problems


def test_account_names_are_unique_per_owner():
    m = mapping.parse(copy.deepcopy(RAW))
    assert {m.by_firefly_id[1].key, m.by_firefly_id[7].key} == {"Alex · Checking", "Sam · Checking"}
    assert m.cash_account_by_destination == {"Alex · Brokerage": "Broker Inc"}

    raw = copy.deepcopy(RAW)
    raw["accounts"][1]["name"] = "Checking"  # Alex's second "Checking"
    with pytest.raises(mapping.MappingError, match="Alex · Checking appears twice"):
        mapping.parse(raw)


def test_a_shared_account_name_needs_its_owner():
    raw = copy.deepcopy(RAW)
    raw["counterparties"]["cash_account_by_destination"] = {"Checking": "Bank"}
    with pytest.raises(mapping.MappingError, match="'Checking' is ambiguous"):
        mapping.parse(raw)
    raw["counterparties"]["cash_account_by_destination"] = {"Sam · Checking": "Bank"}
    assert mapping.parse(raw).cash_account_by_destination == {"Sam · Checking": "Bank"}


def test_token_files_hold_the_token_alone(tmp_path):
    f = tmp_path / "token"
    f.write_text("abc123\n")
    assert mapping.read_token(f) == "abc123"
    f.write_text("\n")
    with pytest.raises(ValueError, match="empty"):
        mapping.read_token(f)


@pytest.mark.parametrize(
    ("section", "problem"),
    [
        ({"abc": {"purpose": "Alex"}}, "not a Firefly transaction ID"),
        ({"1": {"purpose": "Nobody"}}, "is not a party"),
        ({"1": {"purpose": "Alex", "reconciliation": True}}, "has no purpose or category"),
        ({"1": {"reconciliation": "yes"}}, "must be true or false"),
        ({"1": {"budget": "Shared"}}, "unknown keys"),
        ({"1": {}}, "says nothing"),
    ],
)
def test_bad_transaction_decisions(section, problem):
    raw = copy.deepcopy(RAW)
    raw["transactions"] = section
    with pytest.raises(mapping.MappingError) as e:
        mapping.parse(raw)
    assert any(problem in p for p in e.value.problems), e.value.problems
