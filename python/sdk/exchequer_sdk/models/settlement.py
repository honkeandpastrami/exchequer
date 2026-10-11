from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.funder import Funder
    from ..models.stipend import Stipend


T = TypeVar("T", bound="Settlement")


@_attrs_define
class Settlement:
    """A month's parameters and state.

    Attributes:
        closed (bool):
        funders (list[Funder]):
        month (str):  Example: 2026-06.
        stipends (list[Stipend]):
        closed_at (datetime.datetime | None | Unset):
        params_from (None | str | Unset): The month these parameters were set for: this one, an earlier one
            they're carried forward from, or absent when none were ever set.
    """

    closed: bool
    funders: list[Funder]
    month: str
    stipends: list[Stipend]
    closed_at: datetime.datetime | None | Unset = UNSET
    params_from: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        closed = self.closed

        funders = []
        for funders_item_data in self.funders:
            funders_item = funders_item_data.to_dict()
            funders.append(funders_item)

        month = self.month

        stipends = []
        for stipends_item_data in self.stipends:
            stipends_item = stipends_item_data.to_dict()
            stipends.append(stipends_item)

        closed_at: None | str | Unset
        if isinstance(self.closed_at, Unset):
            closed_at = UNSET
        elif isinstance(self.closed_at, datetime.datetime):
            closed_at = self.closed_at.isoformat()
        else:
            closed_at = self.closed_at

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
                "funders": funders,
                "month": month,
                "stipends": stipends,
            }
        )
        if closed_at is not UNSET:
            field_dict["closed_at"] = closed_at
        if params_from is not UNSET:
            field_dict["params_from"] = params_from

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.funder import Funder  # noqa: PLC0415
        from ..models.stipend import Stipend  # noqa: PLC0415

        d = dict(src_dict)
        closed = d.pop("closed")

        funders = []
        _funders = d.pop("funders")
        for funders_item_data in _funders:
            funders_item = Funder.from_dict(funders_item_data)

            funders.append(funders_item)

        month = d.pop("month")

        stipends = []
        _stipends = d.pop("stipends")
        for stipends_item_data in _stipends:
            stipends_item = Stipend.from_dict(stipends_item_data)

            stipends.append(stipends_item)

        def _parse_closed_at(data: object) -> datetime.datetime | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                closed_at_type_0 = datetime.datetime.fromisoformat(data)

                return closed_at_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(datetime.datetime | None | Unset, data)

        closed_at = _parse_closed_at(d.pop("closed_at", UNSET))

        def _parse_params_from(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        params_from = _parse_params_from(d.pop("params_from", UNSET))

        settlement = cls(
            closed=closed,
            funders=funders,
            month=month,
            stipends=stipends,
            closed_at=closed_at,
            params_from=params_from,
        )

        settlement.additional_properties = d
        return settlement

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
