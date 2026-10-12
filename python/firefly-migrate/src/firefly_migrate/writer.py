"""Writes a plan into Exchequer, only through its REST API (design §11).

The target must be empty: the migration is re-run from scratch until its
report is right, never merged into existing data.
"""

from __future__ import annotations

from collections.abc import Callable
from typing import Any
from uuid import UUID

from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.accounts import accounts_create as create_account
from exchequer_sdk.api.accounts import accounts_list as list_accounts
from exchequer_sdk.api.categories import categories_create as create_category
from exchequer_sdk.api.commodities import commodities_create as create_commodity
from exchequer_sdk.api.counterparties import counterparties_create as create_counterparty
from exchequer_sdk.api.counterparties import counterparties_list as list_counterparties
from exchequer_sdk.api.import_batches import import_batches_create as create_import_batch
from exchequer_sdk.api.parties import parties_create as create_party
from exchequer_sdk.api.parties import parties_list as list_parties
from exchequer_sdk.api.transactions import transactions_create as create_transaction
from exchequer_sdk.api.transactions import transactions_list as list_transactions
from exchequer_sdk.models import (
    AccountKind,
    BatchKind,
    CommodityKind,
    CreditLimit,
    ExternalRef,
    Leg,
    NewAccount,
    NewCategory,
    NewCommodity,
    NewCounterparty,
    NewImportBatch,
    NewParty,
    NewSplit,
    NewTransaction,
    PartyKind,
    Problem,
)
from exchequer_sdk.types import UNSET, Response
from firefly_migrate.mapping import Mapping
from firefly_migrate.transform import FIREFLY, Plan
from firefly_migrate.transform import Leg as PlanLeg
from firefly_migrate.transform import Transaction as PlanTransaction


class WriteError(RuntimeError):
    pass


def _created(response: Response[Any], what: str) -> UUID:
    parsed = response.parsed
    if response.status_code != 201 or parsed is None or isinstance(parsed, Problem):
        detail = parsed.detail if isinstance(parsed, Problem) else response.content[:500]
        raise WriteError(f"{what}: HTTP {response.status_code}: {detail}")
    return UUID(str(parsed.id))


def _is_empty(client: AuthenticatedClient) -> bool:
    for page in (
        list_transactions.sync(client=client, limit=1),
        list_accounts.sync(client=client, limit=1),
        list_parties.sync(client=client, limit=1),
    ):
        if page is None or isinstance(page, Problem):
            raise WriteError("couldn't read the target server (is the token right?)")
        if page.items:
            return False
    return True


class Writer:
    def __init__(self, client: AuthenticatedClient, m: Mapping) -> None:
        self.client = client
        self.m = m
        self.parties: dict[str, UUID] = {}
        self.commodities: dict[str, UUID] = {}
        self.accounts: dict[str, UUID] = {}
        self.counterparties: dict[str, UUID] = {}
        self.system: dict[str, UUID] = {}
        self.categories: dict[str, UUID] = {}
        # Import batch per label: Firefly's import tags, and one for the rest.
        self.batches: dict[str, UUID] = {}

    def write(self, plan: Plan, progress: Callable[[int, int], None] | None = None) -> None:
        if not _is_empty(self.client):
            raise WriteError("the target server already has data; migrate into an empty one")
        self._reference_data(plan)
        total = len(plan.transactions)
        for i, t in enumerate(plan.transactions, 1):
            self._transaction(t)
            if progress:
                progress(i, total)

    def _reference_data(self, plan: Plan) -> None:
        c = self.client
        m = self.m
        kinds = [(m.household, PartyKind.HOUSEHOLD)]
        kinds += [(p, PartyKind.PERSON) for p in m.people]
        kinds += [(b, PartyKind.BUSINESS) for b in m.businesses]
        for name, kind in kinds:
            r = create_party.sync_detailed(client=c, body=NewParty(kind=kind, name=name))
            self.parties[name] = _created(r, f"party {name}")

        for code, spec in m.commodities.items():
            body = NewCommodity(code=code, kind=CommodityKind(spec.kind), precision=spec.precision)
            r = create_commodity.sync_detailed(client=c, body=body)
            self.commodities[code] = _created(r, f"commodity {code}")

        for a in m.accounts:
            if a.key not in plan.accounts:
                continue  # not used in the year
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

        page = list_counterparties.sync(client=c, limit=500)
        if page is None or isinstance(page, Problem):
            raise WriteError("couldn't list counterparties")
        for item in page.items:
            if isinstance(item.system, str):
                self.system[item.system] = UUID(str(item.id))
        for name in plan.counterparties:
            r = create_counterparty.sync_detailed(client=c, body=NewCounterparty(name=name))
            self.counterparties[name] = _created(r, f"counterparty {name}")

        for name in plan.categories:
            r = create_category.sync_detailed(client=c, body=NewCategory(name=name))
            self.categories[name] = _created(r, f"category {name}")

    def _leg(self, leg: PlanLeg) -> Leg:
        out = Leg(commodity_id=self.commodities[leg.commodity], quantity=str(leg.quantity))
        if leg.account is not None:
            out.account_id = self.accounts[leg.account]
        elif leg.counterparty is not None:
            out.counterparty_id = self.counterparties[leg.counterparty]
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

    def _transaction(self, t: PlanTransaction) -> None:
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
            import_batch_id=self._batch(t.import_batch or f"Migrated from {FIREFLY}"),
        )
        r = create_transaction.sync_detailed(client=self.client, body=body)
        where = f"Firefly transaction {t.firefly_id}" if t.firefly_id else t.description
        _created(r, f"{where} ({t.date})")
