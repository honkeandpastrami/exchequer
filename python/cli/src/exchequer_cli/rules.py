"""`exchequer rules`: the rules script (design §8, docs/ai.md). Every upload
is a new version, validated by the server; the newest is active.
"""

from __future__ import annotations

import datetime
import difflib
import os
import shlex
import subprocess
import tempfile
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from uuid import UUID

from exchequer_cli.imports import ApiError, _all, etag, ok
from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.accounts import accounts_list
from exchequer_sdk.api.categories import categories_list
from exchequer_sdk.api.counterparties import counterparties_list
from exchequer_sdk.api.parties import parties_list
from exchequer_sdk.api.rules import (
    rules_dry_run,
    rules_get,
    rules_put,
    rules_version,
    rules_versions,
)
from exchequer_sdk.models import (
    DryRun,
    DryRunRequest,
    Problem,
    RuleScript,
    RuleUpload,
    RuleVersion,
)
from exchequer_sdk.types import UNSET, Unset

# Edits the text, with the cursor on a line if given; None if the editor
# failed.
Editor = Callable[[str, int | None], str | None]


def editor(text: str, line: int | None) -> str | None:
    """`$VISUAL` or `$EDITOR` (default `vi`) on a temporary file."""
    command = shlex.split(os.environ.get("VISUAL") or os.environ.get("EDITOR") or "vi")
    with tempfile.TemporaryDirectory() as d:
        path = Path(d) / "rules.rhai"
        path.write_text(text)
        at = [f"+{line}"] if line else []
        if subprocess.run([*command, *at, str(path)], check=False).returncode != 0:
            return None
        return path.read_text()


# ----------------------------------------------------------------- Views ----


def error_lines(p: Problem) -> list[str]:
    """A refused script's errors, one per line."""
    items = p.errors if not isinstance(p.errors, Unset) else []
    if not items:
        return [f"  {p.detail or p.title}"]
    out = []
    for e in items:
        where = ""
        if not isinstance(e.line, Unset) and e.line is not None:
            where = f"line {e.line}"
            if not isinstance(e.column, Unset) and e.column is not None:
                where += f":{e.column}"
            where += ": "
        out.append(f"  {where}{e.message}")
    return out


def first_line(p: Problem) -> int | None:
    items = p.errors if not isinstance(p.errors, Unset) else []
    lines = [e.line for e in items if isinstance(e.line, int)]
    return min(lines) if lines else None


def version_line(v: RuleVersion, active: bool) -> str:
    who = f" by {v.author}" if isinstance(v.author, str) else ""
    note = f"  {v.note}" if isinstance(v.note, str) else ""
    mark = "*" if active else " "
    when = v.created_at.astimezone().strftime("%Y-%m-%d %H:%M")
    return f"{mark} v{v.version:<4} {when}{who}, {v.lines} lines{note}"


def diff(a: RuleScript, b: RuleScript) -> list[str]:
    return list(
        difflib.unified_diff(
            a.source.splitlines(),
            b.source.splitlines(),
            f"v{a.version}",
            f"v{b.version}",
            lineterm="",
        )
    )


@dataclass
class LabelNames:
    """Counterparties, categories, parties and accounts by ID."""

    names: dict[UUID, str] = field(default_factory=dict)
    accounts: dict[UUID, str] = field(default_factory=dict)

    @classmethod
    def load(cls, client: AuthenticatedClient) -> LabelNames:
        out = cls()
        for fetch in (counterparties_list, categories_list, parties_list):
            for x in _all(lambda c, f=fetch: f.sync_detailed(client=client, limit=500, cursor=c)):
                out.names[x.id] = x.name
        for a in _all(lambda c: accounts_list.sync_detailed(client=client, limit=500, cursor=c)):
            out.accounts[a.id] = f"{out.names.get(a.owner_id, '?')} · {a.name}"
        return out

    def of(self, id: UUID | Unset | None, account: UUID | Unset | None = None) -> str:
        """A value's name; with `account`, a transfer with that account."""
        if isinstance(account, UUID):
            return f"transfer with {self.accounts.get(account, str(account))}"
        if id is None or isinstance(id, Unset):
            return "none"
        return self.names.get(id, str(id))


