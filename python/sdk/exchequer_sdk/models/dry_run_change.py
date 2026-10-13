from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.label import Label
from ..types import UNSET, Unset

T = TypeVar("T", bound="DryRunChange")


@_attrs_define
class DryRunChange:
    """A label the script would set differently from the ledger.

    Attributes:
        label (Label): A label a row's new transaction gets.
        line (int): The line that sets it.
        current (None | Unset | UUID): What the ledger has; `null` for none.
        proposed (None | Unset | UUID): What the script sets; `null` for none.
    """

    label: Label
    line: int
    current: None | Unset | UUID = UNSET
    proposed: None | Unset | UUID = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        label = self.label.value

        line = self.line

        current: None | str | Unset
        if isinstance(self.current, Unset):
            current = UNSET
        elif isinstance(self.current, UUID):
            current = str(self.current)
        else:
            current = self.current

        proposed: None | str | Unset
        if isinstance(self.proposed, Unset):
            proposed = UNSET
        elif isinstance(self.proposed, UUID):
            proposed = str(self.proposed)
        else:
            proposed = self.proposed

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "label": label,
                "line": line,
            }
        )
        if current is not UNSET:
            field_dict["current"] = current
        if proposed is not UNSET:
            field_dict["proposed"] = proposed

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        label = Label(d.pop("label"))

        line = d.pop("line")

        def _parse_current(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                current_type_0 = UUID(data)

                return current_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        current = _parse_current(d.pop("current", UNSET))

        def _parse_proposed(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                proposed_type_0 = UUID(data)

                return proposed_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        proposed = _parse_proposed(d.pop("proposed", UNSET))

        dry_run_change = cls(
            label=label,
            line=line,
            current=current,
            proposed=proposed,
        )

        dry_run_change.additional_properties = d
        return dry_run_change

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
