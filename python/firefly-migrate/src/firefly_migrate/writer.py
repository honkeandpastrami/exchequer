"""Writes a year's plan into Exchequer, only through its REST API (design §11),
into an empty server or around the years it holds (`existing.fit`).
"""

from __future__ import annotations

from collections.abc import Callable
from typing import Any
from uuid import UUID

from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.accounts import accounts_add_number as add_number
from exchequer_sdk.api.accounts import accounts_create as create_account
from exchequer_sdk.api.categories import categories_create as create_category
from exchequer_sdk.api.commodities import commodities_create as create_commodity
from exchequer_sdk.api.counterparties import counterparties_create as create_counterparty
from exchequer_sdk.api.import_batches import import_batches_create as create_import_batch
from exchequer_sdk.api.parties import parties_create as create_party
from exchequer_sdk.api.settlements import settlements_close as close_month
from exchequer_sdk.api.settlements import settlements_get as get_settlement
from exchequer_sdk.api.settlements import settlements_reopen as reopen_month
from exchequer_sdk.api.settlements import settlements_statement as get_statement
from exchequer_sdk.api.transactions import transactions_create as create_transaction
from exchequer_sdk.api.transactions import transactions_delete as delete_transaction
from exchequer_sdk.api.transactions import transactions_get as get_transaction
from exchequer_sdk.models import (
    AccountKind,
    BatchKind,
    CommodityKind,
    CreditLimit,
    ExternalRef,
    ImportFingerprint,
    Leg,
    NewAccount,
    NewCategory,
    NewCommodity,
    NewCounterparty,
    NewImportBatch,
    NewNumber,
    NewParty,
    NewSplit,
    NewTransaction,
    PartyKind,
    Problem,
)
from exchequer_sdk.types import UNSET, Response
from firefly_migrate import fingerprints
from firefly_migrate.existing import Fit, Ledger, describe, month
from firefly_migrate.mapping import Mapping
from firefly_migrate.transform import FIREFLY, Plan, normalize_name
from firefly_migrate.transform import Leg as PlanLeg
from firefly_migrate.transform import Transaction as PlanTransaction


class WriteError(RuntimeError):
    pass


def add_numbers(client: AuthenticatedClient, account: UUID, etag: str, numbers: Any) -> str:
    """Adds account numbers to an account; returns its ETag after."""
    for n in numbers:
        r = add_number.sync_detailed(
            account, client=client, body=NewNumber(number=n), if_match=etag
        )
        if r.status_code != 200:
            detail = r.parsed.detail if isinstance(r.parsed, Problem) else r.status_code
            raise WriteError(f"account number …{n[-4:]}: {detail}")
        etag = r.headers["etag"]
    return etag


def _created(response: Response[Any], what: str) -> UUID:
    parsed = response.parsed
    if response.status_code != 201 or parsed is None or isinstance(parsed, Problem):
        detail = parsed.detail if isinstance(parsed, Problem) else response.content[:500]
        raise WriteError(f"{what}: HTTP {response.status_code}: {detail}")
    return UUID(str(parsed.id))


def _ok(response: Response[Any], what: str) -> Any:
    parsed = response.parsed
    if response.status_code >= 300 or isinstance(parsed, Problem):
        detail = parsed.detail if isinstance(parsed, Problem) else response.content[:500]
        raise WriteError(f"{what}: HTTP {response.status_code}: {detail}")
    return parsed


