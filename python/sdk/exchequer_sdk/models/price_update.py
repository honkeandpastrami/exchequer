from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

T = TypeVar("T", bound="PriceUpdate")


@_attrs_define
class PriceUpdate:
    """
    Attributes:
        rate (str):  Example: 1.0842.
    """

    rate: str

    def to_dict(self) -> dict[str, Any]:
        rate = self.rate

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "rate": rate,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        rate = d.pop("rate")

        price_update = cls(
            rate=rate,
        )

        return price_update
