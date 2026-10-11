from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.in_transit import InTransit
    from ..models.line import Line


T = TypeVar("T", bound="Edge")


@_attrs_define
class Edge:
    """One edge of the settlement tree (a child and its parent) for one month.
    Positive amounts go from the child to the parent.

        Attributes:
            child_id (UUID):
            claims (str): Regular claims across the edge this month.
            due (str): `claims − stipend`.
            implied_stipend (str): `claims − paid`: the stipend that would explain the payments made.
            outstanding (str): Everything up to the end of this month, in-transit proceeds included.
            paid (str): Regular payments that settle this month.
            parent_id (UUID):
            stipend (str): What the parent owes the child as stipends this month.
            to_settle (str): `outstanding` without the proceeds still in transit: the regular
                transfer to make (positive: the child pays).
            in_transit (list[InTransit] | None | Unset):
            lines (list[Line] | None | Unset):
    """

    child_id: UUID
    claims: str
    due: str
    implied_stipend: str
    outstanding: str
    paid: str
    parent_id: UUID
    stipend: str
    to_settle: str
    in_transit: list[InTransit] | None | Unset = UNSET
    lines: list[Line] | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        child_id = str(self.child_id)

        claims = self.claims

        due = self.due

        implied_stipend = self.implied_stipend

        outstanding = self.outstanding

        paid = self.paid

        parent_id = str(self.parent_id)

        stipend = self.stipend

        to_settle = self.to_settle

        in_transit: list[dict[str, Any]] | None | Unset
        if isinstance(self.in_transit, Unset):
            in_transit = UNSET
        elif isinstance(self.in_transit, list):
            in_transit = []
            for in_transit_type_0_item_data in self.in_transit:
                in_transit_type_0_item = in_transit_type_0_item_data.to_dict()
                in_transit.append(in_transit_type_0_item)

        else:
            in_transit = self.in_transit

        lines: list[dict[str, Any]] | None | Unset
        if isinstance(self.lines, Unset):
            lines = UNSET
        elif isinstance(self.lines, list):
            lines = []
            for lines_type_0_item_data in self.lines:
                lines_type_0_item = lines_type_0_item_data.to_dict()
                lines.append(lines_type_0_item)

        else:
            lines = self.lines

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "child_id": child_id,
                "claims": claims,
                "due": due,
                "implied_stipend": implied_stipend,
                "outstanding": outstanding,
                "paid": paid,
                "parent_id": parent_id,
                "stipend": stipend,
                "to_settle": to_settle,
            }
        )
        if in_transit is not UNSET:
            field_dict["in_transit"] = in_transit
        if lines is not UNSET:
            field_dict["lines"] = lines

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.in_transit import InTransit  # noqa: PLC0415
        from ..models.line import Line  # noqa: PLC0415

        d = dict(src_dict)
        child_id = UUID(d.pop("child_id"))

        claims = d.pop("claims")

        due = d.pop("due")

        implied_stipend = d.pop("implied_stipend")

        outstanding = d.pop("outstanding")

        paid = d.pop("paid")

        parent_id = UUID(d.pop("parent_id"))

        stipend = d.pop("stipend")

        to_settle = d.pop("to_settle")

        def _parse_in_transit(data: object) -> list[InTransit] | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                in_transit_type_0 = []
                _in_transit_type_0 = data
                for in_transit_type_0_item_data in _in_transit_type_0:
                    in_transit_type_0_item = InTransit.from_dict(in_transit_type_0_item_data)

                    in_transit_type_0.append(in_transit_type_0_item)

                return in_transit_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(list[InTransit] | None | Unset, data)

        in_transit = _parse_in_transit(d.pop("in_transit", UNSET))

        def _parse_lines(data: object) -> list[Line] | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, list):
                    raise TypeError()
                lines_type_0 = []
                _lines_type_0 = data
                for lines_type_0_item_data in _lines_type_0:
                    lines_type_0_item = Line.from_dict(lines_type_0_item_data)

                    lines_type_0.append(lines_type_0_item)

                return lines_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(list[Line] | None | Unset, data)

        lines = _parse_lines(d.pop("lines", UNSET))

        edge = cls(
            child_id=child_id,
            claims=claims,
            due=due,
            implied_stipend=implied_stipend,
            outstanding=outstanding,
            paid=paid,
            parent_id=parent_id,
            stipend=stipend,
            to_settle=to_settle,
            in_transit=in_transit,
            lines=lines,
        )

        edge.additional_properties = d
        return edge

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
