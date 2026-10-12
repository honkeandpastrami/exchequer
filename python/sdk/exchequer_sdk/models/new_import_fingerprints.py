from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define

if TYPE_CHECKING:
    from ..models.import_fingerprint import ImportFingerprint


T = TypeVar("T", bound="NewImportFingerprints")


@_attrs_define
class NewImportFingerprints:
    """
    Attributes:
        items (list[ImportFingerprint]): 1-1000 items, each (account, fingerprint) at most once.
    """

    items: list[ImportFingerprint]

    def to_dict(self) -> dict[str, Any]:
        items = []
        for items_item_data in self.items:
            items_item = items_item_data.to_dict()
            items.append(items_item)

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "items": items,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.import_fingerprint import ImportFingerprint  # noqa: PLC0415

        d = dict(src_dict)
        items = []
        _items = d.pop("items")
        for items_item_data in _items:
            items_item = ImportFingerprint.from_dict(items_item_data)

            items.append(items_item)

        new_import_fingerprints = cls(
            items=items,
        )

        return new_import_fingerprints
