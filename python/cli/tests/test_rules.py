"""`exchequer rules`, without a server."""

import datetime
from uuid import UUID

from exchequer_cli import rules
from exchequer_cli.rules import LabelNames, Rules
from exchequer_sdk.models import (
    DryRun,
    DryRunChange,
    DryRunRow,
    Label,
    Problem,
    ProblemItem,
    RuleScript,
)

DINING, TREATS = UUID(int=1), UUID(int=2)


def problem(*lines: int) -> Problem:
    return Problem(
        status=422,
        title="The script is invalid",
        type_="urn:exchequer:problem:rules-invalid",
        errors=[ProblemItem(message=f"oops {n}", line=n, column=3) for n in lines],
    )


def test_errors_are_shown_by_line():
    assert rules.error_lines(problem(4, 2)) == ["  line 4:3: oops 4", "  line 2:3: oops 2"]
    assert rules.first_line(problem(4, 2)) == 2
    p = Problem(status=422, title="Invalid", type_="x", detail="too long")
    assert rules.error_lines(p) == ["  too long"]
    assert rules.first_line(p) is None


def test_a_dry_run_names_what_would_change():
    d = DryRun(
        rows=3,
        labeled=2,
        changed=1,
        failed=1,
        truncated=True,
        items=[
            DryRunRow(
                account_id=UUID(int=9),
                changes=[DryRunChange(label=Label.CATEGORY, line=2, current=DINING, proposed=None)],
                date=datetime.date(2026, 3, 1),
                description="CORNER CAFE",
                transaction_id=UUID(int=10),
            ),
            DryRunRow(
                account_id=UUID(int=9),
                changes=[],
                date=datetime.date(2026, 3, 3),
                description="HARDWARE",
                transaction_id=UUID(int=11),
                error="unexpected",
            ),
        ],
    )
    assert rules.dry_run_lines(d, LabelNames({DINING: "Dining"})) == [
        "the candidate: 3 transactions, 2 labeled, 1 would change, 1 failed",
        "  2026-03-01 CORNER CAFE",
        "      category: Dining → none  (line 2)",
        "  2026-03-03 HARDWARE",
        "      error: unexpected",
        "  … more not shown",
    ]


def test_versions_diff_and_months():
    a = RuleScript(version=1, source="a\nb\n")
    b = RuleScript(version=2, source="a\nc\n")
    assert rules.diff(a, b)[2:] == ["@@ -1,2 +1,2 @@", " a", "-b", "+c"]
    assert rules.month_range("2026-02") == (datetime.date(2026, 2, 1), datetime.date(2026, 2, 28))
    assert rules.month_range("2026-12")[1] == datetime.date(2026, 12, 31)


class FakeRules(Rules):
    """The server's side, faked: refuses scripts containing "bad" at line 2."""

    def __init__(self, edits: list[str | None]) -> None:
        self.said: list[str] = []
        self.opened: list[tuple[str, int | None]] = []
        self.uploaded: list[str] = []
        edits_left = list(edits)

        def edit(text: str, line: int | None) -> str | None:
            self.opened.append((text, line))
            return edits_left.pop(0)

        super().__init__(client=None, say=self.said.append, edit=edit)  # ty: ignore[invalid-argument-type]

    def active(self) -> tuple[RuleScript, str]:
        return RuleScript(version=3, source="old\n"), '"3"'

    def upload(self, source: str, note: str | None, expected: str) -> RuleScript | Problem:
        self.uploaded.append(source)
        if "bad" in source:
            return problem(2)
        return RuleScript(version=4, source=source)


def test_edit_reopens_a_refused_script_at_its_error():
    r = FakeRules(["x\nbad\n", "x\ngood\n"])
    assert r.edit(None) == 0
    assert r.opened == [("old\n", None), ("x\nbad\n", 2)]
    assert r.said == ["refused:", "  line 2:3: oops 2", "v4 is active"]


def test_edit_unchanged_or_failed_uploads_nothing():
    r = FakeRules(["old\n"])
    assert r.edit(None) == 0
    assert (r.uploaded, r.said) == ([], ["unchanged; nothing uploaded"])
    r = FakeRules([None])
    assert r.edit(None) == 1
    assert r.uploaded == []
