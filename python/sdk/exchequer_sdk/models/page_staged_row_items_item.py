from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.decision import Decision
from ..models.label import Label
from ..models.row_status import RowStatus
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.decision_suggestion import DecisionSuggestion
    from ..models.label_suggestion import LabelSuggestion
    from ..models.page_staged_row_items_item_raw import PageStagedRowItemsItemRaw
    from ..models.transfer_match import TransferMatch


T = TypeVar("T", bound="PageStagedRowItemsItem")


@_attrs_define
class PageStagedRowItemsItem:
    """
    Attributes:
        amount (str): Signed from the account's side: negative is money leaving it. Example: -80.00.
        chosen (list[Label]): The labels you set (a `null` one is an explicit none).
        date (datetime.date):
        decided (bool): Whether `decision` is the client's choice rather than the default.
        description (str):
        missing (list[Label]): For a row that will be a new transaction: the labels that are
            neither yours nor confidently suggested. Commit refuses while any row
            has some.
        not_applicable (list[Label]): Labels that don't apply: a transfer's category and purpose (design
            §3.2).
        position (int):
        raw (PageStagedRowItemsItemRaw): The row's fields as the file had them.
        statement (int):
        status (RowStatus): What a row would become.
        suggestions (list[LabelSuggestion]): At most one per label, confident or not.
        category_id (None | Unset | UUID):
        counterparty_id (None | Unset | UUID): The labels below are what a new transaction gets: your choice, else a
            confident suggestion. For a new transaction: who's on the outside
            side (unknown if absent and there's no `transfer_account_id`).
        decision (Decision | None | Unset):
        match_transaction_id (None | Unset | UUID): The existing transaction a duplicate or transfer duplicate is of.
        native_id (None | str | Unset): The bank's own ID for the row, if it has one.
        purpose_id (None | Unset | UUID): Who the money was for; absent, the account's owner.
        set_description (None | str | Unset): The new transaction's description, if not the bank's.
        suggested_decision (DecisionSuggestion | None | Unset):
        transfer_account_id (None | Unset | UUID): The other side when it's one of your accounts: the row is a transfer,
            whose category and purpose don't apply (unless it's a liquidation).
            A transfer match's is its other side's account.
        transfer_match (None | TransferMatch | Unset):
    """

    amount: str
    chosen: list[Label]
    date: datetime.date
    decided: bool
    description: str
    missing: list[Label]
    not_applicable: list[Label]
    position: int
    raw: PageStagedRowItemsItemRaw
    statement: int
    status: RowStatus
    suggestions: list[LabelSuggestion]
    category_id: None | Unset | UUID = UNSET
    counterparty_id: None | Unset | UUID = UNSET
    decision: Decision | None | Unset = UNSET
    match_transaction_id: None | Unset | UUID = UNSET
    native_id: None | str | Unset = UNSET
    purpose_id: None | Unset | UUID = UNSET
    set_description: None | str | Unset = UNSET
    suggested_decision: DecisionSuggestion | None | Unset = UNSET
    transfer_account_id: None | Unset | UUID = UNSET
    transfer_match: None | TransferMatch | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.decision_suggestion import DecisionSuggestion  # noqa: PLC0415
        from ..models.transfer_match import TransferMatch  # noqa: PLC0415

        amount = self.amount

        chosen = []
        for chosen_item_data in self.chosen:
            chosen_item = chosen_item_data.value
            chosen.append(chosen_item)

        date = self.date.isoformat()

        decided = self.decided

        description = self.description

        missing = []
        for missing_item_data in self.missing:
            missing_item = missing_item_data.value
            missing.append(missing_item)

        not_applicable = []
        for not_applicable_item_data in self.not_applicable:
            not_applicable_item = not_applicable_item_data.value
            not_applicable.append(not_applicable_item)

        position = self.position

        raw = self.raw.to_dict()

        statement = self.statement

        status = self.status.value

        suggestions = []
        for suggestions_item_data in self.suggestions:
            suggestions_item = suggestions_item_data.to_dict()
            suggestions.append(suggestions_item)

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

        suggested_decision: dict[str, Any] | None | Unset
        if isinstance(self.suggested_decision, Unset):
            suggested_decision = UNSET
        elif isinstance(self.suggested_decision, DecisionSuggestion):
            suggested_decision = self.suggested_decision.to_dict()
        else:
            suggested_decision = self.suggested_decision

        transfer_account_id: None | str | Unset
        if isinstance(self.transfer_account_id, Unset):
            transfer_account_id = UNSET
        elif isinstance(self.transfer_account_id, UUID):
            transfer_account_id = str(self.transfer_account_id)
        else:
            transfer_account_id = self.transfer_account_id

        transfer_match: dict[str, Any] | None | Unset
        if isinstance(self.transfer_match, Unset):
            transfer_match = UNSET
        elif isinstance(self.transfer_match, TransferMatch):
            transfer_match = self.transfer_match.to_dict()
        else:
            transfer_match = self.transfer_match

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "amount": amount,
                "chosen": chosen,
                "date": date,
                "decided": decided,
                "description": description,
                "missing": missing,
                "not_applicable": not_applicable,
                "position": position,
                "raw": raw,
                "statement": statement,
                "status": status,
                "suggestions": suggestions,
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
        if suggested_decision is not UNSET:
            field_dict["suggested_decision"] = suggested_decision
        if transfer_account_id is not UNSET:
            field_dict["transfer_account_id"] = transfer_account_id
        if transfer_match is not UNSET:
            field_dict["transfer_match"] = transfer_match

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.decision_suggestion import DecisionSuggestion  # noqa: PLC0415
        from ..models.label_suggestion import LabelSuggestion  # noqa: PLC0415
        from ..models.page_staged_row_items_item_raw import PageStagedRowItemsItemRaw  # noqa: PLC0415
        from ..models.transfer_match import TransferMatch  # noqa: PLC0415

        d = dict(src_dict)
        amount = d.pop("amount")

        chosen = []
        _chosen = d.pop("chosen")
        for chosen_item_data in _chosen:
            chosen_item = Label(chosen_item_data)

            chosen.append(chosen_item)

        date = datetime.date.fromisoformat(d.pop("date"))

        decided = d.pop("decided")

        description = d.pop("description")

        missing = []
        _missing = d.pop("missing")
        for missing_item_data in _missing:
            missing_item = Label(missing_item_data)

            missing.append(missing_item)

        not_applicable = []
        _not_applicable = d.pop("not_applicable")
        for not_applicable_item_data in _not_applicable:
            not_applicable_item = Label(not_applicable_item_data)

            not_applicable.append(not_applicable_item)

        position = d.pop("position")

        raw = PageStagedRowItemsItemRaw.from_dict(d.pop("raw"))

        statement = d.pop("statement")

        status = RowStatus(d.pop("status"))

        suggestions = []
        _suggestions = d.pop("suggestions")
        for suggestions_item_data in _suggestions:
            suggestions_item = LabelSuggestion.from_dict(suggestions_item_data)

            suggestions.append(suggestions_item)

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

        def _parse_suggested_decision(data: object) -> DecisionSuggestion | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                suggested_decision_type_0 = DecisionSuggestion.from_dict(data)

                return suggested_decision_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(DecisionSuggestion | None | Unset, data)

        suggested_decision = _parse_suggested_decision(d.pop("suggested_decision", UNSET))

        def _parse_transfer_account_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                transfer_account_id_type_0 = UUID(data)

                return transfer_account_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        transfer_account_id = _parse_transfer_account_id(d.pop("transfer_account_id", UNSET))

        def _parse_transfer_match(data: object) -> None | TransferMatch | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                transfer_match_type_0 = TransferMatch.from_dict(data)

                return transfer_match_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | TransferMatch | Unset, data)

        transfer_match = _parse_transfer_match(d.pop("transfer_match", UNSET))

        page_staged_row_items_item = cls(
            amount=amount,
            chosen=chosen,
            date=date,
            decided=decided,
            description=description,
            missing=missing,
            not_applicable=not_applicable,
            position=position,
            raw=raw,
            statement=statement,
            status=status,
            suggestions=suggestions,
            category_id=category_id,
            counterparty_id=counterparty_id,
            decision=decision,
            match_transaction_id=match_transaction_id,
            native_id=native_id,
            purpose_id=purpose_id,
            set_description=set_description,
            suggested_decision=suggested_decision,
            transfer_account_id=transfer_account_id,
            transfer_match=transfer_match,
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
