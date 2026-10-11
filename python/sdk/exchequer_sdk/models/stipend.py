from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define

T = TypeVar("T", bound="Stipend")


@_attrs_define
class Stipend:
    """What the Household owes a person for the month.

    Attributes:
        amount (str): In the household currency; zero or more. Example: 650.00.
        party_id (UUID):
    """

    amount: str
    party_id: UUID

    def to_dict(self) -> dict[str, Any]:
        amount = self.amount

        party_id = str(self.party_id)

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "amount": amount,
                "party_id": party_id,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        amount = d.pop("amount")

        party_id = UUID(d.pop("party_id"))

        stipend = cls(
            amount=amount,
            party_id=party_id,
        )

        return stipend
