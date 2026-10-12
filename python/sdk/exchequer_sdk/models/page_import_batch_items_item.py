from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.batch_kind import BatchKind
from ..types import UNSET, Unset

T = TypeVar("T", bound="PageImportBatchItemsItem")


@_attrs_define
class PageImportBatchItemsItem:
    """
    Attributes:
        created_at (datetime.datetime):
        id (UUID):
        kind (BatchKind):
        transaction_count (int): How many transactions are in it now.
        label (None | str | Unset):
    """

    created_at: datetime.datetime
    id: UUID
    kind: BatchKind
    transaction_count: int
    label: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        created_at = self.created_at.isoformat()

        id = str(self.id)

        kind = self.kind.value

        transaction_count = self.transaction_count

        label: None | str | Unset
        if isinstance(self.label, Unset):
            label = UNSET
        else:
            label = self.label

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "created_at": created_at,
                "id": id,
                "kind": kind,
                "transaction_count": transaction_count,
            }
        )
        if label is not UNSET:
            field_dict["label"] = label

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        created_at = datetime.datetime.fromisoformat(d.pop("created_at"))

        id = UUID(d.pop("id"))

        kind = BatchKind(d.pop("kind"))

        transaction_count = d.pop("transaction_count")

        def _parse_label(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        label = _parse_label(d.pop("label", UNSET))

        page_import_batch_items_item = cls(
            created_at=created_at,
            id=id,
            kind=kind,
            transaction_count=transaction_count,
            label=label,
        )

        page_import_batch_items_item.additional_properties = d
        return page_import_batch_items_item

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
