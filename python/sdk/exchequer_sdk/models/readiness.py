from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.review_state import ReviewState

if TYPE_CHECKING:
    from ..models.state_counts import StateCounts


T = TypeVar("T", bound="Readiness")


@_attrs_define
class Readiness:
    """How close an import is to committing (design §6.7).

    Attributes:
        accounts_to_review (int): Statements whose account is a suggestion short of certain, not
            ticked as reviewed.
        adds (int): Transactions a commit would add now: a transfer pair added as one
            transfer is one.
        no_account (int): Statements with no account.
        rows (StateCounts): How many rows are in each state.
        state (ReviewState): What a row, a statement or an import needs (design §6.7), shown as its
            colour.
    """

    accounts_to_review: int
    adds: int
    no_account: int
    rows: StateCounts
    state: ReviewState
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        accounts_to_review = self.accounts_to_review

        adds = self.adds

        no_account = self.no_account

        rows = self.rows.to_dict()

        state = self.state.value

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "accounts_to_review": accounts_to_review,
                "adds": adds,
                "no_account": no_account,
                "rows": rows,
                "state": state,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.state_counts import StateCounts  # noqa: PLC0415

        d = dict(src_dict)
        accounts_to_review = d.pop("accounts_to_review")

        adds = d.pop("adds")

        no_account = d.pop("no_account")

        rows = StateCounts.from_dict(d.pop("rows"))

        state = ReviewState(d.pop("state"))

        readiness = cls(
            accounts_to_review=accounts_to_review,
            adds=adds,
            no_account=no_account,
            rows=rows,
            state=state,
        )

        readiness.additional_properties = d
        return readiness

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
