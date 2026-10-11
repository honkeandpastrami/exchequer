"""The mapping file: parsed, and every problem reported at once."""

import copy

import pytest

from firefly_migrate import mapping
from tests.conftest import RAW


def test_parses_the_example(m):
    assert m.parties == ("Household", "Alex", "Sam", "Studio")
    assert m.by_firefly_id[4].name == "Alex Brokerage"
    assert not m.by_firefly_id[3].is_liquid
    assert m.by_firefly_id[1].is_liquid
    assert m.by_firefly_id[7].archived


def test_reports_every_problem():
    raw = copy.deepcopy(RAW)
    raw["accounts"][1]["firefly"] = [1]  # already Alex Checking's
    raw["accounts"][2]["owner"] = "Nobody"
    raw["purpose"]["budgets"]["Ghost"] = "Ghost"
    raw["scope"]["through"] = raw["scope"]["through"].replace(year=2025)
    with pytest.raises(mapping.MappingError) as e:
        mapping.parse(raw)
    problems = "\n".join(e.value.problems)
    assert "mapped twice" in problems
    assert "'Nobody' is not a party" in problems
    assert "'Ghost' maps to 'Ghost'" in problems
    assert "is not in scope.year" in problems


def test_token_files_hold_the_token_alone(tmp_path):
    f = tmp_path / "token"
    f.write_text("abc123\n")
    assert mapping.read_token(f) == "abc123"
    f.write_text("\n")
    with pytest.raises(ValueError, match="empty"):
        mapping.read_token(f)
