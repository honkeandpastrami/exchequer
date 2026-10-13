from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.label import Label
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.suggester import Suggester


T = TypeVar("T", bound="LabelSuggestion")


@_attrs_define
class LabelSuggestion:
    """A suggested value for one of a row's labels.

    Attributes:
        confidence (int): Per mille: 900 is 0.9.
        confident (bool): Whether it's confident enough (at least 900) to be committed unless
            you choose otherwise.
        label (Label): A label a row's new transaction gets.
        reason (str):
        suggester (Suggester): The exact system that made a suggestion.
        transfer_account_id (None | Unset | UUID): For the counterparty: one of your accounts, suggested as the other
            side (a transfer).
        value (None | Unset | UUID): The suggested counterparty, category or party; `null` suggests none,
            unless `transfer_account_id` is set.
    """

    confidence: int
    confident: bool
    label: Label
    reason: str
    suggester: Suggester
    transfer_account_id: None | Unset | UUID = UNSET
    value: None | Unset | UUID = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        confidence = self.confidence

        confident = self.confident

        label = self.label.value

        reason = self.reason

        suggester = self.suggester.to_dict()

        transfer_account_id: None | str | Unset
        if isinstance(self.transfer_account_id, Unset):
            transfer_account_id = UNSET
        elif isinstance(self.transfer_account_id, UUID):
            transfer_account_id = str(self.transfer_account_id)
        else:
            transfer_account_id = self.transfer_account_id

        value: None | str | Unset
        if isinstance(self.value, Unset):
            value = UNSET
        elif isinstance(self.value, UUID):
            value = str(self.value)
        else:
            value = self.value

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "confidence": confidence,
                "confident": confident,
                "label": label,
                "reason": reason,
                "suggester": suggester,
            }
        )
        if transfer_account_id is not UNSET:
            field_dict["transfer_account_id"] = transfer_account_id
        if value is not UNSET:
            field_dict["value"] = value

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.suggester import Suggester  # noqa: PLC0415

        d = dict(src_dict)
        confidence = d.pop("confidence")

        confident = d.pop("confident")

        label = Label(d.pop("label"))

        reason = d.pop("reason")

        suggester = Suggester.from_dict(d.pop("suggester"))

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

        def _parse_value(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                value_type_0 = UUID(data)

                return value_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        value = _parse_value(d.pop("value", UNSET))

        label_suggestion = cls(
            confidence=confidence,
            confident=confident,
            label=label,
            reason=reason,
            suggester=suggester,
            transfer_account_id=transfer_account_id,
            value=value,
        )

        label_suggestion.additional_properties = d
        return label_suggestion

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
