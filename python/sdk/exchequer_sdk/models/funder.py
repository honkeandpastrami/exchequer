from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define

T = TypeVar("T", bound="Funder")


@_attrs_define
class Funder:
    """Who funds a business for the month: the household or a person.

    Attributes:
        business_id (UUID):
        funder_id (UUID):
    """

    business_id: UUID
    funder_id: UUID

    def to_dict(self) -> dict[str, Any]:
        business_id = str(self.business_id)

        funder_id = str(self.funder_id)

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "business_id": business_id,
                "funder_id": funder_id,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        business_id = UUID(d.pop("business_id"))

        funder_id = UUID(d.pop("funder_id"))

        funder = cls(
            business_id=business_id,
            funder_id=funder_id,
        )

        return funder
