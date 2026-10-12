from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.edge import Edge
    from ..models.unpriced import Unpriced


T = TypeVar("T", bound="Statement")


@_attrs_define
class Statement:
    """The statement for one month (design §4.3).

    Attributes:
        closed (bool):
        currency_id (UUID):
        edges (list[Edge]):
        month (str):  Example: 2026-06.
        unpriced (list[Unpriced]): Splits left out because they have no value in USD.
        params_from (None | str | Unset):
    """

    closed: bool
    currency_id: UUID
    edges: list[Edge]
    month: str
    unpriced: list[Unpriced]
    params_from: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        closed = self.closed

        currency_id = str(self.currency_id)

        edges = []
        for edges_item_data in self.edges:
            edges_item = edges_item_data.to_dict()
            edges.append(edges_item)

        month = self.month

        unpriced = []
        for unpriced_item_data in self.unpriced:
            unpriced_item = unpriced_item_data.to_dict()
            unpriced.append(unpriced_item)

        params_from: None | str | Unset
        if isinstance(self.params_from, Unset):
            params_from = UNSET
        else:
            params_from = self.params_from

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "closed": closed,
                "currency_id": currency_id,
                "edges": edges,
                "month": month,
                "unpriced": unpriced,
            }
        )
        if params_from is not UNSET:
            field_dict["params_from"] = params_from

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.edge import Edge  # noqa: PLC0415
        from ..models.unpriced import Unpriced  # noqa: PLC0415

        d = dict(src_dict)
        closed = d.pop("closed")

        currency_id = UUID(d.pop("currency_id"))

        edges = []
        _edges = d.pop("edges")
        for edges_item_data in _edges:
            edges_item = Edge.from_dict(edges_item_data)

            edges.append(edges_item)

        month = d.pop("month")

        unpriced = []
        _unpriced = d.pop("unpriced")
        for unpriced_item_data in _unpriced:
            unpriced_item = Unpriced.from_dict(unpriced_item_data)

            unpriced.append(unpriced_item)

        def _parse_params_from(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        params_from = _parse_params_from(d.pop("params_from", UNSET))

        statement = cls(
            closed=closed,
            currency_id=currency_id,
            edges=edges,
            month=month,
            unpriced=unpriced,
            params_from=params_from,
        )

        statement.additional_properties = d
        return statement

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
