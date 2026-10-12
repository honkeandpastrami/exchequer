from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.suggester_kind import SuggesterKind

if TYPE_CHECKING:
    from ..models.suggester_details import SuggesterDetails


T = TypeVar("T", bound="Suggester")


@_attrs_define
class Suggester:
    """The exact system that made a suggestion.

    Attributes:
        details (SuggesterDetails): What else identifies it, e.g. the data a local model learned from.
        id (UUID):
        kind (SuggesterKind): Which level of the suggestion stack made a suggestion (docs/ai.md).
        name (str): E.g. the rules, a local method, or a remote deployment and model.
        version (str):
    """

    details: SuggesterDetails
    id: UUID
    kind: SuggesterKind
    name: str
    version: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        details = self.details.to_dict()

        id = str(self.id)

        kind = self.kind.value

        name = self.name

        version = self.version

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "details": details,
                "id": id,
                "kind": kind,
                "name": name,
                "version": version,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.suggester_details import SuggesterDetails  # noqa: PLC0415

        d = dict(src_dict)
        details = SuggesterDetails.from_dict(d.pop("details"))

        id = UUID(d.pop("id"))

        kind = SuggesterKind(d.pop("kind"))

        name = d.pop("name")

        version = d.pop("version")

        suggester = cls(
            details=details,
            id=id,
            kind=kind,
            name=name,
            version=version,
        )

        suggester.additional_properties = d
        return suggester

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
