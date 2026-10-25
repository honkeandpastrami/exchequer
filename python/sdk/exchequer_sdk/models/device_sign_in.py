from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="DeviceSignIn")


@_attrs_define
class DeviceSignIn:
    """The CLI waiting to sign in, as the approver sees it.

    Attributes:
        client (str): What the CLI called itself, e.g. `cli on laptop`.
        expires_at (datetime.datetime):
        ttl_days (int): How many days its token will work.
    """

    client: str
    expires_at: datetime.datetime
    ttl_days: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        client = self.client

        expires_at = self.expires_at.isoformat()

        ttl_days = self.ttl_days

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "client": client,
                "expires_at": expires_at,
                "ttl_days": ttl_days,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        client = d.pop("client")

        expires_at = datetime.datetime.fromisoformat(d.pop("expires_at"))

        ttl_days = d.pop("ttl_days")

        device_sign_in = cls(
            client=client,
            expires_at=expires_at,
            ttl_days=ttl_days,
        )

        device_sign_in.additional_properties = d
        return device_sign_in

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
