from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="StatementUpdate")


@_attrs_define
class StatementUpdate:
    """One of the two: pick the account, or tick or untick its suggestion.
    Picking an account, or ticking its suggestion, also adds the file's
    account number to it, so its next files resolve by themselves.

        Attributes:
            account_id (None | Unset | UUID): The account the statement is for; `null` to unset it.
            reviewed (bool | None | Unset): `true` ticks the suggested account as reviewed: it becomes the
                statement's own. `false` unticks it: back to the suggestion.
    """

    account_id: None | Unset | UUID = UNSET
    reviewed: bool | None | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        account_id: None | str | Unset
        if isinstance(self.account_id, Unset):
            account_id = UNSET
        elif isinstance(self.account_id, UUID):
            account_id = str(self.account_id)
        else:
            account_id = self.account_id

        reviewed: bool | None | Unset
        if isinstance(self.reviewed, Unset):
            reviewed = UNSET
        else:
            reviewed = self.reviewed

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if account_id is not UNSET:
            field_dict["account_id"] = account_id
        if reviewed is not UNSET:
            field_dict["reviewed"] = reviewed

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

        def _parse_reviewed(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        reviewed = _parse_reviewed(d.pop("reviewed", UNSET))

        statement_update = cls(
            account_id=account_id,
            reviewed=reviewed,
        )

        return statement_update
