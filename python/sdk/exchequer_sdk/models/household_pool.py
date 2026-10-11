from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.category_amount import CategoryAmount
    from ..models.party_amount import PartyAmount


T = TypeVar("T", bound="HouseholdPool")


@_attrs_define
class HouseholdPool:
    """The household's month (design §4.3), in the household currency. Amounts
    in other commodities aren't included until prices exist.

        Attributes:
            business_funding (list[PartyAmount]): How much each business's net funding grew this month.
            currency_id (UUID):
            expenses (list[CategoryAmount]): Spending for the household, by category (negative).
            income (list[CategoryAmount]): Income for the household, by category (positive).
            liquidations (str): Investment proceeds that became shared (positive).
            month (str):  Example: 2026-06.
            shared_savings_change (str): The change in the household's own liquid accounts over the month.
            stipends (list[PartyAmount]): What the household owes each person for the month.
    """

    business_funding: list[PartyAmount]
    currency_id: UUID
    expenses: list[CategoryAmount]
    income: list[CategoryAmount]
    liquidations: str
    month: str
    shared_savings_change: str
    stipends: list[PartyAmount]
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        business_funding = []
        for business_funding_item_data in self.business_funding:
            business_funding_item = business_funding_item_data.to_dict()
            business_funding.append(business_funding_item)

        currency_id = str(self.currency_id)

        expenses = []
        for expenses_item_data in self.expenses:
            expenses_item = expenses_item_data.to_dict()
            expenses.append(expenses_item)

        income = []
        for income_item_data in self.income:
            income_item = income_item_data.to_dict()
            income.append(income_item)

        liquidations = self.liquidations

        month = self.month

        shared_savings_change = self.shared_savings_change

        stipends = []
        for stipends_item_data in self.stipends:
            stipends_item = stipends_item_data.to_dict()
            stipends.append(stipends_item)

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "business_funding": business_funding,
                "currency_id": currency_id,
                "expenses": expenses,
                "income": income,
                "liquidations": liquidations,
                "month": month,
                "shared_savings_change": shared_savings_change,
                "stipends": stipends,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.category_amount import CategoryAmount  # noqa: PLC0415
        from ..models.party_amount import PartyAmount  # noqa: PLC0415

        d = dict(src_dict)
        business_funding = []
        _business_funding = d.pop("business_funding")
        for business_funding_item_data in _business_funding:
            business_funding_item = PartyAmount.from_dict(business_funding_item_data)

            business_funding.append(business_funding_item)

        currency_id = UUID(d.pop("currency_id"))

        expenses = []
        _expenses = d.pop("expenses")
        for expenses_item_data in _expenses:
            expenses_item = CategoryAmount.from_dict(expenses_item_data)

            expenses.append(expenses_item)

        income = []
        _income = d.pop("income")
        for income_item_data in _income:
            income_item = CategoryAmount.from_dict(income_item_data)

            income.append(income_item)

        liquidations = d.pop("liquidations")

        month = d.pop("month")

        shared_savings_change = d.pop("shared_savings_change")

        stipends = []
        _stipends = d.pop("stipends")
        for stipends_item_data in _stipends:
            stipends_item = PartyAmount.from_dict(stipends_item_data)

            stipends.append(stipends_item)

        household_pool = cls(
            business_funding=business_funding,
            currency_id=currency_id,
            expenses=expenses,
            income=income,
            liquidations=liquidations,
            month=month,
            shared_savings_change=shared_savings_change,
            stipends=stipends,
        )

        household_pool.additional_properties = d
        return household_pool

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
