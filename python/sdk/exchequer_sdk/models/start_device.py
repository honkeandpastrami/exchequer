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
    """

    client: str

    def to_dict(self) -> dict[str, Any]:
        client = self.client

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "client": client,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        client = d.pop("client")

        start_device = cls(
            client=client,
        )

        return start_device
