from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define

from ..models.batch_kind import BatchKind
from ..types import UNSET, Unset

T = TypeVar("T", bound="NewImportBatch")


@_attrs_define
class NewImportBatch:
    """
    Attributes:
        kind (BatchKind):
        label (None | str | Unset): 1-200 characters, e.g. where the transactions came from.
    """

    kind: BatchKind
    label: None | str | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        kind = self.kind.value

        label: None | str | Unset
        if isinstance(self.label, Unset):
            label = UNSET
        else:
            label = self.label

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "kind": kind,
            }
        )
        if label is not UNSET:
            field_dict["label"] = label

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        kind = BatchKind(d.pop("kind"))

        def _parse_label(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        label = _parse_label(d.pop("label", UNSET))

        new_import_batch = cls(
            kind=kind,
            label=label,
        )

        return new_import_batch
