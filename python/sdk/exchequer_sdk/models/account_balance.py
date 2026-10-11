from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

T = TypeVar("T", bound="AccountBalance")


@_attrs_define
class AccountBalance:
    """
    Attributes:
        account_id (UUID):
        balance (str): At the commodity's precision.
        commodity_id (UUID):
    """

    account_id: UUID
    balance: str
    commodity_id: UUID
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        account_id = str(self.account_id)

        balance = self.balance

        commodity_id = str(self.commodity_id)

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "account_id": account_id,
                "balance": balance,
                "commodity_id": commodity_id,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        account_id = UUID(d.pop("account_id"))

        balance = d.pop("balance")

        commodity_id = UUID(d.pop("commodity_id"))

        account_balance = cls(
            account_id=account_id,
            balance=balance,
            commodity_id=commodity_id,
        )

        account_balance.additional_properties = d
        return account_balance

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
