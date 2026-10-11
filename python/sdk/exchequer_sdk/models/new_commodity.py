from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

from ..models.commodity_kind import CommodityKind

T = TypeVar("T", bound="NewCommodity")


@_attrs_define
class NewCommodity:
    """
    Attributes:
        code (str):
        kind (CommodityKind):
        precision (int):
    """

    code: str
    kind: CommodityKind
    precision: int

    def to_dict(self) -> dict[str, Any]:
        code = self.code

        kind = self.kind.value

        precision = self.precision

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "code": code,
                "kind": kind,
                "precision": precision,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        code = d.pop("code")

        kind = CommodityKind(d.pop("kind"))

        precision = d.pop("precision")

        new_commodity = cls(
            code=code,
            kind=kind,
            precision=precision,
        )

        return new_commodity
