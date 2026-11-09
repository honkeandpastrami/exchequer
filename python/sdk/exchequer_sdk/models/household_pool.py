from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.category_amount import CategoryAmount
    from ..models.party_amount import PartyAmount
    from ..models.transaction_filter import TransactionFilter


T = TypeVar("T", bound="HouseholdPool")


@_attrs_define
class HouseholdPool:
    """The household's month (design §4.3), in USD. Income, expenses and
    savings count only amounts in USD; stipends and business
    funding come from the settlement, which values other commodities at
    prices.

        Attributes:
            business_funding (list[PartyAmount]): How much each business's net funding grew this month.
            currency_id (UUID):
            expenses (list[CategoryAmount]): Spending for the household, by category (negative). Money into an
                investment or a loan (a mortgage's principal) counts.
            fetching (bool): Missing or stale prices are being fetched: once `GET /prices/fetching`
                says they're done, the figures may be fresher.
            filter_ (TransactionFilter): Which transactions to list, and what of each counts. Filters combine with
                AND; repeat a key for alternatives (`kind=income&kind=expense`). `none`
                takes splits with no value there (`category_id=none`: uncategorized).

                The amounts that count are the postings that pass, as `/query` sums them;
                with `settles`, the settlement lines of one party's statement instead.
            income (list[CategoryAmount]): Income for the household, by category (positive). Money out of an
                investment into a liquid account counts.
            month (str):  Example: 2026-06.
            shared_savings_change (str): The change in the household's own liquid accounts over the month.
            stipends (list[PartyAmount]): What the household owes each person for the month.
            savings_filter (None | TransactionFilter | Unset):
    """

    business_funding: list[PartyAmount]
    currency_id: UUID
    expenses: list[CategoryAmount]
    fetching: bool
    filter_: TransactionFilter
    income: list[CategoryAmount]
    month: str
    shared_savings_change: str
    stipends: list[PartyAmount]
    savings_filter: None | TransactionFilter | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.transaction_filter import TransactionFilter  # noqa: PLC0415

        business_funding = []
        for business_funding_item_data in self.business_funding:
            business_funding_item = business_funding_item_data.to_dict()
            business_funding.append(business_funding_item)

        currency_id = str(self.currency_id)

        expenses = []
        for expenses_item_data in self.expenses:
            expenses_item = expenses_item_data.to_dict()
            expenses.append(expenses_item)

        fetching = self.fetching

        filter_ = self.filter_.to_dict()

        income = []
        for income_item_data in self.income:
            income_item = income_item_data.to_dict()
            income.append(income_item)

        month = self.month

        shared_savings_change = self.shared_savings_change

        stipends = []
        for stipends_item_data in self.stipends:
            stipends_item = stipends_item_data.to_dict()
            stipends.append(stipends_item)

        savings_filter: dict[str, Any] | None | Unset
        if isinstance(self.savings_filter, Unset):
            savings_filter = UNSET
        elif isinstance(self.savings_filter, TransactionFilter):
            savings_filter = self.savings_filter.to_dict()
        else:
            savings_filter = self.savings_filter

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "business_funding": business_funding,
                "currency_id": currency_id,
                "expenses": expenses,
                "fetching": fetching,
                "filter": filter_,
                "income": income,
                "month": month,
                "shared_savings_change": shared_savings_change,
                "stipends": stipends,
            }
        )
        if savings_filter is not UNSET:
            field_dict["savings_filter"] = savings_filter

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.category_amount import CategoryAmount  # noqa: PLC0415
        from ..models.party_amount import PartyAmount  # noqa: PLC0415
        from ..models.transaction_filter import TransactionFilter  # noqa: PLC0415

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

        fetching = d.pop("fetching")

        filter_ = TransactionFilter.from_dict(d.pop("filter"))

        income = []
        _income = d.pop("income")
        for income_item_data in _income:
            income_item = CategoryAmount.from_dict(income_item_data)

            income.append(income_item)

        month = d.pop("month")

        shared_savings_change = d.pop("shared_savings_change")

        stipends = []
        _stipends = d.pop("stipends")
        for stipends_item_data in _stipends:
            stipends_item = PartyAmount.from_dict(stipends_item_data)

            stipends.append(stipends_item)

        def _parse_savings_filter(data: object) -> None | TransactionFilter | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                savings_filter_type_0 = TransactionFilter.from_dict(data)

                return savings_filter_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | TransactionFilter | Unset, data)

        savings_filter = _parse_savings_filter(d.pop("savings_filter", UNSET))

        household_pool = cls(
            business_funding=business_funding,
            currency_id=currency_id,
            expenses=expenses,
            fetching=fetching,
            filter_=filter_,
            income=income,
            month=month,
            shared_savings_change=shared_savings_change,
            stipends=stipends,
            savings_filter=savings_filter,
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
