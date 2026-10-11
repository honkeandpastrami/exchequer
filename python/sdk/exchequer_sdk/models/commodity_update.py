from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define

from ..models.commodity_kind import CommodityKind
from ..types import UNSET, Unset

T = TypeVar("T", bound="CommodityUpdate")


@_attrs_define
class CommodityUpdate:
    """Fields to change; omitted ones stay as they are.

    Attributes:
        code (None | str | Unset):
        kind (CommodityKind | None | Unset):
        precision (int | None | Unset): Can only increase.
    """

    code: None | str | Unset = UNSET
    kind: CommodityKind | None | Unset = UNSET
    precision: int | None | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        code: None | str | Unset
        if isinstance(self.code, Unset):
            code = UNSET
        else:
            code = self.code

        kind: None | str | Unset
        if isinstance(self.kind, Unset):
            kind = UNSET
        elif isinstance(self.kind, CommodityKind):
            kind = self.kind.value
        else:
            kind = self.kind

        precision: int | None | Unset
        if isinstance(self.precision, Unset):
            precision = UNSET
        else:
            precision = self.precision

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if code is not UNSET:
            field_dict["code"] = code
        if kind is not UNSET:
            field_dict["kind"] = kind
        if precision is not UNSET:
            field_dict["precision"] = precision

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)

        def _parse_code(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        code = _parse_code(d.pop("code", UNSET))

        def _parse_kind(data: object) -> CommodityKind | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                kind_type_0 = CommodityKind(data)

                return kind_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(CommodityKind | None | Unset, data)

        kind = _parse_kind(d.pop("kind", UNSET))

        def _parse_precision(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        precision = _parse_precision(d.pop("precision", UNSET))

        commodity_update = cls(
            code=code,
            kind=kind,
            precision=precision,
        )

        return commodity_update
