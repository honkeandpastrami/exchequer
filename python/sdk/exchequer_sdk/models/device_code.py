from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="DeviceCode")


@_attrs_define
class DeviceCode:
    """Show `user_code` to the user; poll with `device_code` until approved.

    Attributes:
        device_code (str): The CLI's secret. Shown only in this response.
        expires_at (datetime.datetime):
        interval (int): Seconds between polls.
        user_code (str): What the user approves in the web app, at `#/device/<user_code>`.
    """

    device_code: str
    expires_at: datetime.datetime
    interval: int
    user_code: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        device_code = self.device_code

        expires_at = self.expires_at.isoformat()

        interval = self.interval

        user_code = self.user_code

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "device_code": device_code,
                "expires_at": expires_at,
                "interval": interval,
                "user_code": user_code,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        device_code = d.pop("device_code")

        expires_at = datetime.datetime.fromisoformat(d.pop("expires_at"))

        interval = d.pop("interval")

        user_code = d.pop("user_code")

        device_code = cls(
            device_code=device_code,
            expires_at=expires_at,
            interval=interval,
            user_code=user_code,
        )

        device_code.additional_properties = d
        return device_code

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
