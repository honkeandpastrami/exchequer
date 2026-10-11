from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.candidate import Candidate


T = TypeVar("T", bound="BalanceCheck")


@_attrs_define
class BalanceCheck:
    """
    Attributes:
        account_id (UUID):
        amount (str): What the bank showed.
        commodity_id (UUID):
        created_at (datetime.datetime):
        date (datetime.date):
        difference (str): `ledger − amount`.
        id (UUID):
        ledger (str): The ledger's balance at the end of `date`, signed like `amount`.
        matches (bool):
        source (str): `user` or `ofx`.
        candidates (list[Candidate] | None | Unset): For a mismatch, on a single check only.
        searched_after (datetime.date | None | Unset): For a mismatch: candidates were searched after this date (the
            last
            verified date before the check), or from the beginning if absent.
        verified_through (datetime.date | None | Unset): The latest date up to this check's on which a check of the same
            account and commodity matched.
    """

    account_id: UUID
    amount: str
    commodity_id: UUID
    created_at: datetime.datetime
    date: datetime.date
    difference: str
    id: UUID
    ledger: str
    matches: bool
    source: str
    candidates: list[Candidate] | None | Unset = UNSET
    searched_after: datetime.date | None | Unset = UNSET
    verified_through: datetime.date | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        account_id = str(self.account_id)

        amount = self.amount

        commodity_id = str(self.commodity_id)

        created_at = self.created_at.isoformat()

        date = self.date.isoformat()

        difference = self.difference

        id = str(self.id)

        ledger = self.ledger

        matches = self.matches

        source = self.source

        candidates: list[dict[str, Any]] | None | Unset
        if isinstance(self.candidates, Unset):
            candidates = UNSET
        elif isinstance(self.candidates, list):
            candidates = []
            for candidates_type_0_item_data in self.candidates:
                candidates_type_0_item = candidates_type_0_item_data.to_dict()
                candidates.append(candidates_type_0_item)

        else:
            candidates = self.candidates

        searched_after: None | str | Unset
        if isinstance(self.searched_after, Unset):
            searched_after = UNSET
        elif isinstance(self.searched_after, datetime.date):
            searched_after = self.searched_after.isoformat()
        else:
            searched_after = self.searched_after

        verified_through: None | str | Unset
        if isinstance(self.verified_through, Unset):
            verified_through = UNSET
        elif isinstance(self.verified_through, datetime.date):
            verified_through = self.verified_through.isoformat()
        else:
            verified_through = self.verified_through

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "account_id": account_id,
                "amount": amount,
                "commodity_id": commodity_id,
                "created_at": created_at,
                "date": date,
                "difference": difference,
                "id": id,
                "ledger": ledger,
                "matches": matches,
                "source": source,
            }
        )
        if candidates is not UNSET:
            field_dict["candidates"] = candidates
        if searched_after is not UNSET:
            field_dict["searched_after"] = searched_after
        if verified_through is not UNSET:
            field_dict["verified_through"] = verified_through

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.candidate import Candidate  # noqa: PLC0415

        d = dict(src_dict)
        account_id = UUID(d.pop("account_id"))

        amount = d.pop("amount")

        commodity_id = UUID(d.pop("commodity_id"))

        created_at = datetime.datetime.fromisoformat(d.pop("created_at"))

        date = datetime.date.fromisoformat(d.pop("date"))

        difference = d.pop("difference")

        id = UUID(d.pop("id"))

        ledger = d.pop("ledger")

        matches = d.pop("matches")

        source = d.pop("source")

        def _parse_candidates(data: object) -> list[Candidate] | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                candidates_type_0 = []
                _candidates_type_0 = data
                for candidates_type_0_item_data in _candidates_type_0:
                    candidates_type_0_item = Candidate.from_dict(candidates_type_0_item_data)

                    candidates_type_0.append(candidates_type_0_item)

                return candidates_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(list[Candidate] | None | Unset, data)

        candidates = _parse_candidates(d.pop("candidates", UNSET))

        def _parse_searched_after(data: object) -> datetime.date | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                searched_after_type_0 = datetime.date.fromisoformat(data)

                return searched_after_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(datetime.date | None | Unset, data)

        searched_after = _parse_searched_after(d.pop("searched_after", UNSET))

        def _parse_verified_through(data: object) -> datetime.date | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                verified_through_type_0 = datetime.date.fromisoformat(data)

                return verified_through_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(datetime.date | None | Unset, data)

        verified_through = _parse_verified_through(d.pop("verified_through", UNSET))

        balance_check = cls(
            account_id=account_id,
            amount=amount,
            commodity_id=commodity_id,
            created_at=created_at,
            date=date,
            difference=difference,
            id=id,
            ledger=ledger,
            matches=matches,
            source=source,
            candidates=candidates,
            searched_after=searched_after,
            verified_through=verified_through,
        )

        balance_check.additional_properties = d
        return balance_check

    @property
    def additional_keys(self) -> list[str]:
        return list(self.additional_properties.keys())

    def __getitem__(self, key: str) -> Any:
        return self.additional_properties[key]

    def __setitem__(self, key: str, value: Any) -> None:
        self.additional_properties[key] = value

    def __delitem__(self, key: str) -> None:
        del self.additional_properties[key]

    def __contains__(self, key: str) -> bool:
        return key in self.additional_properties
