from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

T = TypeVar("T", bound="NewToken")


@_attrs_define
class NewToken:
    """
    Attributes:
        name (str): What uses the token, e.g. `backup script`. Unique.
        ttl_days (int): How many days it works, 1-365. It can be revoked sooner.
    """

    name: str
    ttl_days: int

    def to_dict(self) -> dict[str, Any]:
        name = self.name

        ttl_days = self.ttl_days

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "name": name,
                "ttl_days": ttl_days,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        name = d.pop("name")

        ttl_days = d.pop("ttl_days")

        new_token = cls(
            name=name,
            ttl_days=ttl_days,
        )

        return new_token
