from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.backtest_level import BacktestLevel
    from ..models.label_score import LabelScore
    from ..models.score import Score


T = TypeVar("T", bound="BacktestMonth")


@_attrs_define
class BacktestMonth:
    """One month: the levels built from everything before it, then asked about
    its transactions.

        Attributes:
            accounts (Score): Guesses against what the ledger says.
            labels (list[LabelScore]): The month's transactions, each asked its labels with its account
                known: the final suggestions, whichever level made them.
            levels (list[BacktestLevel]): Per level, in order.
            month (str):
            trained_on (int): Labeled transactions they learned from.
    """

    accounts: Score
    labels: list[LabelScore]
    levels: list[BacktestLevel]
    month: str
    trained_on: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        accounts = self.accounts.to_dict()

        labels = []
        for labels_item_data in self.labels:
            labels_item = labels_item_data.to_dict()
            labels.append(labels_item)

        levels = []
        for levels_item_data in self.levels:
            levels_item = levels_item_data.to_dict()
            levels.append(levels_item)

        month = self.month

        trained_on = self.trained_on

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "accounts": accounts,
                "labels": labels,
                "levels": levels,
                "month": month,
                "trained_on": trained_on,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.backtest_level import BacktestLevel  # noqa: PLC0415
        from ..models.label_score import LabelScore  # noqa: PLC0415
        from ..models.score import Score  # noqa: PLC0415

        d = dict(src_dict)
        accounts = Score.from_dict(d.pop("accounts"))

        labels = []
        _labels = d.pop("labels")
        for labels_item_data in _labels:
            labels_item = LabelScore.from_dict(labels_item_data)

            labels.append(labels_item)

        levels = []
        _levels = d.pop("levels")
        for levels_item_data in _levels:
            levels_item = BacktestLevel.from_dict(levels_item_data)

            levels.append(levels_item)

        month = d.pop("month")

        trained_on = d.pop("trained_on")

        backtest_month = cls(
            accounts=accounts,
            labels=labels,
            levels=levels,
            month=month,
            trained_on=trained_on,
        )

        backtest_month.additional_properties = d
        return backtest_month

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
