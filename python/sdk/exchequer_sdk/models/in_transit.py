from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.split_pointer import SplitPointer


T = TypeVar("T", bound="InTransit")


@_attrs_define
class InTransit:
    """
    Attributes:
        position (int):
        transaction_id (UUID):
        amount (str):
        date (datetime.date):
        cleared_by (None | SplitPointer | Unset):
    """

    position: int
    transaction_id: UUID
    amount: str
    date: datetime.date
    cleared_by: None | SplitPointer | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.split_pointer import SplitPointer  # noqa: PLC0415

        position = self.position

        transaction_id = str(self.transaction_id)

        amount = self.amount

        date = self.date.isoformat()

        cleared_by: dict[str, Any] | None | Unset
        if isinstance(self.cleared_by, Unset):
            cleared_by = UNSET
        elif isinstance(self.cleared_by, SplitPointer):
            cleared_by = self.cleared_by.to_dict()
        else:
            cleared_by = self.cleared_by

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "position": position,
                "transaction_id": transaction_id,
                "amount": amount,
                "date": date,
            }
        )
        if cleared_by is not UNSET:
            field_dict["cleared_by"] = cleared_by

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.split_pointer import SplitPointer  # noqa: PLC0415

        d = dict(src_dict)
        position = d.pop("position")

        transaction_id = UUID(d.pop("transaction_id"))

        amount = d.pop("amount")

        date = datetime.date.fromisoformat(d.pop("date"))

        def _parse_cleared_by(data: object) -> None | SplitPointer | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                cleared_by_type_0 = SplitPointer.from_dict(data)

                return cleared_by_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | SplitPointer | Unset, data)

        cleared_by = _parse_cleared_by(d.pop("cleared_by", UNSET))

        in_transit = cls(
            position=position,
            transaction_id=transaction_id,
            amount=amount,
            date=date,
            cleared_by=cleared_by,
        )

        in_transit.additional_properties = d
        return in_transit

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
