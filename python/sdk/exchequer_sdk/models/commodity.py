from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.commodity_kind import CommodityKind

T = TypeVar("T", bound="Commodity")


@_attrs_define
class Commodity:
    """
    Attributes:
        code (str): `USD`, `EUR`, `VTI`, `BRK.B`…
        id (UUID):
        kind (CommodityKind):
        precision (int): Decimal places a quantity may have: 2 for USD, 0 for JPY.
    """

    code: str
    id: UUID
    kind: CommodityKind
    precision: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        code = self.code

        id = str(self.id)

        kind = self.kind.value

        precision = self.precision

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "code": code,
                "id": id,
                "kind": kind,
                "precision": precision,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        code = d.pop("code")

        id = UUID(d.pop("id"))

        kind = CommodityKind(d.pop("kind"))

        precision = d.pop("precision")

        commodity = cls(
            code=code,
            id=id,
            kind=kind,
            precision=precision,
        )

        commodity.additional_properties = d
        return commodity

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
