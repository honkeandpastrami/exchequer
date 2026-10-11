from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.funder import Funder
    from ..models.stipend import Stipend


T = TypeVar("T", bound="SettlementParams")


@_attrs_define
class SettlementParams:
    """
    Attributes:
        funders (list[Funder] | Unset): Businesses left out are funded by the household.
        stipends (list[Stipend] | Unset):
    """

    funders: list[Funder] | Unset = UNSET
    stipends: list[Stipend] | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        funders: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.funders, Unset):
            funders = []
            for funders_item_data in self.funders:
                funders_item = funders_item_data.to_dict()
                funders.append(funders_item)

        stipends: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.stipends, Unset):
            stipends = []
            for stipends_item_data in self.stipends:
                stipends_item = stipends_item_data.to_dict()
                stipends.append(stipends_item)

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if funders is not UNSET:
            field_dict["funders"] = funders
        if stipends is not UNSET:
            field_dict["stipends"] = stipends

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.funder import Funder  # noqa: PLC0415
        from ..models.stipend import Stipend  # noqa: PLC0415

        d = dict(src_dict)
        _funders = d.pop("funders", UNSET)
        funders: list[Funder] | Unset = UNSET
        if _funders is not UNSET:
            funders = []
            for funders_item_data in _funders:
                funders_item = Funder.from_dict(funders_item_data)

                funders.append(funders_item)

        _stipends = d.pop("stipends", UNSET)
        stipends: list[Stipend] | Unset = UNSET
        if _stipends is not UNSET:
            stipends = []
            for stipends_item_data in _stipends:
                stipends_item = Stipend.from_dict(stipends_item_data)

                stipends.append(stipends_item)

        settlement_params = cls(
            funders=funders,
            stipends=stipends,
        )

        return settlement_params
