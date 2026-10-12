from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.external_ref import ExternalRef
    from ..models.split import Split


T = TypeVar("T", bound="Transaction")


@_attrs_define
class Transaction:
    """
    Attributes:
        date (datetime.date):
        description (str):
        external_refs (list[ExternalRef]):
        id (UUID):
        import_batch_id (UUID): Read-only: the import batch the transaction entered with.
        splits (list[Split]):
        tags (list[str]):
        notes (None | str | Unset):
    """

    date: datetime.date
    description: str
    external_refs: list[ExternalRef]
    id: UUID
    import_batch_id: UUID
    splits: list[Split]
    tags: list[str]
    notes: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        date = self.date.isoformat()

        description = self.description

        external_refs = []
        for external_refs_item_data in self.external_refs:
            external_refs_item = external_refs_item_data.to_dict()
            external_refs.append(external_refs_item)

        id = str(self.id)

        import_batch_id = str(self.import_batch_id)

        splits = []
        for splits_item_data in self.splits:
            splits_item = splits_item_data.to_dict()
            splits.append(splits_item)

        tags = self.tags

        notes: None | str | Unset
        if isinstance(self.notes, Unset):
            notes = UNSET
        else:
            notes = self.notes

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "date": date,
                "description": description,
                "external_refs": external_refs,
                "id": id,
                "import_batch_id": import_batch_id,
                "splits": splits,
                "tags": tags,
            }
        )
        if notes is not UNSET:
            field_dict["notes"] = notes

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.external_ref import ExternalRef  # noqa: PLC0415
        from ..models.split import Split  # noqa: PLC0415

        d = dict(src_dict)
        date = datetime.date.fromisoformat(d.pop("date"))

        description = d.pop("description")

        external_refs = []
        _external_refs = d.pop("external_refs")
        for external_refs_item_data in _external_refs:
            external_refs_item = ExternalRef.from_dict(external_refs_item_data)

            external_refs.append(external_refs_item)

        id = UUID(d.pop("id"))

        import_batch_id = UUID(d.pop("import_batch_id"))

        splits = []
        _splits = d.pop("splits")
        for splits_item_data in _splits:
            splits_item = Split.from_dict(splits_item_data)

            splits.append(splits_item)

        tags = cast(list[str], d.pop("tags"))

        def _parse_notes(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        notes = _parse_notes(d.pop("notes", UNSET))

        transaction = cls(
            date=date,
            description=description,
            external_refs=external_refs,
            id=id,
            import_batch_id=import_batch_id,
            splits=splits,
            tags=tags,
            notes=notes,
        )

        transaction.additional_properties = d
        return transaction

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
