from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="Unpriced")


@_attrs_define
class Unpriced:
    """A split the settlement couldn't value: neither leg is in USD (prices come
    later).

        Attributes:
            position (int):
            transaction_id (UUID):
            date (datetime.date):
    """

    position: int
    transaction_id: UUID
    date: datetime.date
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        position = self.position

        transaction_id = str(self.transaction_id)

        date = self.date.isoformat()

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "position": position,
                "transaction_id": transaction_id,
                "date": date,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        position = d.pop("position")

        transaction_id = UUID(d.pop("transaction_id"))

        date = datetime.date.fromisoformat(d.pop("date"))

        unpriced = cls(
            position=position,
            transaction_id=transaction_id,
            date=date,
        )

        unpriced.additional_properties = d
        return unpriced

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
