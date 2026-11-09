from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.commodity_amount import CommodityAmount
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
        matched (list[CommodityAmount] | None | Unset): In a list: what its splits that pass the filter add up to, per
            commodity. Those amounts are what a report or a statement with the
            same filter counted of it.
        notes (None | str | Unset):
    """

    date: datetime.date
    description: str
    external_refs: list[ExternalRef]
    id: UUID
    import_batch_id: UUID
    splits: list[Split]
    tags: list[str]
    matched: list[CommodityAmount] | None | Unset = UNSET
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

        matched: list[dict[str, Any]] | None | Unset
        if isinstance(self.matched, Unset):
            matched = UNSET
        elif isinstance(self.matched, list):
            matched = []
            for matched_type_0_item_data in self.matched:
                matched_type_0_item = matched_type_0_item_data.to_dict()
                matched.append(matched_type_0_item)

        else:
            matched = self.matched

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
        if matched is not UNSET:
            field_dict["matched"] = matched
        if notes is not UNSET:
            field_dict["notes"] = notes

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.commodity_amount import CommodityAmount  # noqa: PLC0415
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

        def _parse_matched(data: object) -> list[CommodityAmount] | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                matched_type_0 = []
                _matched_type_0 = data
                for matched_type_0_item_data in _matched_type_0:
                    matched_type_0_item = CommodityAmount.from_dict(matched_type_0_item_data)

                    matched_type_0.append(matched_type_0_item)

                return matched_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(list[CommodityAmount] | None | Unset, data)

        matched = _parse_matched(d.pop("matched", UNSET))

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
            matched=matched,
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
