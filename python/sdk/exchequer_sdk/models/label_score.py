from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.label import Label

if TYPE_CHECKING:
    from ..models.bin_ import Bin


T = TypeVar("T", bound="LabelScore")


@_attrs_define
class LabelScore:
    """
    Attributes:
        asked (int): Rows (or statements) asked about.
        bins (list[Bin]):
        confident (int): Guesses at 900 or more, which commit would take.
        confident_right (int):
        guessed (int): Of them, those with a guess.
        right (int): Of them, those guessed right.
        label (Label): A label a row's new transaction gets.
    """

    asked: int
    bins: list[Bin]
    confident: int
    confident_right: int
    guessed: int
    right: int
    label: Label
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        asked = self.asked

        bins = []
        for bins_item_data in self.bins:
            bins_item = bins_item_data.to_dict()
            bins.append(bins_item)

        confident = self.confident

        confident_right = self.confident_right

        guessed = self.guessed

        right = self.right

        label = self.label.value

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "asked": asked,
                "bins": bins,
                "confident": confident,
                "confident_right": confident_right,
                "guessed": guessed,
                "right": right,
                "label": label,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.bin_ import Bin  # noqa: PLC0415

        d = dict(src_dict)
        asked = d.pop("asked")

        bins = []
        _bins = d.pop("bins")
        for bins_item_data in _bins:
            bins_item = Bin.from_dict(bins_item_data)

            bins.append(bins_item)

        confident = d.pop("confident")

        confident_right = d.pop("confident_right")

        guessed = d.pop("guessed")

        right = d.pop("right")

        label = Label(d.pop("label"))

        label_score = cls(
            asked=asked,
            bins=bins,
            confident=confident,
            confident_right=confident_right,
            guessed=guessed,
            right=right,
            label=label,
        )

        label_score.additional_properties = d
        return label_score

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
