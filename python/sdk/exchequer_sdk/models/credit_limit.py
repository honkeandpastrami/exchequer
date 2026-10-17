from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define

T = TypeVar("T", bound="CreditLimit")


@_attrs_define
class CreditLimit:
    """A credit card's limit: display metadata, never part of the balance. Only
    a `credit_card` account has one (422 otherwise); changing a card to
    another kind needs the limit cleared, in the same request or before.

        Attributes:
            commodity_id (UUID):
            quantity (str): Positive decimal string, e.g. `"9000.00"`. Example: 9000.00.
    """

    commodity_id: UUID
    quantity: str

    def to_dict(self) -> dict[str, Any]:
        commodity_id = str(self.commodity_id)

        quantity = self.quantity

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "commodity_id": commodity_id,
                "quantity": quantity,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        commodity_id = UUID(d.pop("commodity_id"))

        quantity = d.pop("quantity")

        credit_limit = cls(
            commodity_id=commodity_id,
            quantity=quantity,
        )

        return credit_limit
