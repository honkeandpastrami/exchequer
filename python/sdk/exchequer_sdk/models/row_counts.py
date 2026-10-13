from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="RowCounts")


@_attrs_define
class RowCounts:
    """How many rows have each status.

    Attributes:
        duplicate (int):
        near_duplicate (int):
        needs_account (int):
        new (int):
        transfer_duplicate (int):
        transfer_match (int):
    """

    duplicate: int
    near_duplicate: int
    needs_account: int
    new: int
    transfer_duplicate: int
    transfer_match: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        duplicate = self.duplicate

        near_duplicate = self.near_duplicate

        needs_account = self.needs_account

        new = self.new

        transfer_duplicate = self.transfer_duplicate

        transfer_match = self.transfer_match

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "duplicate": duplicate,
                "near_duplicate": near_duplicate,
                "needs_account": needs_account,
                "new": new,
                "transfer_duplicate": transfer_duplicate,
                "transfer_match": transfer_match,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        duplicate = d.pop("duplicate")

        near_duplicate = d.pop("near_duplicate")

        needs_account = d.pop("needs_account")

        new = d.pop("new")

        transfer_duplicate = d.pop("transfer_duplicate")

        transfer_match = d.pop("transfer_match")

        row_counts = cls(
            duplicate=duplicate,
            near_duplicate=near_duplicate,
            needs_account=needs_account,
            new=new,
            transfer_duplicate=transfer_duplicate,
            transfer_match=transfer_match,
        )

        row_counts.additional_properties = d
        return row_counts

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
