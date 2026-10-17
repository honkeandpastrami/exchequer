from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.file_format import FileFormat
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.readiness import Readiness
    from ..models.staged_statement import StagedStatement


T = TypeVar("T", bound="PageImportItemsItem")


@_attrs_define
class PageImportItemsItem:
    """
    Attributes:
        created_at (datetime.datetime):
        file_sha256 (str): The file's identity: its bytes' SHA-256 (hex), whatever it's called.
        format_ (FileFormat):
        id (UUID):
        readiness (Readiness): An import's readiness, and why.
        statements (list[StagedStatement]):
        file_name (None | str | Unset):
    """

    created_at: datetime.datetime
    file_sha256: str
    format_: FileFormat
    id: UUID
    readiness: Readiness
    statements: list[StagedStatement]
    file_name: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        created_at = self.created_at.isoformat()

        file_sha256 = self.file_sha256

        format_ = self.format_.value

        id = str(self.id)

        readiness = self.readiness.to_dict()

        statements = []
        for statements_item_data in self.statements:
            statements_item = statements_item_data.to_dict()
            statements.append(statements_item)

        file_name: None | str | Unset
        if isinstance(self.file_name, Unset):
            file_name = UNSET
        else:
            file_name = self.file_name

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "created_at": created_at,
                "file_sha256": file_sha256,
                "format": format_,
                "id": id,
                "readiness": readiness,
                "statements": statements,
            }
        )
        if file_name is not UNSET:
            field_dict["file_name"] = file_name

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.readiness import Readiness  # noqa: PLC0415
        from ..models.staged_statement import StagedStatement  # noqa: PLC0415

        d = dict(src_dict)
        created_at = datetime.datetime.fromisoformat(d.pop("created_at"))

        file_sha256 = d.pop("file_sha256")

        format_ = FileFormat(d.pop("format"))

        id = UUID(d.pop("id"))

        readiness = Readiness.from_dict(d.pop("readiness"))

        statements = []
        _statements = d.pop("statements")
        for statements_item_data in _statements:
            statements_item = StagedStatement.from_dict(statements_item_data)

            statements.append(statements_item)

        def _parse_file_name(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        file_name = _parse_file_name(d.pop("file_name", UNSET))

        page_import_items_item = cls(
            created_at=created_at,
            file_sha256=file_sha256,
            format_=format_,
            id=id,
            readiness=readiness,
            statements=statements,
            file_name=file_name,
        )

        page_import_items_item.additional_properties = d
        return page_import_items_item

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
