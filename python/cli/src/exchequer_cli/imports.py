"""`exchequer import`: upload bank files, review what each row would
become, and commit (design §6). Staged imports live on the server, so one
left unfinished here can be finished elsewhere.
"""

from __future__ import annotations

import io
from collections.abc import Callable, Iterator
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Protocol
from uuid import UUID

from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.accounts import accounts_list
from exchequer_sdk.api.imports import (
    imports_commit,
    imports_discard,
    imports_get,
    imports_rows,
    imports_set_row,
    imports_set_statement,
    imports_upload,
)
from exchequer_sdk.api.parties import parties_list
from exchequer_sdk.api.transactions import transactions_get
from exchequer_sdk.models import (
    Committed,
    Decision,
    Import,
    Problem,
    RowStatus,
    RowUpdate,
    StagedRow,
    StagedStatement,
    StatementUpdate,
)
from exchequer_sdk.types import UNSET, File, Response, Unset

STATUS = {
    RowStatus.NEW: "new",
    RowStatus.DUPLICATE: "duplicate",
    RowStatus.NEAR_DUPLICATE: "near duplicate",
    RowStatus.TRANSFER_MATCH: "transfer match",
    RowStatus.TRANSFER_DUPLICATE: "transfer already recorded",
    RowStatus.NEEDS_ACCOUNT: "needs account",
}
# The letter that picks each decision in a prompt.
KEYS = {Decision.MATCH: "m", Decision.NEW: "n", Decision.REPLACE: "r", Decision.SKIP: "d"}
LABELS = {
    Decision.MATCH: "match",
    Decision.NEW: "new",
    Decision.REPLACE: "replace",
    Decision.SKIP: "do not import",
}


class ApiError(RuntimeError):
    def __init__(self, status: int, detail: str) -> None:
        super().__init__(detail)
        self.status = status


def ok(r: Response[Any], what: str, *codes: int) -> Any:
    if r.status_code not in (codes or (200,)) or isinstance(r.parsed, Problem):
        detail = r.parsed.detail if isinstance(r.parsed, Problem) else r.content[:300]
        raise ApiError(r.status_code, f"{what}: {detail}")
    return r.parsed


def etag(r: Response[Any]) -> str:
    return r.headers["etag"]


class Ui(Protocol):
    def say(self, text: str) -> None: ...

    def choose(self, prompt: str, options: list[str]) -> int | None:
        """An option's index, or None for none of them."""
        ...

    def confirm(self, prompt: str, default: bool) -> bool: ...

    def key(self, prompt: str, keys: str, default: str) -> str:
        """One of `keys`."""
        ...


@dataclass
class Names:
    """Accounts by ID, as `Owner · Name`, and the ones a file may be for."""

    accounts: dict[UUID, str] = field(default_factory=dict)
    choices: list[tuple[UUID, str]] = field(default_factory=list)

    @classmethod
    def load(cls, client: AuthenticatedClient) -> Names:
        parties = {
            p.id: p.name
            for p in _all(lambda c: parties_list.sync_detailed(client=client, limit=500, cursor=c))
        }
        out = cls()
        for a in _all(lambda c: accounts_list.sync_detailed(client=client, limit=500, cursor=c)):
            name = f"{parties.get(a.owner_id, '?')} · {a.name}"
            out.accounts[a.id] = name
            if not a.archived:
                out.choices.append((a.id, name))
        out.choices.sort(key=lambda c: c[1])
        return out

    def find(self, name: str) -> UUID:
        matches = [i for i, n in self.accounts.items() if name in (n, n.split(" · ", 1)[1])]
        if len(matches) != 1:
            raise ApiError(0, f"no single account named {name!r} (write it as Owner · Name)")
        return matches[0]


def _all(fetch: Callable[[str | Unset], Response[Any]]) -> Iterator[Any]:
    cursor: str | Unset = UNSET
    while True:
        page = ok(fetch(cursor), "list")
        yield from page.items
        if not page.next_cursor:
            return
        cursor = page.next_cursor


# ----------------------------------------------------------------- Views ----


