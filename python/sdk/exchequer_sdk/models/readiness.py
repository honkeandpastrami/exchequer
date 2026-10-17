from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.readiness_state import ReadinessState

T = TypeVar("T", bound="Readiness")


@_attrs_define
class Readiness:
    """An import's readiness, and why.

    Attributes:
        no_account (int): Statements with no account. Needs work.
        state (ReadinessState): How close an import is to committing (design §6.7): `ready` commits as
            shown, with nothing left to a guess (a rule's suggestion is yours);
            `suggested` would commit too, on confident AI suggestions you haven't
            confirmed; `needs_work` needs you first.
        suggested_accounts (int): Statements whose account is a confident suggestion, not yet chosen.
        suggested_rows (int): Rows that would take a confident AI suggestion.
        undecided (int): Rows whose decision is neither yours nor confidently suggested (a
            near duplicate never is). Needs work.
        unlabeled (int): New rows with a label neither chosen nor confidently suggested.
            Needs work.
    """

    no_account: int
    state: ReadinessState
    suggested_accounts: int
    suggested_rows: int
    undecided: int
    unlabeled: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        no_account = self.no_account

        state = self.state.value

        suggested_accounts = self.suggested_accounts

        suggested_rows = self.suggested_rows

        undecided = self.undecided

        unlabeled = self.unlabeled

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "no_account": no_account,
                "state": state,
                "suggested_accounts": suggested_accounts,
                "suggested_rows": suggested_rows,
                "undecided": undecided,
                "unlabeled": unlabeled,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        no_account = d.pop("no_account")

        state = ReadinessState(d.pop("state"))

        suggested_accounts = d.pop("suggested_accounts")

        suggested_rows = d.pop("suggested_rows")

        undecided = d.pop("undecided")

        unlabeled = d.pop("unlabeled")

        readiness = cls(
            no_account=no_account,
            state=state,
            suggested_accounts=suggested_accounts,
            suggested_rows=suggested_rows,
            undecided=undecided,
            unlabeled=unlabeled,
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
