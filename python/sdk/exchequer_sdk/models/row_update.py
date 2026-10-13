from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define

from ..models.decision import Decision
from ..models.label import Label
from ..types import UNSET, Unset

T = TypeVar("T", bound="RowUpdate")


@_attrs_define
class RowUpdate:
    """Fields to change; omitted ones stay as they are, `null` clears one.

    Attributes:
        category_id (None | Unset | UUID):
        counterparty_id (None | Unset | UUID):
        decision (Decision | None | Unset):
        purpose_id (None | Unset | UUID):
        revert (list[Label] | Unset): Labels to stop choosing, so their suggestions apply again. Setting a
            label (to a value or `null`) makes it your choice.
        set_description (None | str | Unset):
        transfer_account_id (None | Unset | UUID): One of your accounts as the other side: the row is a transfer. It
            sets the counterparty label (as yours), so not with
            `counterparty_id`; `null` is the same as `counterparty_id: null`.
    """

    category_id: None | Unset | UUID = UNSET
    counterparty_id: None | Unset | UUID = UNSET
    decision: Decision | None | Unset = UNSET
    purpose_id: None | Unset | UUID = UNSET
    revert: list[Label] | Unset = UNSET
    set_description: None | str | Unset = UNSET
    transfer_account_id: None | Unset | UUID = UNSET

    def to_dict(self) -> dict[str, Any]:
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

        purpose_id: None | str | Unset
        if isinstance(self.purpose_id, Unset):
            purpose_id = UNSET
        elif isinstance(self.purpose_id, UUID):
            purpose_id = str(self.purpose_id)
        else:
            purpose_id = self.purpose_id

        revert: list[str] | Unset = UNSET
        if not isinstance(self.revert, Unset):
            revert = []
            for revert_item_data in self.revert:
                revert_item = revert_item_data.value
                revert.append(revert_item)

        set_description: None | str | Unset
        if isinstance(self.set_description, Unset):
            set_description = UNSET
        else:
            set_description = self.set_description

        transfer_account_id: None | str | Unset
        if isinstance(self.transfer_account_id, Unset):
            transfer_account_id = UNSET
        elif isinstance(self.transfer_account_id, UUID):
            transfer_account_id = str(self.transfer_account_id)
        else:
            transfer_account_id = self.transfer_account_id

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if category_id is not UNSET:
            field_dict["category_id"] = category_id
        if counterparty_id is not UNSET:
            field_dict["counterparty_id"] = counterparty_id
        if decision is not UNSET:
            field_dict["decision"] = decision
        if purpose_id is not UNSET:
            field_dict["purpose_id"] = purpose_id
        if revert is not UNSET:
            field_dict["revert"] = revert
        if set_description is not UNSET:
            field_dict["set_description"] = set_description
        if transfer_account_id is not UNSET:
            field_dict["transfer_account_id"] = transfer_account_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)

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

        _revert = d.pop("revert", UNSET)
        revert: list[Label] | Unset = UNSET
        if _revert is not UNSET:
            revert = []
            for revert_item_data in _revert:
                revert_item = Label(revert_item_data)

                revert.append(revert_item)

        def _parse_set_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        set_description = _parse_set_description(d.pop("set_description", UNSET))

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

        row_update = cls(
            category_id=category_id,
            counterparty_id=counterparty_id,
            decision=decision,
            purpose_id=purpose_id,
            revert=revert,
            set_description=set_description,
            transfer_account_id=transfer_account_id,
        )

        return row_update
