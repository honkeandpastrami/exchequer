from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="FinishRegistration")


@_attrs_define
class FinishRegistration:
    """
    Attributes:
        credential (Any): The browser's `PublicKeyCredential`, as JSON (binary fields base64url).
        name (None | str | Unset): What to call the passkey, e.g. `iPhone`.
    """

    credential: Any
    name: None | str | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        credential = self.credential

        name: None | str | Unset
        if isinstance(self.name, Unset):
            name = UNSET
        else:
            name = self.name

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "credential": credential,
            }
        )
        if name is not UNSET:
            field_dict["name"] = name

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        credential = d.pop("credential")

        def _parse_name(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        name = _parse_name(d.pop("name", UNSET))

        finish_registration = cls(
            credential=credential,
            name=name,
        )

        return finish_registration
