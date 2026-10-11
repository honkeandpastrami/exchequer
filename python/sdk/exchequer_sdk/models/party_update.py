from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="PartyUpdate")


@_attrs_define
class PartyUpdate:
    """
    Attributes:
        currency_id (None | Unset | UUID): Household only; `null` clears it.
        name (None | str | Unset):
    """

    currency_id: None | Unset | UUID = UNSET
    name: None | str | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        currency_id: None | str | Unset
        if isinstance(self.currency_id, Unset):
            currency_id = UNSET
        elif isinstance(self.currency_id, UUID):
            currency_id = str(self.currency_id)
        else:
            currency_id = self.currency_id

        name: None | str | Unset
        if isinstance(self.name, Unset):
            name = UNSET
        else:
            name = self.name

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if currency_id is not UNSET:
            field_dict["currency_id"] = currency_id
        if name is not UNSET:
            field_dict["name"] = name

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)

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

        def _parse_name(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        name = _parse_name(d.pop("name", UNSET))

        party_update = cls(
            currency_id=currency_id,
            name=name,
        )

        return party_update
