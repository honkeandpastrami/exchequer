from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.candidate_reason import CandidateReason

if TYPE_CHECKING:
    from ..models.candidate_split import CandidateSplit


T = TypeVar("T", bound="Candidate")


@_attrs_define
class Candidate:
    """A possible explanation for the difference, most likely first.

    Attributes:
        reason (CandidateReason):
        splits (list[CandidateSplit]):
    """

    reason: CandidateReason
    splits: list[CandidateSplit]
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        reason = self.reason.value

        splits = []
        for splits_item_data in self.splits:
            splits_item = splits_item_data.to_dict()
            splits.append(splits_item)

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "reason": reason,
                "splits": splits,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.candidate_split import CandidateSplit  # noqa: PLC0415

        d = dict(src_dict)
        reason = CandidateReason(d.pop("reason"))

        splits = []
        _splits = d.pop("splits")
        for splits_item_data in _splits:
            splits_item = CandidateSplit.from_dict(splits_item_data)

            splits.append(splits_item)

        candidate = cls(
            reason=reason,
            splits=splits,
        )

        candidate.additional_properties = d
        return candidate

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
