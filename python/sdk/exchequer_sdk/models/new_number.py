from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

T = TypeVar("T", bound="NewNumber")


@_attrs_define
class NewNumber:
    """
    Attributes:
        number (str): The full number, as a bank file shows it: letters and digits, up to
            34; spaces and dashes are ignored. Never shown again in full.
    """

    number: str

    def to_dict(self) -> dict[str, Any]:
        number = self.number

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "number": number,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        number = d.pop("number")

        new_number = cls(
            number=number,
        )

        return new_number