class Writer:
    """Writes a year into the server `ledger` was read from, as `fit` worked
    out: only once nothing stops it."""

    def __init__(self, client: AuthenticatedClient, m: Mapping, ledger: Ledger) -> None:
        self.client = client
        self.m = m
        self.ledger = ledger
        self.parties = {name: pid for name, (pid, _) in ledger.parties.items()}
        self.commodities = {code: cid for code, (cid, *_) in ledger.commodities.items()}
        self.accounts = {key: aid for key, (aid, _) in ledger.accounts.items()}
        # By normalized name (`transform.normalize_name`).
        self.counterparties = dict(ledger.counterparties)
        self.system = dict(ledger.system)
        self.categories = dict(ledger.categories)
        # Import batch per label: Firefly's import tags, and one for the rest.
        self.batches: dict[str, UUID] = {}

    def write(
        self, plan: Plan, fit: Fit, progress: Callable[[int, int], None] | None = None
    ) -> None:
        if fit.stops:
            raise WriteError("the plan doesn't fit the server; see the report")
        # Closed months after the year: what the year changes in them is checked.
        after = month(plan.through)
        closed = sorted(x for x in self.ledger.closed if x > after)
        before = {x: self._statement(x) for x in closed}

        self._reference_data(plan)
        total = len(fit.writes)
        rows: list[ImportFingerprint] = []
        for i, t in enumerate(fit.writes, 1):
            tid = self._transaction(t)
            rows += [
                ImportFingerprint(account_id=self.accounts[a], fingerprint=fp, transaction_id=tid)
                for a, fp in t.fingerprints
                if a in self.accounts
            ]
            if progress:
                progress(i, total)
        try:
            fingerprints.post(self.client, rows, fingerprints.Outcome())
        except fingerprints.FingerprintError as e:
            raise WriteError(str(e)) from e

        if fit.replace:
            self._replace(fit)
        changed = [x for x in closed if self._statement(x) != before[x]]
        if changed:
            raise WriteError(
                "written, but the statements of closed months changed: " + ", ".join(changed)
            )

    def _statement(self, x: str) -> dict[str, Any]:
        r = get_statement.sync_detailed(x, client=self.client)
        statement = _ok(r, f"statement {x}").to_dict()
        statement.pop("closed", None)
        return statement

    def _settlement_etag(self, x: str) -> str:
        r = get_settlement.sync_detailed(x, client=self.client)
        _ok(r, f"settlement {x}")
        return r.headers["etag"]

    def _replace(self, fit: Fit) -> None:
        """Deletes the opening balances the year's end replaces, reopening
        their month just for it."""
        if fit.reopen:
            etag = self._settlement_etag(fit.reopen)
            _ok(
                reopen_month.sync_detailed(fit.reopen, client=self.client, if_match=etag),
                f"reopen {fit.reopen}",
            )
        for h in fit.replace:
            r = get_transaction.sync_detailed(h.id, client=self.client)
            _ok(r, f"opening balance {h.id}")
            _ok(
                delete_transaction.sync_detailed(
                    h.id, client=self.client, if_match=r.headers["etag"]
                ),
                f"delete the opening balance {describe(h.t)}",
            )
        if fit.reopen:
            etag = self._settlement_etag(fit.reopen)
            _ok(
                close_month.sync_detailed(fit.reopen, client=self.client, if_match=etag),
                f"close {fit.reopen}",
            )

    def _reference_data(self, plan: Plan) -> None:
        c = self.client
        m = self.m
        kinds = [(m.household, PartyKind.HOUSEHOLD)]
        kinds += [(p, PartyKind.PERSON) for p in m.people]
        kinds += [(b, PartyKind.BUSINESS) for b in m.businesses]
        for name, kind in kinds:
            if name in self.parties:
                continue
            r = create_party.sync_detailed(client=c, body=NewParty(kind=kind, name=name))
            self.parties[name] = _created(r, f"party {name}")

        for code, spec in m.commodities.items():
            if code in self.commodities:
                continue
            body = NewCommodity(code=code, kind=CommodityKind(spec.kind), precision=spec.precision)
            r = create_commodity.sync_detailed(client=c, body=body)
            self.commodities[code] = _created(r, f"commodity {code}")

        for a in m.accounts:
            if a.key not in plan.accounts or a.key in self.accounts:
                continue  # not used in the year, or there already
            limit = plan.accounts[a.key]
            body = NewAccount(
                kind=AccountKind(a.kind),
                name=a.name,
                owner_id=self.parties[a.owner],
                archived=a.archived,
                liquid=a.liquid if a.liquid is not None else UNSET,
                credit_limit=(
                    CreditLimit(
                        commodity_id=self.commodities[limit.commodity],
                        quantity=str(limit.quantity),
                    )
                    if limit
                    else UNSET
                ),
            )
            r = create_account.sync_detailed(client=c, body=body)
            self.accounts[a.key] = _created(r, f"account {a.key}")
            add_numbers(c, self.accounts[a.key], r.headers["etag"], plan.numbers.get(a.key, ()))

        for name in plan.counterparties:
            if normalize_name(name) in self.counterparties:
                continue
            r = create_counterparty.sync_detailed(client=c, body=NewCounterparty(name=name))
            self.counterparties[normalize_name(name)] = _created(r, f"counterparty {name}")

        for name in plan.categories:
            if name in self.categories:
                continue
            r = create_category.sync_detailed(client=c, body=NewCategory(name=name))
            self.categories[name] = _created(r, f"category {name}")

    def _leg(self, leg: PlanLeg) -> Leg:
        out = Leg(commodity_id=self.commodities[leg.commodity], quantity=str(leg.quantity))
        if leg.account is not None:
            out.account_id = self.accounts[leg.account]
        elif leg.counterparty is not None:
            out.counterparty_id = self.counterparties[normalize_name(leg.counterparty)]
        elif leg.system is not None:
            out.counterparty_id = self.system[leg.system]
        return out

    def _batch(self, label: str) -> UUID:
        if label not in self.batches:
            r = create_import_batch.sync_detailed(
                client=self.client, body=NewImportBatch(kind=BatchKind.MIGRATION, label=label)
            )
            self.batches[label] = _created(r, f"import batch {label!r}")
        return self.batches[label]

    def _transaction(self, t: PlanTransaction) -> UUID:
        splits = [
            NewSplit(
                from_=self._leg(s.from_),
                to=self._leg(s.to),
                purpose_id=self.parties[s.purpose] if s.purpose else UNSET,
                category_id=self.categories[s.category] if s.category else UNSET,
                description=s.description if s.description else UNSET,
            )
            for s in t.splits
        ]
        body = NewTransaction(
            date=t.date,
            description=t.description,
            splits=splits,
            notes=t.notes if t.notes else UNSET,
            tags=t.tags,
            external_refs=(
                [ExternalRef(source=FIREFLY, ref=t.firefly_id)] if t.firefly_id else UNSET
            ),
            import_batch_id=self._batch(t.import_batch or f"Migrated from {FIREFLY} {self.m.year}"),
        )
        r = create_transaction.sync_detailed(client=self.client, body=body)
        where = f"Firefly transaction {t.firefly_id}" if t.firefly_id else t.description
        return _created(r, f"{where} ({t.date})")
