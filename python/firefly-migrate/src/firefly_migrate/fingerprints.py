"""Records, in Exchequer, which bank rows the migrated transactions came
from: Firefly's `external_id`s, the IDs the preimporter gave each row
(design §6.3). Then a bank file with those rows finds them as duplicates.
`run` does it for what it writes; `firefly-migrate fingerprints` does it for
an earlier migration. Only counts are printed: the IDs are bank data.
"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from uuid import UUID

from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.imports import imports_add_fingerprints as add_fingerprints
from exchequer_sdk.api.transactions import transactions_list as list_transactions
from exchequer_sdk.models import ImportFingerprint, NewImportFingerprints, Problem
from firefly_migrate.check import _all, accounts_by_key
from firefly_migrate.transform import FIREFLY, Plan

# Most items in one request (the server's limit).
CHUNK = 1000


class FingerprintError(RuntimeError):
    pass


@dataclass
class Outcome:
    added: int = 0
    unchanged: int = 0
    conflicts: int = 0
    # Planned transactions with IDs that the server doesn't have.
    missing_transactions: int = 0
    missing_accounts: set[str] = field(default_factory=set)


def post(client: AuthenticatedClient, items: list[ImportFingerprint], out: Outcome) -> None:
    for i in range(0, len(items), CHUNK):
        body = NewImportFingerprints(items=items[i : i + CHUNK])
        r = add_fingerprints.sync_detailed(client=client, body=body)
        if r.status_code != 200 or r.parsed is None or isinstance(r.parsed, Problem):
            detail = r.parsed.detail if isinstance(r.parsed, Problem) else r.status_code
            raise FingerprintError(f"recording row IDs: {detail}")
        out.added += r.parsed.added
        out.unchanged += r.parsed.unchanged
        out.conflicts += len(r.parsed.conflicts)


def items_for(
    plan: Plan, transactions: dict[str, UUID], accounts: dict[str, UUID], out: Outcome
) -> list[ImportFingerprint]:
    """`transactions`: Exchequer's transaction per Firefly group ID;
    `accounts`: Exchequer's account per `Owner · Name`."""
    items = []
    for t in plan.transactions:
        if not t.fingerprints or not t.firefly_id:
            continue
        tid = transactions.get(t.firefly_id)
        if tid is None:
            out.missing_transactions += 1
            continue
        for key, fp in t.fingerprints:
            if key not in accounts:
                out.missing_accounts.add(key)
                continue
            items.append(
                ImportFingerprint(account_id=accounts[key], fingerprint=fp, transaction_id=tid)
            )
    return items


def fill(
    client: AuthenticatedClient, plan: Plan, *, apply: bool, say: Callable[[str], None]
) -> Outcome:
    out = Outcome()
    accounts = {key: a.id for key, a in accounts_by_key(client).items()}
    transactions = {
        ref.ref: t.id
        for t in _all(lambda c: list_transactions.sync(client=client, limit=500, cursor=c))
        for ref in t.external_refs
        if ref.source == FIREFLY
    }
    items = items_for(plan, transactions, accounts, out)
    say(f"{len(items)} row IDs for {len({i.transaction_id for i in items})} transactions")
    if out.missing_transactions:
        say(f"{out.missing_transactions} planned transactions with row IDs aren't in Exchequer")
    for key in sorted(out.missing_accounts):
        say(f"account {key} isn't in Exchequer; its row IDs are left out")
    if apply:
        post(client, items, out)
    return out
