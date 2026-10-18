from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="RowSplit")


@_attrs_define
class RowSplit:
    """One part of a row split before commit: a split of the new transaction
    between the statement's account and this part's other side.

        Attributes:
            amount (str): Positive; the parts add up to the row's amount, unsigned. Example: 500.00.
            category_id (None | Unset | UUID): Absent: none. Not for a transfer (unless a liquidation).
            counterparty_id (None | Unset | UUID): Who's on the other side; with neither this nor
                `transfer_account_id`, outside the household, unknown who.
            description (None | str | Unset): The split's own note, 1-500 characters.
            purpose_id (None | Unset | UUID): Absent: the account's owner. Not for a transfer (unless a
                liquidation).
            transfer_account_id (None | Unset | UUID): The other side when it's one of your accounts.
    """

    amount: str
    category_id: None | Unset | UUID = UNSET
    counterparty_id: None | Unset | UUID = UNSET
    description: None | str | Unset = UNSET
    purpose_id: None | Unset | UUID = UNSET
    transfer_account_id: None | Unset | UUID = UNSET

    def to_dict(self) -> dict[str, Any]:
        amount = self.amount

        category_id: None | str | Unset
        if isinstance(self.category_id, Unset):
            category_id = UNSET
        elif isinstance(self.category_id, UUID):
            category_id = str(self.category_id)
        else:
            category_id = self.category_id

        counterparty_id: None | str | Unset
        if isinstance(self.counterparty_id, Unset):
            counterparty_id = UNSET
        elif isinstance(self.counterparty_id, UUID):
            counterparty_id = str(self.counterparty_id)
        else:
            counterparty_id = self.counterparty_id

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        purpose_id: None | str | Unset
        if isinstance(self.purpose_id, Unset):
            purpose_id = UNSET
        elif isinstance(self.purpose_id, UUID):
            purpose_id = str(self.purpose_id)
        else:
            purpose_id = self.purpose_id

        transfer_account_id: None | str | Unset
        if isinstance(self.transfer_account_id, Unset):
            transfer_account_id = UNSET
        elif isinstance(self.transfer_account_id, UUID):
            transfer_account_id = str(self.transfer_account_id)
        else:
            transfer_account_id = self.transfer_account_id

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "amount": amount,
            }
        )
        if category_id is not UNSET:
            field_dict["category_id"] = category_id
        if counterparty_id is not UNSET:
            field_dict["counterparty_id"] = counterparty_id
        if description is not UNSET:
            field_dict["description"] = description
        if purpose_id is not UNSET:
            field_dict["purpose_id"] = purpose_id
        if transfer_account_id is not UNSET:
            field_dict["transfer_account_id"] = transfer_account_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        amount = d.pop("amount")

        def _parse_category_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                category_id_type_0 = UUID(data)

                return category_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        category_id = _parse_category_id(d.pop("category_id", UNSET))

        def _parse_counterparty_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                counterparty_id_type_0 = UUID(data)

                return counterparty_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        counterparty_id = _parse_counterparty_id(d.pop("counterparty_id", UNSET))

        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))

        def _parse_purpose_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                purpose_id_type_0 = UUID(data)

                return purpose_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        purpose_id = _parse_purpose_id(d.pop("purpose_id", UNSET))

        def _parse_transfer_account_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                transfer_account_id_type_0 = UUID(data)

                return transfer_account_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        transfer_account_id = _parse_transfer_account_id(d.pop("transfer_account_id", UNSET))

        row_split = cls(
            amount=amount,
            category_id=category_id,
            counterparty_id=counterparty_id,
            description=description,
            purpose_id=purpose_id,
            transfer_account_id=transfer_account_id,
        )

        return row_split
