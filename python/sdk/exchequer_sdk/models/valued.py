from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.price_used import PriceUsed


T = TypeVar("T", bound="Valued")


@_attrs_define
class Valued:
    """A valuation, and whether fresher prices for it are being fetched.

    Attributes:
        derived (bool): Chained through another commodity, not one price.
        prices (list[PriceUsed]): One price, or two chained through a pivot commodity (`derived`).
        provisional (bool): Uses a price for a day still in progress.
        rate (str): `1 from = rate to`. Example: 312.45.
        stale (bool): No price close enough to the date: the newest older one was used.
        fetching (bool): Missing or stale prices are being fetched: once
            `GET /prices/fetching` says they're done, the answer may be fresher.
    """

    derived: bool
    prices: list[PriceUsed]
    provisional: bool
    rate: str
    stale: bool
    fetching: bool
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        derived = self.derived

        prices = []
        for prices_item_data in self.prices:
            prices_item = prices_item_data.to_dict()
            prices.append(prices_item)

        provisional = self.provisional

        rate = self.rate

        stale = self.stale

        fetching = self.fetching

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "derived": derived,
                "prices": prices,
                "provisional": provisional,
                "rate": rate,
                "stale": stale,
                "fetching": fetching,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.price_used import PriceUsed  # noqa: PLC0415

        d = dict(src_dict)
        derived = d.pop("derived")

        prices = []
        _prices = d.pop("prices")
        for prices_item_data in _prices:
            prices_item = PriceUsed.from_dict(prices_item_data)

            prices.append(prices_item)

        provisional = d.pop("provisional")

        rate = d.pop("rate")

        stale = d.pop("stale")

        fetching = d.pop("fetching")

        valued = cls(
            derived=derived,
            prices=prices,
            provisional=provisional,
            rate=rate,
            stale=stale,
            fetching=fetching,
        )

        valued.additional_properties = d
        return valued

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
