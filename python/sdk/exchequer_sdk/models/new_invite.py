from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="NewInvite")


@_attrs_define
class NewInvite:
    """
    Attributes:
        name (str): Who it's for, e.g. `Alex`: added if there's no such user yet.
        replace (bool | Unset): Using it removes their other passkeys and revokes their tokens: for
            a lost device.
    """

    name: str
    replace: bool | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        name = self.name

        replace = self.replace

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "name": name,
            }
        )
        if replace is not UNSET:
            field_dict["replace"] = replace

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        name = d.pop("name")

        replace = d.pop("replace", UNSET)

        new_invite = cls(
            name=name,
            replace=replace,
        )

        return new_invite
