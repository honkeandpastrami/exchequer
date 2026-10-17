from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.decision import Decision

if TYPE_CHECKING:
    from ..models.suggester import Suggester


T = TypeVar("T", bound="DecisionSuggestion")


@_attrs_define
class DecisionSuggestion:
    """What matching suggests committing does with a row.

    Attributes:
        confidence (int): Per mille: 900 is 0.9.
        confident (bool): Whether it's confident enough (at least 900) to be committed unless
            you choose otherwise. A near duplicate never is.
        decision (Decision): What committing does with a row.
        reason (str):
        suggester (Suggester): The exact system that made a suggestion.
    """

    confidence: int
    confident: bool
    decision: Decision
    reason: str
    suggester: Suggester
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        confidence = self.confidence

        confident = self.confident

        decision = self.decision.value

        reason = self.reason

        suggester = self.suggester.to_dict()

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "confidence": confidence,
                "confident": confident,
                "decision": decision,
                "reason": reason,
                "suggester": suggester,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.suggester import Suggester  # noqa: PLC0415

        d = dict(src_dict)
        confidence = d.pop("confidence")

        confident = d.pop("confident")

        decision = Decision(d.pop("decision"))

        reason = d.pop("reason")

        suggester = Suggester.from_dict(d.pop("suggester"))

        decision_suggestion = cls(
            confidence=confidence,
            confident=confident,
            decision=decision,
            reason=reason,
            suggester=suggester,
        )

        decision_suggestion.additional_properties = d
        return decision_suggestion

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
