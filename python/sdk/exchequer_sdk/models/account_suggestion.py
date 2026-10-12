from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.suggester import Suggester


T = TypeVar("T", bound="AccountSuggestion")


@_attrs_define
class AccountSuggestion:
    """A statement's suggested account.

    Attributes:
        account_id (UUID):
        confidence (int): Per mille: 900 is 0.9.
        confident (bool):
        reason (str):
        suggester (Suggester): The exact system that made a suggestion.
    """

    account_id: UUID
    confidence: int
    confident: bool
    reason: str
    suggester: Suggester
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        account_id = str(self.account_id)

        confidence = self.confidence

        confident = self.confident

        reason = self.reason

        suggester = self.suggester.to_dict()

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "account_id": account_id,
                "confidence": confidence,
                "confident": confident,
                "reason": reason,
                "suggester": suggester,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.suggester import Suggester  # noqa: PLC0415

        d = dict(src_dict)
        account_id = UUID(d.pop("account_id"))

        confidence = d.pop("confidence")

        confident = d.pop("confident")

        reason = d.pop("reason")

        suggester = Suggester.from_dict(d.pop("suggester"))

        account_suggestion = cls(
            account_id=account_id,
            confidence=confidence,
            confident=confident,
            reason=reason,
            suggester=suggester,
        )

        account_suggestion.additional_properties = d
        return account_suggestion

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
