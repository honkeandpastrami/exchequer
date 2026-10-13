from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="Bin")


@_attrs_define
class Bin:
    """Guesses by confidence: those from `from` up to the next bin's `from`
    (the last bin includes 1000).

        Attributes:
            from_ (int): Per mille.
            guesses (int):
            right (int):
    """

    from_: int
    guesses: int
    right: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from_ = self.from_

        guesses = self.guesses

        right = self.right

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "from": from_,
                "guesses": guesses,
                "right": right,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        from_ = d.pop("from")

        guesses = d.pop("guesses")

        right = d.pop("right")

        bin_ = cls(
            from_=from_,
            guesses=guesses,
            right=right,
        )

        bin_.additional_properties = d
        return bin_

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
