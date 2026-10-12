from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.imported_row_raw_type_0 import ImportedRowRawType0


T = TypeVar("T", bound="ImportedRow")


@_attrs_define
class ImportedRow:
    """A bank row behind a transaction, and a file it came in.

    Attributes:
        account_id (UUID):
        fingerprint (str): The row's ID: the bank's own, or a hash of its date, description and
            amount.
        file_sha256 (None | str | Unset): The file it came in (its SHA-256); absent for rows recorded another
            way, such as by a migration.
        imported_at (datetime.datetime | None | Unset):
        raw (ImportedRowRawType0 | None | Unset): The row's fields as that file had them.
    """

    account_id: UUID
    fingerprint: str
    file_sha256: None | str | Unset = UNSET
    imported_at: datetime.datetime | None | Unset = UNSET
    raw: ImportedRowRawType0 | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.imported_row_raw_type_0 import ImportedRowRawType0  # noqa: PLC0415

        account_id = str(self.account_id)

        fingerprint = self.fingerprint

        file_sha256: None | str | Unset
        if isinstance(self.file_sha256, Unset):
            file_sha256 = UNSET
        else:
            file_sha256 = self.file_sha256

        imported_at: None | str | Unset
        if isinstance(self.imported_at, Unset):
            imported_at = UNSET
        elif isinstance(self.imported_at, datetime.datetime):
            imported_at = self.imported_at.isoformat()
        else:
            imported_at = self.imported_at

        raw: dict[str, Any] | None | Unset
        if isinstance(self.raw, Unset):
            raw = UNSET
        elif isinstance(self.raw, ImportedRowRawType0):
            raw = self.raw.to_dict()
        else:
            raw = self.raw

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "account_id": account_id,
                "fingerprint": fingerprint,
            }
        )
        if file_sha256 is not UNSET:
            field_dict["file_sha256"] = file_sha256
        if imported_at is not UNSET:
            field_dict["imported_at"] = imported_at
        if raw is not UNSET:
            field_dict["raw"] = raw

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.imported_row_raw_type_0 import ImportedRowRawType0  # noqa: PLC0415

        d = dict(src_dict)
        account_id = UUID(d.pop("account_id"))

        fingerprint = d.pop("fingerprint")

        def _parse_file_sha256(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        file_sha256 = _parse_file_sha256(d.pop("file_sha256", UNSET))

        def _parse_imported_at(data: object) -> datetime.datetime | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                imported_at_type_0 = datetime.datetime.fromisoformat(data)

                return imported_at_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(datetime.datetime | None | Unset, data)

        imported_at = _parse_imported_at(d.pop("imported_at", UNSET))

        def _parse_raw(data: object) -> ImportedRowRawType0 | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                raw_type_0 = ImportedRowRawType0.from_dict(data)

                return raw_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ImportedRowRawType0 | None | Unset, data)

        raw = _parse_raw(d.pop("raw", UNSET))

        imported_row = cls(
            account_id=account_id,
            fingerprint=fingerprint,
            file_sha256=file_sha256,
            imported_at=imported_at,
            raw=raw,
        )

        imported_row.additional_properties = d
        return imported_row

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
