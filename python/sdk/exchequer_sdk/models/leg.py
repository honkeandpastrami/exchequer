from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="Leg")


@_attrs_define
class Leg:
    """One side of a split: `account_id` (yours) or `counterparty_id` (outside
    the household). With neither, it's outside the household, unknown who.

        Attributes:
            commodity_id (UUID):
            quantity (str): Positive decimal string, e.g. `"80.00"`. Example: 80.00.
            account_id (None | Unset | UUID):
            counterparty_id (None | Unset | UUID):
    """

    commodity_id: UUID
    quantity: str
    account_id: None | Unset | UUID = UNSET
    counterparty_id: None | Unset | UUID = UNSET

    def to_dict(self) -> dict[str, Any]:
        commodity_id = str(self.commodity_id)

        quantity = self.quantity

        account_id: None | str | Unset
        if isinstance(self.account_id, Unset):
            account_id = UNSET
        elif isinstance(self.account_id, UUID):
            account_id = str(self.account_id)
        else:
            account_id = self.account_id

        counterparty_id: None | str | Unset
        if isinstance(self.counterparty_id, Unset):
            counterparty_id = UNSET
        elif isinstance(self.counterparty_id, UUID):
            counterparty_id = str(self.counterparty_id)
        else:
            counterparty_id = self.counterparty_id

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "commodity_id": commodity_id,
                "quantity": quantity,
            }
        )
        if account_id is not UNSET:
            field_dict["account_id"] = account_id
        if counterparty_id is not UNSET:
            field_dict["counterparty_id"] = counterparty_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        commodity_id = UUID(d.pop("commodity_id"))

        quantity = d.pop("quantity")

        def _parse_account_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                account_id_type_0 = UUID(data)

                return account_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        account_id = _parse_account_id(d.pop("account_id", UNSET))

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

        leg = cls(
            commodity_id=commodity_id,
            quantity=quantity,
            account_id=account_id,
            counterparty_id=counterparty_id,
        )

        return leg