def dry_run_lines(d: DryRun, names: LabelNames) -> list[str]:
    which = f"v{d.version}" if isinstance(d.version, int) else "the candidate"
    out = [
        f"{which}: {d.rows} transactions, {d.labeled} labeled, "
        f"{d.changed} would change, {d.failed} failed"
    ]
    for r in d.items:
        out.append(f"  {r.date} {r.description}")
        if isinstance(r.error, str):
            out.append(f"      error: {r.error}")
        for c in r.changes:
            out.append(
                f"      {c.label.value}: {names.of(c.current, c.current_transfer_account_id)}"
                f" → {names.of(c.proposed, c.proposed_transfer_account_id)}"
                f"  (line {c.line})"
            )
    if d.truncated:
        out.append("  … more not shown")
    return out


# ---------------------------------------------------------------- Rules ----


class Rules:
    def __init__(
        self, client: AuthenticatedClient, say: Callable[[str], None], edit: Editor = editor
    ) -> None:
        self.client = client
        self.say = say
        self.edit_text = edit

    def active(self) -> tuple[RuleScript, str]:
        r = rules_get.sync_detailed(client=self.client)
        return ok(r, "rules"), etag(r)

    def version(self, n: int) -> RuleScript:
        if n == 0:
            return RuleScript(version=0, source="")
        return ok(rules_version.sync_detailed(n, client=self.client), f"version {n}")

    def show(self, n: int | None) -> str:
        return self.version(n).source if n is not None else self.active()[0].source

    def log(self) -> list[str]:
        versions = list(
            _all(lambda c: rules_versions.sync_detailed(client=self.client, limit=500, cursor=c))
        )
        if not versions:
            return ["no rules yet"]
        return [version_line(v, i == 0) for i, v in enumerate(versions)]

    def diff(self, a: int, b: int | None) -> list[str]:
        new = self.version(b) if b is not None else self.active()[0]
        return diff(self.version(a), new)

    def upload(self, source: str, note: str | None, expected: str) -> RuleScript | Problem:
        r = rules_put.sync_detailed(
            client=self.client,
            body=RuleUpload(source=source, note=note if note else UNSET),
            if_match=expected,
        )
        if r.status_code == 422 and isinstance(r.parsed, Problem):
            return r.parsed
        return ok(r, "upload", 201)

    def edit(self, note: str | None) -> int:
        """Opens the active script; uploads it on save. A refused script is
        reopened at its first error, until it's accepted or left unchanged."""
        current, expected = self.active()
        text, line = current.source, None
        while True:
            edited = self.edit_text(text, line)
            if edited is None:
                self.say("the editor failed; nothing uploaded")
                return 1
            if edited == current.source:
                self.say("unchanged; nothing uploaded")
                return 0
            result = self.upload(edited, note, expected)
            if isinstance(result, RuleScript):
                self.say(f"v{result.version} is active")
                return 0
            self.say("refused:")
            for e in error_lines(result):
                self.say(e)
            text, line = edited, first_line(result)

    def rollback(self, n: int, note: str | None) -> int:
        old = self.version(n)
        _, expected = self.active()
        result = self.upload(old.source, note or f"back to v{n}", expected)
        if isinstance(result, Problem):
            self.say(f"v{n} no longer validates:")
            for e in error_lines(result):
                self.say(e)
            return 1
        self.say(f"v{result.version} is active (v{n}'s script)")
        return 0

    def dry_run(
        self, start: datetime.date, end: datetime.date, candidate: str | None
    ) -> tuple[int, list[str]]:
        r = rules_dry_run.sync_detailed(
            client=self.client,
            body=DryRunRequest(source=candidate if candidate is not None else UNSET),
            from_=start,
            to=end,
        )
        if r.status_code == 422 and isinstance(r.parsed, Problem):
            return 1, ["refused:", *error_lines(r.parsed)]
        d: DryRun = ok(r, "dry run")
        return 0, dry_run_lines(d, LabelNames.load(self.client))


def month_range(month: str) -> tuple[datetime.date, datetime.date]:
    """`YYYY-MM` → its first and last day."""
    try:
        start = datetime.date.fromisoformat(f"{month}-01")
    except ValueError as e:
        raise ApiError(0, f"not a month (YYYY-MM): {month!r}") from e
    end = (start.replace(day=28) + datetime.timedelta(days=4)).replace(day=1)
    return start, end - datetime.timedelta(days=1)
