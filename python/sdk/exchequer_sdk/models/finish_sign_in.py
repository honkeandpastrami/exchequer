from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

T = TypeVar("T", bound="FinishSignIn")


@_attrs_define
class FinishSignIn:
    """
    Attributes:
        credential (Any): The browser's `PublicKeyCredential`, as JSON (binary fields base64url).
    """

    credential: Any

    def to_dict(self) -> dict[str, Any]:
        credential = self.credential

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "credential": credential,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        credential = d.pop("credential")

        finish_sign_in = cls(
            credential=credential,
        )

        return finish_sign_in
