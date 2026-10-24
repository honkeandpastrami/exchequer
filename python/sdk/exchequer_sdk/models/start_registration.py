from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

T = TypeVar("T", bound="StartRegistration")


@_attrs_define
class StartRegistration:
    """
    Attributes:
        invite (str): The invite's code.
    """

    invite: str

    def to_dict(self) -> dict[str, Any]:
        invite = self.invite

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "invite": invite,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        invite = d.pop("invite")

        start_registration = cls(
            invite=invite,
        )

        return start_registration