def statement_line(s: StagedStatement, names: Names) -> str:
    number = f"…{s.number_last4}" if isinstance(s.number_last4, str) else "no number"
    account = names.accounts.get(s.account_id, "?") if isinstance(s.account_id, UUID) else None
    if account and s.account_suggested:
        account += " (suggested)"
    counts = s.counts.to_dict()
    shown = ", ".join(f"{counts[k.value]} {STATUS[k]}" for k in STATUS if counts.get(k.value))
    text = f"  {number} → {account or 'NO ACCOUNT YET'}: {shown or 'no rows'}"
    if s.ledger and not isinstance(s.ledger, Unset):
        text += f"; bank balance {s.ledger.amount} on {s.ledger.date}"
    if s.skipped:
        text += f"; {len(s.skipped)} unreadable lines"
    return text


def row_line(r: StagedRow, matched: str | None = None, names: Names | None = None) -> str:
    description = r.set_description if isinstance(r.set_description, str) else r.description
    text = f"{r.date}  {r.amount:>12}  {description[:40]:<40}  {STATUS[r.status]}"
    if matched:
        text += f" of {matched}"
    m = r.transfer_match
    if not isinstance(m, Unset) and m is not None:
        account = names.accounts.get(m.account_id, "?") if names else str(m.account_id)
        text += f" with {account}: {m.date} {m.description!r}"
    return text


def decisions_for(status: RowStatus) -> list[Decision]:
    """What a row with this status can become, the default first."""
    if status == RowStatus.NEW:
        return [Decision.NEW, Decision.SKIP]
    if status == RowStatus.DUPLICATE:
        return [Decision.MATCH, Decision.REPLACE, Decision.SKIP]
    if status == RowStatus.NEAR_DUPLICATE:
        return [Decision.MATCH, Decision.REPLACE, Decision.NEW, Decision.SKIP]
    return [Decision.MATCH, Decision.NEW, Decision.SKIP]


def committed_lines(c: Committed, names: Names) -> list[str]:
    out = [
        f"  {c.created} new ({c.transfers} transfers with their match), {c.matched} already there, "
        f"{c.replaced} replaced, {c.skipped} not imported"
    ]
    for b in c.balance_checks:
        account = names.accounts.get(b.account_id, "?")
        if b.matches:
            out.append(f"  {account}: balance {b.amount} on {b.date} matches")
        else:
            out.append(
                f"  {account}: the bank says {b.amount} on {b.date}, the ledger {b.ledger} "
                f"(off by {b.difference})"
            )
    return out


# ----------------------------------------------------------------- Flow ----


