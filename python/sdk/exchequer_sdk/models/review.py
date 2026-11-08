from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define

if TYPE_CHECKING:
    from ..models.row_ref import RowRef


T = TypeVar("T", bound="Review")


@_attrs_define
class Review:
    """Rows to tick as reviewed, or untick.

    Attributes:
        reviewed (bool): `true` ticks the rows that don't need a label or a decision (the
            others stay as they are); `false` unticks them all.
        rows (list[RowRef]): The rows, as you saw them; at most 500.
    """

    reviewed: bool
    rows: list[RowRef]

    def to_dict(self) -> dict[str, Any]:
        reviewed = self.reviewed

        rows = []
        for rows_item_data in self.rows:
            rows_item = rows_item_data.to_dict()
            rows.append(rows_item)

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "reviewed": reviewed,
                "rows": rows,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.row_ref import RowRef  # noqa: PLC0415

        d = dict(src_dict)
        reviewed = d.pop("reviewed")

        rows = []
        _rows = d.pop("rows")
        for rows_item_data in _rows:
            rows_item = RowRef.from_dict(rows_item_data)

            rows.append(rows_item)

        review = cls(
            reviewed=reviewed,
            rows=rows,
        )

        return review
