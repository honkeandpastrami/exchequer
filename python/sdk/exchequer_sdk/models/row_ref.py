from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

T = TypeVar("T", bound="RowRef")


@_attrs_define
class RowRef:
    """A staged row of the import.

    Attributes:
        position (int):
        statement (int):
    """

    position: int
    statement: int

    def to_dict(self) -> dict[str, Any]:
        position = self.position

        statement = self.statement

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "position": position,
                "statement": statement,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        position = d.pop("position")

        statement = d.pop("statement")

        row_ref = cls(
            position=position,
            statement=statement,
        )

        return row_ref