class Importer:
    def __init__(self, client: AuthenticatedClient, ui: Ui, *, yes: bool) -> None:
        self.client = client
        self.ui = ui
        self.yes = yes
        self.names = Names.load(client)
        self._matched: dict[UUID, str] = {}

    def run(self, files: list[Path], account: str | None) -> int:
        account_id = self.names.find(account) if account else None
        failed = 0
        for f in files:
            try:
                if not self.one(f, account_id):
                    failed += 1
            except ApiError as e:
                self.ui.say(f"{f.name}: {e}")
                failed += 1
        return 1 if failed else 0

    def one(self, f: Path, account_id: UUID | None) -> bool:
        """Whether the file was committed (or deliberately discarded)."""
        body = File(payload=io.BytesIO(f.read_bytes()), file_name=f.name)
        r = imports_upload.sync_detailed(
            client=self.client,
            body=body,
            file_name=f.name,
            account_id=account_id if account_id else UNSET,
        )
        staged: Import = ok(r, "upload", 201)
        self.ui.say(f"{f.name}:")
        if not self.accounts(staged):
            self.ui.say(f"  left staged ({staged.id}): no account (give --account)")
            return False
        while True:
            imp, tag = self.read(staged.id)
            for s in imp.statements:
                self.ui.say(statement_line(s, self.names))
            if not self.yes:
                action = self.ui.key(
                    "  [c]ommit, [r]eview rows, [l]eave staged, [d]iscard?", "crld", "c"
                )
                if action == "l":
                    self.ui.say(f"  left staged ({imp.id})")
                    return False
                if action == "d":
                    ok(
                        imports_discard.sync_detailed(imp.id, client=self.client, if_match=tag),
                        "discard",
                        204,
                    )
                    self.ui.say("  discarded")
                    return True
                if action == "r":
                    self.review(imp.id)
                    continue
            r = imports_commit.sync_detailed(
                imp.id, client=self.client, if_match=self.read(imp.id)[1]
            )
            if (
                r.status_code == 409
                and isinstance(r.parsed, Problem)
                and "again" in str(r.parsed.detail)
            ):
                self.ui.say("  the ledger changed since the rows were checked; here they are again")
                continue
            if (
                r.status_code == 409
                and isinstance(r.parsed, Problem)
                and r.parsed.type_.endswith(":decisions-missing")
                and not self.yes
            ):
                # A near duplicate or transfer match waits for your decision.
                self.ui.say(f"  {r.parsed.detail}; [r]eview the rows to decide")
                continue
            if (
                r.status_code == 409
                and isinstance(r.parsed, Problem)
                and r.parsed.type_.endswith((":labels-missing", ":decisions-missing"))
            ):
                # Labels are chosen in the web app's review (design §8).
                self.ui.say(f"  left staged ({imp.id}): {r.parsed.detail}")
                return False
            done: Committed = ok(r, "commit")
            for line in committed_lines(done, self.names):
                self.ui.say(line)
            return True

    def read(self, id: UUID) -> tuple[Import, str]:
        r = imports_get.sync_detailed(id, client=self.client)
        return ok(r, "read"), etag(r)

    def accounts(self, staged: Import) -> bool:
        """Gives every statement an account; False if one is left without."""
        for s in staged.statements:
            if isinstance(s.account_id, UUID):
                continue
            last4 = s.number_last4 if isinstance(s.number_last4, str) else None
            if self.yes:
                return False
            what = f"…{last4}" if last4 else "this file"
            pick = self.ui.choose(
                f"  Which account is {what} for?", [n for _, n in self.names.choices]
            )
            if pick is None:
                return False
            account = self.names.choices[pick][0]
            remember = bool(last4) and self.ui.confirm(
                f"  Remember …{last4} as {self.names.choices[pick][1]}?", True
            )
            _, tag = self.read(staged.id)
            ok(
                imports_set_statement.sync_detailed(
                    staged.id,
                    s.position,
                    client=self.client,
                    body=StatementUpdate(account_id=account, remember_number=remember),
                    if_match=tag,
                ),
                "set the account",
            )
        return True

    def matched(self, id: UUID) -> str:
        if id not in self._matched:
            t = ok(transactions_get.sync_detailed(id, client=self.client), "read a transaction")
            self._matched[id] = f"{t.date} {t.description!r}"
        return self._matched[id]

    def review(self, id: UUID) -> None:
        """Asks about every row but exact duplicates."""
        rows: list[StagedRow] = list(
            _all(lambda c: imports_rows.sync_detailed(id, client=self.client, limit=500, cursor=c))
        )
        for r in rows:
            if r.status in (RowStatus.DUPLICATE, RowStatus.NEEDS_ACCOUNT):
                continue
            match = r.match_transaction_id if isinstance(r.match_transaction_id, UUID) else None
            self.ui.say(row_line(r, self.matched(match) if match else None, self.names))
            options = decisions_for(r.status)
            current = r.decision if isinstance(r.decision, Decision) else options[0]
            prompt = "    " + ", ".join(f"[{KEYS[d]}]{LABELS[d][1:]}" for d in options) + "?"
            picked = self.ui.key(prompt, "".join(KEYS[d] for d in options), KEYS[current])
            decision = next(d for d in options if KEYS[d] == picked)
            # Taking the default is a decision too: a guess needs one.
            if decision != current or not r.decided:
                try:
                    ok(
                        imports_set_row.sync_detailed(
                            id,
                            r.statement,
                            r.position,
                            client=self.client,
                            body=RowUpdate(decision=decision),
                            if_match=self.read(id)[1],
                        ),
                        "set a row's decision",
                    )
                except ApiError as e:
                    # E.g. a transaction a bank row can't replace.
                    if e.status != 422:
                        raise
                    self.ui.say(f"    kept {LABELS[current]}: {e}")
