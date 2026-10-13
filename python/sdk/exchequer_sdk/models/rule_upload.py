from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="RuleUpload")


@_attrs_define
class RuleUpload:
    """
    Attributes:
        source (str): The whole script; empty for no rules.
        note (None | str | Unset): Why, like a commit message (at most 500 characters).
    """

    source: str
    note: None | str | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        source = self.source

        note: None | str | Unset
        if isinstance(self.note, Unset):
            note = UNSET
        else:
            note = self.note

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "source": source,
            }
        )
        if note is not UNSET:
            field_dict["note"] = note

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        source = d.pop("source")

        def _parse_note(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        note = _parse_note(d.pop("note", UNSET))

        rule_upload = cls(
            source=source,
            note=note,
        )

        return rule_upload
