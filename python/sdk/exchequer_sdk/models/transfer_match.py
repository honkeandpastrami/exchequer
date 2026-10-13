from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="TransferMatch")


@_attrs_define
class TransferMatch:
    """A transfer match's other side.

    Attributes:
        account_id (UUID):
        date (datetime.date):
        description (str):
        import_id (UUID):
        position (int):
        statement (int):
        file_name (None | str | Unset): Its file's name, if it was given.
    """

    account_id: UUID
    date: datetime.date
    description: str
    import_id: UUID
    position: int
    statement: int
    file_name: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        account_id = str(self.account_id)

        date = self.date.isoformat()

        description = self.description

        import_id = str(self.import_id)

        position = self.position

        statement = self.statement

        file_name: None | str | Unset
        if isinstance(self.file_name, Unset):
            file_name = UNSET
        else:
            file_name = self.file_name

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "account_id": account_id,
                "date": date,
                "description": description,
                "import_id": import_id,
                "position": position,
                "statement": statement,
            }
        )
        if file_name is not UNSET:
            field_dict["file_name"] = file_name

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        account_id = UUID(d.pop("account_id"))

        date = datetime.date.fromisoformat(d.pop("date"))

        description = d.pop("description")

        import_id = UUID(d.pop("import_id"))

        position = d.pop("position")

        statement = d.pop("statement")

        def _parse_file_name(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        file_name = _parse_file_name(d.pop("file_name", UNSET))

        transfer_match = cls(
            account_id=account_id,
            date=date,
            description=description,
            import_id=import_id,
            position=position,
            statement=statement,
            file_name=file_name,
        )

        transfer_match.additional_properties = d
        return transfer_match

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
