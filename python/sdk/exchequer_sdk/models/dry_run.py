from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.dry_run_row import DryRunRow


T = TypeVar("T", bound="DryRun")


@_attrs_define
class DryRun:
    """
    Attributes:
        changed (int): Those it would label differently from the ledger.
        failed (int): Those it failed on.
        items (list[DryRunRow]): Changed and failed rows, by date (at most 500).
        labeled (int): Of them, those it set any label for.
        rows (int): Transactions it ran on: those between one of your accounts and the
            outside, dated in the range.
        truncated (bool):
        version (int | None | Unset): The script's version; absent for a candidate.
    """

    changed: int
    failed: int
    items: list[DryRunRow]
    labeled: int
    rows: int
    truncated: bool
    version: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        changed = self.changed

        failed = self.failed

        items = []
        for items_item_data in self.items:
            items_item = items_item_data.to_dict()
            items.append(items_item)

        labeled = self.labeled

        rows = self.rows

        truncated = self.truncated

        version: int | None | Unset
        if isinstance(self.version, Unset):
            version = UNSET
        else:
            version = self.version

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "changed": changed,
                "failed": failed,
                "items": items,
                "labeled": labeled,
                "rows": rows,
                "truncated": truncated,
            }
        )
        if version is not UNSET:
            field_dict["version"] = version

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.dry_run_row import DryRunRow  # noqa: PLC0415

        d = dict(src_dict)
        changed = d.pop("changed")

        failed = d.pop("failed")

        items = []
        _items = d.pop("items")
        for items_item_data in _items:
            items_item = DryRunRow.from_dict(items_item_data)

            items.append(items_item)

        labeled = d.pop("labeled")

        rows = d.pop("rows")

        truncated = d.pop("truncated")

        def _parse_version(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        version = _parse_version(d.pop("version", UNSET))

        dry_run = cls(
            changed=changed,
            failed=failed,
            items=items,
            labeled=labeled,
            rows=rows,
            truncated=truncated,
            version=version,
        )

        dry_run.additional_properties = d
        return dry_run

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
