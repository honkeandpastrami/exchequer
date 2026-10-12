from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.external_ref import ExternalRef
    from ..models.new_split import NewSplit


T = TypeVar("T", bound="NewTransaction")


@_attrs_define
class NewTransaction:
    """
    Attributes:
        date (datetime.date):
        description (str):
        splits (list[NewSplit]):
        external_refs (list[ExternalRef] | Unset):
        import_batch_id (None | Unset | UUID): On create: a migration batch to add the transaction to (see
            `POST /import-batches`); omitted, the transaction gets a batch of its
            own. On replace: omitted or unchanged, since a transaction never
            leaves the batch it entered with.
        notes (None | str | Unset):
        tags (list[str] | Unset): Tag names; unknown ones are created.
    """

    date: datetime.date
    description: str
    splits: list[NewSplit]
    external_refs: list[ExternalRef] | Unset = UNSET
    import_batch_id: None | Unset | UUID = UNSET
    notes: None | str | Unset = UNSET
    tags: list[str] | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        date = self.date.isoformat()

        description = self.description

        splits = []
        for splits_item_data in self.splits:
            splits_item = splits_item_data.to_dict()
            splits.append(splits_item)

        external_refs: list[dict[str, Any]] | Unset = UNSET
        if not isinstance(self.external_refs, Unset):
            external_refs = []
            for external_refs_item_data in self.external_refs:
                external_refs_item = external_refs_item_data.to_dict()
                external_refs.append(external_refs_item)

        import_batch_id: None | str | Unset
        if isinstance(self.import_batch_id, Unset):
            import_batch_id = UNSET
        elif isinstance(self.import_batch_id, UUID):
            import_batch_id = str(self.import_batch_id)
        else:
            import_batch_id = self.import_batch_id

        notes: None | str | Unset
        if isinstance(self.notes, Unset):
            notes = UNSET
        else:
            notes = self.notes

        tags: list[str] | Unset = UNSET
        if not isinstance(self.tags, Unset):
            tags = self.tags

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "date": date,
                "description": description,
                "splits": splits,
            }
        )
        if external_refs is not UNSET:
            field_dict["external_refs"] = external_refs
        if import_batch_id is not UNSET:
            field_dict["import_batch_id"] = import_batch_id
        if notes is not UNSET:
            field_dict["notes"] = notes
        if tags is not UNSET:
            field_dict["tags"] = tags

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.external_ref import ExternalRef  # noqa: PLC0415
        from ..models.new_split import NewSplit  # noqa: PLC0415

        d = dict(src_dict)
        date = datetime.date.fromisoformat(d.pop("date"))

        description = d.pop("description")

        splits = []
        _splits = d.pop("splits")
        for splits_item_data in _splits:
            splits_item = NewSplit.from_dict(splits_item_data)

            splits.append(splits_item)

        _external_refs = d.pop("external_refs", UNSET)
        external_refs: list[ExternalRef] | Unset = UNSET
        if _external_refs is not UNSET:
            external_refs = []
            for external_refs_item_data in _external_refs:
                external_refs_item = ExternalRef.from_dict(external_refs_item_data)

                external_refs.append(external_refs_item)

        def _parse_import_batch_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                import_batch_id_type_0 = UUID(data)

                return import_batch_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        import_batch_id = _parse_import_batch_id(d.pop("import_batch_id", UNSET))

        def _parse_notes(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        notes = _parse_notes(d.pop("notes", UNSET))

        tags = cast(list[str], d.pop("tags", UNSET))

        new_transaction = cls(
            date=date,
            description=description,
            splits=splits,
            external_refs=external_refs,
            import_batch_id=import_batch_id,
            notes=notes,
            tags=tags,
        )

        return new_transaction
