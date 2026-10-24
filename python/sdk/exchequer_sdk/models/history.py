from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.statement import Statement
    from ..models.unpriced import Unpriced


T = TypeVar("T", bound="History")


@_attrs_define
class History:
    """
    Attributes:
        currency_id (UUID):
        fetching (bool): Missing or stale prices are being fetched: asked again shortly, the
            figures may be fresher.
        months (list[Statement]): One statement per month, without lines.
        unpriced (list[Unpriced]):
    """

    currency_id: UUID
    fetching: bool
    months: list[Statement]
    unpriced: list[Unpriced]
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        currency_id = str(self.currency_id)

        fetching = self.fetching

        months = []
        for months_item_data in self.months:
            months_item = months_item_data.to_dict()
            months.append(months_item)

        unpriced = []
        for unpriced_item_data in self.unpriced:
            unpriced_item = unpriced_item_data.to_dict()
            unpriced.append(unpriced_item)

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "currency_id": currency_id,
                "fetching": fetching,
                "months": months,
                "unpriced": unpriced,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.statement import Statement  # noqa: PLC0415
        from ..models.unpriced import Unpriced  # noqa: PLC0415

        d = dict(src_dict)
        currency_id = UUID(d.pop("currency_id"))

        fetching = d.pop("fetching")

        months = []
        _months = d.pop("months")
        for months_item_data in _months:
            months_item = Statement.from_dict(months_item_data)

            months.append(months_item)

        unpriced = []
        _unpriced = d.pop("unpriced")
        for unpriced_item_data in _unpriced:
            unpriced_item = Unpriced.from_dict(unpriced_item_data)

            unpriced.append(unpriced_item)

        history = cls(
            currency_id=currency_id,
            fetching=fetching,
            months=months,
            unpriced=unpriced,
        )

        history.additional_properties = d
        return history

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
