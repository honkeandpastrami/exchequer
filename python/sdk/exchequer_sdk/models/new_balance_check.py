from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="NewBalanceCheck")


@_attrs_define
class NewBalanceCheck:
    """
    Attributes:
        account_id (UUID):
        amount (str): As the bank shows it (owed amounts positive on cards and mortgages). Example: 1234.56.
        commodity_id (None | Unset | UUID): Default: the household currency.
        date (datetime.date | None | Unset): Default: today (UTC).
    """

    account_id: UUID
    amount: str
    commodity_id: None | Unset | UUID = UNSET
    date: datetime.date | None | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        account_id = str(self.account_id)

        amount = self.amount

        commodity_id: None | str | Unset
        if isinstance(self.commodity_id, Unset):
            commodity_id = UNSET
        elif isinstance(self.commodity_id, UUID):
            commodity_id = str(self.commodity_id)
        else:
            commodity_id = self.commodity_id

        date: None | str | Unset
        if isinstance(self.date, Unset):
            date = UNSET
        elif isinstance(self.date, datetime.date):
            date = self.date.isoformat()
        else:
            date = self.date

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "account_id": account_id,
                "amount": amount,
            }
        )
        if commodity_id is not UNSET:
            field_dict["commodity_id"] = commodity_id
        if date is not UNSET:
            field_dict["date"] = date

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        account_id = UUID(d.pop("account_id"))

        amount = d.pop("amount")

        def _parse_commodity_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                commodity_id_type_0 = UUID(data)

                return commodity_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        commodity_id = _parse_commodity_id(d.pop("commodity_id", UNSET))

        def _parse_date(data: object) -> datetime.date | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                date_type_0 = datetime.date.fromisoformat(data)

                return date_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(datetime.date | None | Unset, data)

        date = _parse_date(d.pop("date", UNSET))

        new_balance_check = cls(
            account_id=account_id,
            amount=amount,
            commodity_id=commodity_id,
            date=date,
        )

        return new_balance_check
