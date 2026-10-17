from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.line_kind import LineKind
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.valuation_out import ValuationOut


T = TypeVar("T", bound="Line")


@_attrs_define
class Line:
    """An item on an edge. Positive amounts go from the child to the parent.

    Attributes:
        position (int):
        transaction_id (UUID):
        amount (str):
        date (datetime.date):
        kind (LineKind):
        valuation (None | Unset | ValuationOut):
    """

    position: int
    transaction_id: UUID
    amount: str
    date: datetime.date
    kind: LineKind
    valuation: None | Unset | ValuationOut = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.valuation_out import ValuationOut  # noqa: PLC0415

        position = self.position

        transaction_id = str(self.transaction_id)

        amount = self.amount

        date = self.date.isoformat()

        kind = self.kind.value

        valuation: dict[str, Any] | None | Unset
        if isinstance(self.valuation, Unset):
            valuation = UNSET
        elif isinstance(self.valuation, ValuationOut):
            valuation = self.valuation.to_dict()
        else:
            valuation = self.valuation

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "position": position,
                "transaction_id": transaction_id,
                "amount": amount,
                "date": date,
                "kind": kind,
            }
        )
        if valuation is not UNSET:
            field_dict["valuation"] = valuation

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.valuation_out import ValuationOut  # noqa: PLC0415

        d = dict(src_dict)
        position = d.pop("position")

        transaction_id = UUID(d.pop("transaction_id"))

        amount = d.pop("amount")

        date = datetime.date.fromisoformat(d.pop("date"))

        kind = LineKind(d.pop("kind"))

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

        line = cls(
            position=position,
            transaction_id=transaction_id,
            amount=amount,
            date=date,
            kind=kind,
            valuation=valuation,
        )

        line.additional_properties = d
        return line

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
