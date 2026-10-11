from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

from ..models.party_kind import PartyKind

T = TypeVar("T", bound="NewParty")


@_attrs_define
class NewParty:
    """
    Attributes:
        kind (PartyKind):
        name (str):
    """

    kind: PartyKind
    name: str

    def to_dict(self) -> dict[str, Any]:
        kind = self.kind.value

        name = self.name

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "kind": kind,
                "name": name,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        kind = PartyKind(d.pop("kind"))

        name = d.pop("name")

        new_party = cls(
            kind=kind,
            name=name,
        )

        return new_party
