from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="Invite")


@_attrs_define
class Invite:
    """A one-time link's secret: the web app opens it at `#/invite/<code>`.

    Attributes:
        code (str): Shown only in this response.
        expires_at (datetime.datetime):
        replace (bool): Using it removes the user's other passkeys and revokes their tokens.
        user_id (UUID):
    """

    code: str
    expires_at: datetime.datetime
    replace: bool
    user_id: UUID
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        code = self.code

        expires_at = self.expires_at.isoformat()

        replace = self.replace

        user_id = str(self.user_id)

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "code": code,
                "expires_at": expires_at,
                "replace": replace,
                "user_id": user_id,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        code = d.pop("code")

        expires_at = datetime.datetime.fromisoformat(d.pop("expires_at"))

        replace = d.pop("replace")

        user_id = UUID(d.pop("user_id"))

        invite = cls(
            code=code,
            expires_at=expires_at,
            replace=replace,
            user_id=user_id,
        )

        invite.additional_properties = d
        return invite

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
