from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.backtest_month import BacktestMonth


T = TypeVar("T", bound="Backtest")


@_attrs_define
class Backtest:
    """
    Attributes:
        method (str): The method measured (`<name>-v<version>`).
        months (list[BacktestMonth]):
        remote (bool): Whether remote AI was asked too.
    """

    method: str
    months: list[BacktestMonth]
    remote: bool
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        method = self.method

        months = []
        for months_item_data in self.months:
            months_item = months_item_data.to_dict()
            months.append(months_item)

        remote = self.remote

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "method": method,
                "months": months,
                "remote": remote,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.backtest_month import BacktestMonth  # noqa: PLC0415

        d = dict(src_dict)
        method = d.pop("method")

        months = []
        _months = d.pop("months")
        for months_item_data in _months:
            months_item = BacktestMonth.from_dict(months_item_data)

            months.append(months_item)

        remote = d.pop("remote")

        backtest = cls(
            method=method,
            months=months,
            remote=remote,
        )

        backtest.additional_properties = d
        return backtest

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
