from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define

T = TypeVar("T", bound="NewPrice")


@_attrs_define
class NewPrice:
    """
    Attributes:
        base_id (UUID):
        date (datetime.date):
        quote_id (UUID):
        rate (str):  Example: 1.0842.
    """

    base_id: UUID
    date: datetime.date
    quote_id: UUID
    rate: str

    def to_dict(self) -> dict[str, Any]:
        base_id = str(self.base_id)

        date = self.date.isoformat()

        quote_id = str(self.quote_id)

        rate = self.rate

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "base_id": base_id,
                "date": date,
                "quote_id": quote_id,
                "rate": rate,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        base_id = UUID(d.pop("base_id"))

        date = datetime.date.fromisoformat(d.pop("date"))

        quote_id = UUID(d.pop("quote_id"))

        rate = d.pop("rate")

        new_price = cls(
            base_id=base_id,
            date=date,
            quote_id=quote_id,
            rate=rate,
        )

        return new_price
