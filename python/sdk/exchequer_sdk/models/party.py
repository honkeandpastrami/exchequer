from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.party_kind import PartyKind
from ..types import UNSET, Unset

T = TypeVar("T", bound="Party")


@_attrs_define
class Party:
    """
    Attributes:
        id (UUID):
        kind (PartyKind):
        name (str):
        currency_id (None | Unset | UUID): The household currency: what settlement statements are in. Only on
            the household.
    """

    id: UUID
    kind: PartyKind
    name: str
    currency_id: None | Unset | UUID = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        id = str(self.id)

        kind = self.kind.value

        name = self.name

        currency_id: None | str | Unset
        if isinstance(self.currency_id, Unset):
            currency_id = UNSET
        elif isinstance(self.currency_id, UUID):
            currency_id = str(self.currency_id)
        else:
            currency_id = self.currency_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "id": id,
                "kind": kind,
                "name": name,
            }
        )
        if currency_id is not UNSET:
            field_dict["currency_id"] = currency_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        id = UUID(d.pop("id"))

        kind = PartyKind(d.pop("kind"))

        name = d.pop("name")

        def _parse_currency_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                currency_id_type_0 = UUID(data)

                return currency_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        currency_id = _parse_currency_id(d.pop("currency_id", UNSET))

        party = cls(
            id=id,
            kind=kind,
            name=name,
            currency_id=currency_id,
        )

        party.additional_properties = d
        return party

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
