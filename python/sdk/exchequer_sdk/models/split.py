from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.kind import Kind
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.leg import Leg


T = TypeVar("T", bound="Split")


@_attrs_define
class Split:
    """
    Attributes:
        from_ (Leg): One side of a split: `account_id` (yours) or `counterparty_id` (outside
            the household). With neither, it's outside the household, unknown who.
        kind (Kind): Derived from who is on each side; never stored or entered.
        to (Leg): One side of a split: `account_id` (yours) or `counterparty_id` (outside
            the household). With neither, it's outside the household, unknown who.
        category_id (None | Unset | UUID):
        description (None | str | Unset):
        purpose_id (None | Unset | UUID):
        settles (None | str | Unset):  Example: 2026-06.
    """

    from_: Leg
    kind: Kind
    to: Leg
    category_id: None | Unset | UUID = UNSET
    description: None | str | Unset = UNSET
    purpose_id: None | Unset | UUID = UNSET
    settles: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from_ = self.from_.to_dict()

        kind = self.kind.value

        to = self.to.to_dict()

        category_id: None | str | Unset
        if isinstance(self.category_id, Unset):
            category_id = UNSET
        elif isinstance(self.category_id, UUID):
            category_id = str(self.category_id)
        else:
            category_id = self.category_id

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        purpose_id: None | str | Unset
        if isinstance(self.purpose_id, Unset):
            purpose_id = UNSET
        elif isinstance(self.purpose_id, UUID):
            purpose_id = str(self.purpose_id)
        else:
            purpose_id = self.purpose_id

        settles: None | str | Unset
        if isinstance(self.settles, Unset):
            settles = UNSET
        else:
            settles = self.settles

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "from": from_,
                "kind": kind,
                "to": to,
            }
        )
        if category_id is not UNSET:
            field_dict["category_id"] = category_id
        if description is not UNSET:
            field_dict["description"] = description
        if purpose_id is not UNSET:
            field_dict["purpose_id"] = purpose_id
        if settles is not UNSET:
            field_dict["settles"] = settles

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.leg import Leg  # noqa: PLC0415

        d = dict(src_dict)
        from_ = Leg.from_dict(d.pop("from"))

        kind = Kind(d.pop("kind"))

        to = Leg.from_dict(d.pop("to"))

        def _parse_category_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                category_id_type_0 = UUID(data)

                return category_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        category_id = _parse_category_id(d.pop("category_id", UNSET))

        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))

        def _parse_purpose_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                purpose_id_type_0 = UUID(data)

                return purpose_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        purpose_id = _parse_purpose_id(d.pop("purpose_id", UNSET))

        def _parse_settles(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        settles = _parse_settles(d.pop("settles", UNSET))

        split = cls(
            from_=from_,
            kind=kind,
            to=to,
            category_id=category_id,
            description=description,
            purpose_id=purpose_id,
            settles=settles,
        )

        split.additional_properties = d
        return split

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
