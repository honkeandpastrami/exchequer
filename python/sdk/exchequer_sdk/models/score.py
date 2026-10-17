from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.bin_ import Bin
    from ..models.metrics import Metrics


T = TypeVar("T", bound="Score")


@_attrs_define
class Score:
    """Guesses against what the ledger says.

    Attributes:
        all_ (Metrics): The usual measures, as fractions (null without a denominator). A row
            not guessed counts against recall and accuracy, not precision; each row
            has one true value, so accuracy is the micro recall. Macro means are
            over the classes (values, or accounts) either true or guessed, a class
            never guessed or never true counting 0.
        asked (int): Rows (or statements) asked about.
        bins (list[Bin]):
        confident (int): Guesses at 900 or more, which commit would take.
        confident_only (Metrics): The usual measures, as fractions (null without a denominator). A row
            not guessed counts against recall and accuracy, not precision; each row
            has one true value, so accuracy is the micro recall. Macro means are
            over the classes (values, or accounts) either true or guessed, a class
            never guessed or never true counting 0.
        confident_right (int):
        guessed (int): Of them, those with a guess.
        right (int): Of them, those guessed right.
    """

    all_: Metrics
    asked: int
    bins: list[Bin]
    confident: int
    confident_only: Metrics
    confident_right: int
    guessed: int
    right: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        all_ = self.all_.to_dict()

        asked = self.asked

        bins = []
        for bins_item_data in self.bins:
            bins_item = bins_item_data.to_dict()
            bins.append(bins_item)

        confident = self.confident

        confident_only = self.confident_only.to_dict()

        confident_right = self.confident_right

        guessed = self.guessed

        right = self.right

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "all": all_,
                "asked": asked,
                "bins": bins,
                "confident": confident,
                "confident_only": confident_only,
                "confident_right": confident_right,
                "guessed": guessed,
                "right": right,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.bin_ import Bin  # noqa: PLC0415
        from ..models.metrics import Metrics  # noqa: PLC0415

        d = dict(src_dict)
        all_ = Metrics.from_dict(d.pop("all"))

        asked = d.pop("asked")

        bins = []
        _bins = d.pop("bins")
        for bins_item_data in _bins:
            bins_item = Bin.from_dict(bins_item_data)

            bins.append(bins_item)

        confident = d.pop("confident")

        confident_only = Metrics.from_dict(d.pop("confident_only"))

        confident_right = d.pop("confident_right")

        guessed = d.pop("guessed")

        right = d.pop("right")

        score = cls(
            all_=all_,
            asked=asked,
            bins=bins,
            confident=confident,
            confident_only=confident_only,
            confident_right=confident_right,
            guessed=guessed,
            right=right,
        )

        score.additional_properties = d
        return score

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
