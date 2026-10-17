from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="PagePriceItemsItem")


@_attrs_define
class PagePriceItemsItem:
    """
    Attributes:
        base_id (UUID):
        date (datetime.date): At the end of this day.
        fetched_at (datetime.datetime): When it was entered or fetched.
        id (UUID):
        provisional (bool): For a day still in progress: replaced by the close once available.
        quote_id (UUID):
        rate (str): `1 base = rate quote`. Example: 1.0842.
        source (str): `manual`, or `provider:<name>` (`provider:tiingo`, `provider:ecb`).
    """

    base_id: UUID
    date: datetime.date
    fetched_at: datetime.datetime
    id: UUID
    provisional: bool
    quote_id: UUID
    rate: str
    source: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        base_id = str(self.base_id)

        date = self.date.isoformat()

        fetched_at = self.fetched_at.isoformat()

        id = str(self.id)

        provisional = self.provisional

        quote_id = str(self.quote_id)

        rate = self.rate

        source = self.source

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "base_id": base_id,
                "date": date,
                "fetched_at": fetched_at,
                "id": id,
                "provisional": provisional,
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

        fetched_at = datetime.datetime.fromisoformat(d.pop("fetched_at"))

        id = UUID(d.pop("id"))

        provisional = d.pop("provisional")

        quote_id = UUID(d.pop("quote_id"))

        rate = d.pop("rate")

        source = d.pop("source")

        page_price_items_item = cls(
            base_id=base_id,
            date=date,
            fetched_at=fetched_at,
            id=id,
            provisional=provisional,
            quote_id=quote_id,
            rate=rate,
            source=source,
        )

        page_price_items_item.additional_properties = d
        return page_price_items_item

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
