from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.import_fingerprint import ImportFingerprint


T = TypeVar("T", bound="ImportFingerprintsAdded")


@_attrs_define
class ImportFingerprintsAdded:
    """
    Attributes:
        added (int): Recorded now.
        conflicts (list[ImportFingerprint]): Already recorded for another transaction: left as they were.
        unchanged (int): Already recorded for the same transaction.
    """

    added: int
    conflicts: list[ImportFingerprint]
    unchanged: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        added = self.added

        conflicts = []
        for conflicts_item_data in self.conflicts:
            conflicts_item = conflicts_item_data.to_dict()
            conflicts.append(conflicts_item)

        unchanged = self.unchanged

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "added": added,
                "conflicts": conflicts,
                "unchanged": unchanged,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.import_fingerprint import ImportFingerprint  # noqa: PLC0415

        d = dict(src_dict)
        added = d.pop("added")

        conflicts = []
        _conflicts = d.pop("conflicts")
        for conflicts_item_data in _conflicts:
            conflicts_item = ImportFingerprint.from_dict(conflicts_item_data)

            conflicts.append(conflicts_item)

        unchanged = d.pop("unchanged")

        import_fingerprints_added = cls(
            added=added,
            conflicts=conflicts,
            unchanged=unchanged,
        )

        import_fingerprints_added.additional_properties = d
        return import_fingerprints_added

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
