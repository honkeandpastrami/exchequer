from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.account_balance import AccountBalance


T = TypeVar("T", bound="Balances")


@_attrs_define
class Balances:
    """
    Attributes:
        balances (list[AccountBalance]): Every account and commodity whose balance on the date isn't zero.
        date (datetime.date):
        value_in_id (None | Unset | UUID): The commodity values are in, when asked.
    """

    balances: list[AccountBalance]
    date: datetime.date
    value_in_id: None | Unset | UUID = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        balances = []
        for balances_item_data in self.balances:
            balances_item = balances_item_data.to_dict()
            balances.append(balances_item)

        date = self.date.isoformat()

        value_in_id: None | str | Unset
        if isinstance(self.value_in_id, Unset):
            value_in_id = UNSET
        elif isinstance(self.value_in_id, UUID):
            value_in_id = str(self.value_in_id)
        else:
            value_in_id = self.value_in_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "balances": balances,
                "date": date,
            }
        )
        if value_in_id is not UNSET:
            field_dict["value_in_id"] = value_in_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.account_balance import AccountBalance  # noqa: PLC0415

        d = dict(src_dict)
        balances = []
        _balances = d.pop("balances")
        for balances_item_data in _balances:
            balances_item = AccountBalance.from_dict(balances_item_data)

            balances.append(balances_item)

        date = datetime.date.fromisoformat(d.pop("date"))

        def _parse_value_in_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                value_in_id_type_0 = UUID(data)

                return value_in_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        value_in_id = _parse_value_in_id(d.pop("value_in_id", UNSET))

        balances = cls(
            balances=balances,
            date=date,
            value_in_id=value_in_id,
        )

        balances.additional_properties = d
        return balances

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
