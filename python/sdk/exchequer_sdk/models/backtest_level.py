from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.backtest_level_details import BacktestLevelDetails
    from ..models.label_score import LabelScore
    from ..models.score import Score


T = TypeVar("T", bound="BacktestLevel")


@_attrs_define
class BacktestLevel:
    """One level's share: of the rows (or statements), those whose final
    suggestion it made, and how right they were.

        Attributes:
            accounts (Score): Guesses against what the ledger says.
            details (BacktestLevelDetails): What it records as its details (e.g. its data's snapshot).
            kind (str): `local` or `remote`.
            labels (list[LabelScore]):
            name (str):
            version (str):
    """

    accounts: Score
    details: BacktestLevelDetails
    kind: str
    labels: list[LabelScore]
    name: str
    version: str
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        accounts = self.accounts.to_dict()

        details = self.details.to_dict()

        kind = self.kind

        labels = []
        for labels_item_data in self.labels:
            labels_item = labels_item_data.to_dict()
            labels.append(labels_item)

        name = self.name

        version = self.version

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "accounts": accounts,
                "details": details,
                "kind": kind,
                "labels": labels,
                "name": name,
                "version": version,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.backtest_level_details import BacktestLevelDetails  # noqa: PLC0415
        from ..models.label_score import LabelScore  # noqa: PLC0415
        from ..models.score import Score  # noqa: PLC0415

        d = dict(src_dict)
        accounts = Score.from_dict(d.pop("accounts"))

        details = BacktestLevelDetails.from_dict(d.pop("details"))

        kind = d.pop("kind")

        labels = []
        _labels = d.pop("labels")
        for labels_item_data in _labels:
            labels_item = LabelScore.from_dict(labels_item_data)

            labels.append(labels_item)

        name = d.pop("name")

        version = d.pop("version")

        backtest_level = cls(
            accounts=accounts,
            details=details,
            kind=kind,
            labels=labels,
            name=name,
            version=version,
        )

        backtest_level.additional_properties = d
        return backtest_level

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
