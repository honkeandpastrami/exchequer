from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="StateCounts")


@_attrs_define
class StateCounts:
    """How many rows are in each state.

    Attributes:
        needs_you (int):
        not_added (int):
        ready (int):
        to_review (int):
    """

    needs_you: int
    not_added: int
    ready: int
    to_review: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        needs_you = self.needs_you

        not_added = self.not_added

        ready = self.ready

        to_review = self.to_review

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "needs_you": needs_you,
                "not_added": not_added,
                "ready": ready,
                "to_review": to_review,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        needs_you = d.pop("needs_you")

        not_added = d.pop("not_added")

        ready = d.pop("ready")

        to_review = d.pop("to_review")

        state_counts = cls(
            needs_you=needs_you,
            not_added=not_added,
            ready=ready,
            to_review=to_review,
        )

        state_counts.additional_properties = d
        return state_counts

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
