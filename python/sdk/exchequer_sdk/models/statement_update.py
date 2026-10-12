from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="StatementUpdate")


@_attrs_define
class StatementUpdate:
    """
    Attributes:
        account_id (None | Unset | UUID): The account the statement is for; `null` to unset it.
        remember_number (bool | Unset): Also add the file's account number to the account, so its next
            files resolve by themselves.
    """

    account_id: None | Unset | UUID = UNSET
    remember_number: bool | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        account_id: None | str | Unset
        if isinstance(self.account_id, Unset):
            account_id = UNSET
        elif isinstance(self.account_id, UUID):
            account_id = str(self.account_id)
        else:
            account_id = self.account_id

        remember_number = self.remember_number

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if account_id is not UNSET:
            field_dict["account_id"] = account_id
        if remember_number is not UNSET:
            field_dict["remember_number"] = remember_number

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)

        def _parse_account_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                account_id_type_0 = UUID(data)

                return account_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        account_id = _parse_account_id(d.pop("account_id", UNSET))

        remember_number = d.pop("remember_number", UNSET)

        statement_update = cls(
            account_id=account_id,
            remember_number=remember_number,
        )

        return statement_update
