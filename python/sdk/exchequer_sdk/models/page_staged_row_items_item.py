from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.decision import Decision
from ..models.row_status import RowStatus
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.page_staged_row_items_item_raw import PageStagedRowItemsItemRaw


T = TypeVar("T", bound="PageStagedRowItemsItem")


@_attrs_define
class PageStagedRowItemsItem:
    """
    Attributes:
        amount (str): Signed from the account's side: negative is money leaving it. Example: -80.00.
        date (datetime.date):
        decided (bool): Whether `decision` is the client's choice rather than the default.
        description (str):
        position (int):
        raw (PageStagedRowItemsItemRaw): The row's fields as the file had them.
        statement (int):
        status (RowStatus): What a row would become.
        category_id (None | Unset | UUID):
        counterparty_id (None | Unset | UUID): For a new transaction: who's on the outside side (unknown if absent).
        decision (Decision | None | Unset):
        match_transaction_id (None | Unset | UUID): The existing transaction a duplicate or match is of.
        native_id (None | str | Unset): The bank's own ID for the row, if it has one.
        purpose_id (None | Unset | UUID): Who the money was for; absent, the account's owner.
        set_description (None | str | Unset): The new transaction's description, if not the bank's.
    """

    amount: str
    date: datetime.date
    decided: bool
    description: str
    position: int
    raw: PageStagedRowItemsItemRaw
    statement: int
    status: RowStatus
    category_id: None | Unset | UUID = UNSET
    counterparty_id: None | Unset | UUID = UNSET
    decision: Decision | None | Unset = UNSET
    match_transaction_id: None | Unset | UUID = UNSET
    native_id: None | str | Unset = UNSET
    purpose_id: None | Unset | UUID = UNSET
    set_description: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        amount = self.amount

        date = self.date.isoformat()

        decided = self.decided

        description = self.description

        position = self.position

        raw = self.raw.to_dict()

        statement = self.statement

        status = self.status.value

        category_id: None | str | Unset
        if isinstance(self.category_id, Unset):
            category_id = UNSET
        elif isinstance(self.category_id, UUID):
            category_id = str(self.category_id)
        else:
            category_id = self.category_id

        counterparty_id: None | str | Unset
        if isinstance(self.counterparty_id, Unset):
            counterparty_id = UNSET
        elif isinstance(self.counterparty_id, UUID):
            counterparty_id = str(self.counterparty_id)
        else:
            counterparty_id = self.counterparty_id

        decision: None | str | Unset
        if isinstance(self.decision, Unset):
            decision = UNSET
        elif isinstance(self.decision, Decision):
            decision = self.decision.value
        else:
            decision = self.decision

        match_transaction_id: None | str | Unset
        if isinstance(self.match_transaction_id, Unset):
            match_transaction_id = UNSET
        elif isinstance(self.match_transaction_id, UUID):
            match_transaction_id = str(self.match_transaction_id)
        else:
            match_transaction_id = self.match_transaction_id

        native_id: None | str | Unset
        if isinstance(self.native_id, Unset):
            native_id = UNSET
        else:
            native_id = self.native_id

        purpose_id: None | str | Unset
        if isinstance(self.purpose_id, Unset):
            purpose_id = UNSET
        elif isinstance(self.purpose_id, UUID):
            purpose_id = str(self.purpose_id)
        else:
            purpose_id = self.purpose_id

        set_description: None | str | Unset
        if isinstance(self.set_description, Unset):
            set_description = UNSET
        else:
            set_description = self.set_description

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "amount": amount,
                "date": date,
                "decided": decided,
                "description": description,
                "position": position,
                "raw": raw,
                "statement": statement,
                "status": status,
            }
        )
        if category_id is not UNSET:
            field_dict["category_id"] = category_id
        if counterparty_id is not UNSET:
            field_dict["counterparty_id"] = counterparty_id
        if decision is not UNSET:
            field_dict["decision"] = decision
        if match_transaction_id is not UNSET:
            field_dict["match_transaction_id"] = match_transaction_id
        if native_id is not UNSET:
            field_dict["native_id"] = native_id
        if purpose_id is not UNSET:
            field_dict["purpose_id"] = purpose_id
        if set_description is not UNSET:
            field_dict["set_description"] = set_description

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.page_staged_row_items_item_raw import PageStagedRowItemsItemRaw  # noqa: PLC0415

        d = dict(src_dict)
        amount = d.pop("amount")

        date = datetime.date.fromisoformat(d.pop("date"))

        decided = d.pop("decided")

        description = d.pop("description")

        position = d.pop("position")

        raw = PageStagedRowItemsItemRaw.from_dict(d.pop("raw"))

        statement = d.pop("statement")

        status = RowStatus(d.pop("status"))

        def _parse_category_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                category_id_type_0 = UUID(data)

                return category_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        category_id = _parse_category_id(d.pop("category_id", UNSET))

        def _parse_counterparty_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                counterparty_id_type_0 = UUID(data)

                return counterparty_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        counterparty_id = _parse_counterparty_id(d.pop("counterparty_id", UNSET))

        def _parse_decision(data: object) -> Decision | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                decision_type_0 = Decision(data)

                return decision_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(Decision | None | Unset, data)

        decision = _parse_decision(d.pop("decision", UNSET))

        def _parse_match_transaction_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                match_transaction_id_type_0 = UUID(data)

                return match_transaction_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        match_transaction_id = _parse_match_transaction_id(d.pop("match_transaction_id", UNSET))

        def _parse_native_id(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        native_id = _parse_native_id(d.pop("native_id", UNSET))

        def _parse_purpose_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                purpose_id_type_0 = UUID(data)

                return purpose_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        purpose_id = _parse_purpose_id(d.pop("purpose_id", UNSET))

        def _parse_set_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        set_description = _parse_set_description(d.pop("set_description", UNSET))

        page_staged_row_items_item = cls(
            amount=amount,
            date=date,
            decided=decided,
            description=description,
            position=position,
            raw=raw,
            statement=statement,
            status=status,
            category_id=category_id,
            counterparty_id=counterparty_id,
            decision=decision,
            match_transaction_id=match_transaction_id,
            native_id=native_id,
            purpose_id=purpose_id,
            set_description=set_description,
        )

        page_staged_row_items_item.additional_properties = d
        return page_staged_row_items_item

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
