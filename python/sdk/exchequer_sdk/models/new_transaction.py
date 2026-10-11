from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

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
        notes (None | str | Unset):
        tags (list[str] | Unset): Tag names; unknown ones are created.
    """

    date: datetime.date
    description: str
    splits: list[NewSplit]
    external_refs: list[ExternalRef] | Unset = UNSET
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
            notes=notes,
            tags=tags,
        )

        return new_transaction
