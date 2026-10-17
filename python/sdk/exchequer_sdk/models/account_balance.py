from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.valuation_out import ValuationOut


T = TypeVar("T", bound="AccountBalance")


@_attrs_define
class AccountBalance:
    """
    Attributes:
        account_id (UUID):
        balance (str): At the commodity's precision.
        commodity_id (UUID):
        valuation (None | Unset | ValuationOut):
        value (None | str | Unset): With `value_in`: the balance's value in it, at its precision.
            Absent when no price connects them.
    """

    account_id: UUID
    balance: str
    commodity_id: UUID
    valuation: None | Unset | ValuationOut = UNSET
    value: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.valuation_out import ValuationOut  # noqa: PLC0415

        account_id = str(self.account_id)

        balance = self.balance

        commodity_id = str(self.commodity_id)

        valuation: dict[str, Any] | None | Unset
        if isinstance(self.valuation, Unset):
            valuation = UNSET
        elif isinstance(self.valuation, ValuationOut):
            valuation = self.valuation.to_dict()
        else:
            valuation = self.valuation

        value: None | str | Unset
        if isinstance(self.value, Unset):
            value = UNSET
        else:
            value = self.value

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "account_id": account_id,
                "balance": balance,
                "commodity_id": commodity_id,
            }
        )
        if valuation is not UNSET:
            field_dict["valuation"] = valuation
        if value is not UNSET:
            field_dict["value"] = value

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.valuation_out import ValuationOut  # noqa: PLC0415

        d = dict(src_dict)
        account_id = UUID(d.pop("account_id"))

        balance = d.pop("balance")

        commodity_id = UUID(d.pop("commodity_id"))

        def _parse_valuation(data: object) -> None | Unset | ValuationOut:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                valuation_type_0 = ValuationOut.from_dict(data)

                return valuation_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | ValuationOut, data)

        valuation = _parse_valuation(d.pop("valuation", UNSET))

        def _parse_value(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        value = _parse_value(d.pop("value", UNSET))

        account_balance = cls(
            account_id=account_id,
            balance=balance,
            commodity_id=commodity_id,
            valuation=valuation,
            value=value,
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
