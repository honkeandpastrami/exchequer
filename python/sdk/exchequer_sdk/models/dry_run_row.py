from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.dry_run_change import DryRunChange


T = TypeVar("T", bound="DryRunRow")


@_attrs_define
class DryRunRow:
    """A transaction the script would label differently, or fails on.

    Attributes:
        account_id (UUID):
        changes (list[DryRunChange]):
        date (datetime.date):
        description (str):
        transaction_id (UUID):
        error (None | str | Unset):
    """

    account_id: UUID
    changes: list[DryRunChange]
    date: datetime.date
    description: str
    transaction_id: UUID
    error: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        account_id = str(self.account_id)

        changes = []
        for changes_item_data in self.changes:
            changes_item = changes_item_data.to_dict()
            changes.append(changes_item)

        date = self.date.isoformat()

        description = self.description

        transaction_id = str(self.transaction_id)

        error: None | str | Unset
        if isinstance(self.error, Unset):
            error = UNSET
        else:
            error = self.error

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "account_id": account_id,
                "changes": changes,
                "date": date,
                "description": description,
                "transaction_id": transaction_id,
            }
        )
        if error is not UNSET:
            field_dict["error"] = error

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.dry_run_change import DryRunChange  # noqa: PLC0415

        d = dict(src_dict)
        account_id = UUID(d.pop("account_id"))

        changes = []
        _changes = d.pop("changes")
        for changes_item_data in _changes:
            changes_item = DryRunChange.from_dict(changes_item_data)

            changes.append(changes_item)

        date = datetime.date.fromisoformat(d.pop("date"))

        description = d.pop("description")

        transaction_id = UUID(d.pop("transaction_id"))

        def _parse_error(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        error = _parse_error(d.pop("error", UNSET))

        dry_run_row = cls(
            account_id=account_id,
            changes=changes,
            date=date,
            description=description,
            transaction_id=transaction_id,
            error=error,
        )

        dry_run_row.additional_properties = d
        return dry_run_row

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
