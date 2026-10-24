from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

T = TypeVar("T", bound="StartDevice")


@_attrs_define
class StartDevice:
    """
    Attributes:
        client (str): What the CLI calls itself, shown to the approver: `cli on laptop`.
        ttl_days (int): How many days its token works, 1-365; shown to the approver.
    """

    client: str
    ttl_days: int

    def to_dict(self) -> dict[str, Any]:
        client = self.client

        ttl_days = self.ttl_days

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "client": client,
                "ttl_days": ttl_days,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        client = d.pop("client")

        ttl_days = d.pop("ttl_days")

        start_device = cls(
            client=client,
            ttl_days=ttl_days,
        )

        return start_device
