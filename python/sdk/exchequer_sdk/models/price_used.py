from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="PriceUsed")


@_attrs_define
class PriceUsed:
    """A stored price as a valuation used it.

    Attributes:
        base_id (UUID):
        date (datetime.date):
        id (UUID):
        inverted (bool): Used as `1 quote = 1/rate base`.
        quote_id (UUID):
        rate (str):
        source (str):
    """

    base_id: UUID
    date: datetime.date
    id: UUID
    inverted: bool
    quote_id: UUID
    rate: str
    source: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        base_id = str(self.base_id)

        date = self.date.isoformat()

        id = str(self.id)

        inverted = self.inverted

        quote_id = str(self.quote_id)

        rate = self.rate

        source = self.source

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "base_id": base_id,
                "date": date,
                "id": id,
                "inverted": inverted,
                "quote_id": quote_id,
                "rate": rate,
                "source": source,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        base_id = UUID(d.pop("base_id"))

        date = datetime.date.fromisoformat(d.pop("date"))

        id = UUID(d.pop("id"))

        inverted = d.pop("inverted")

        quote_id = UUID(d.pop("quote_id"))

        rate = d.pop("rate")

        source = d.pop("source")

        price_used = cls(
            base_id=base_id,
            date=date,
            id=id,
            inverted=inverted,
            quote_id=quote_id,
            rate=rate,
            source=source,
        )

        price_used.additional_properties = d
        return price_used

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
