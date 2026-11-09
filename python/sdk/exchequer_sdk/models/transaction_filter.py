from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.kind_name import KindName
from ..models.line_kind import LineKind
from ..types import UNSET, Unset

T = TypeVar("T", bound="TransactionFilter")


@_attrs_define
class TransactionFilter:
    """Which transactions to list, and what of each counts. Filters combine with
    AND; repeat a key for alternatives (`kind=income&kind=expense`). `none`
    takes splits with no value there (`category_id=none`: uncategorized).

    The amounts that count are the postings that pass, as `/query` sums them;
    with `settles`, the settlement lines of one party's statement instead.

        Attributes:
            account_id (list[UUID] | Unset): On one of these accounts.
            category_id (list[str] | Unset):
            commodity_id (list[UUID] | Unset):
            counterparty_id (list[str] | Unset): The outside party of an income or expense.
            from_ (datetime.date | None | Unset): Earliest date, inclusive.
            kind (list[KindName] | Unset):
            line (LineKind | None | Unset):
            owner_id (list[UUID] | Unset): On an account one of these parties owns.
            party_id (None | Unset | UUID): With `settles`: the party whose statement it is (the child of its
                edge).
            purpose_id (list[str] | Unset): For one of these parties: the effective purpose, as `/query` has it.
            q (None | str | Unset): Text in the description or notes, or a split's description
                (case-insensitive).
            settles (None | str | Unset): With `party_id`: the lines on that party's settlement statement for
                this month (`YYYY-MM`). A payment can settle a month other than its
                date's.
            tag_id (list[UUID] | Unset): Transactions with one of these tags.
            to (datetime.date | None | Unset): Latest date, inclusive.
            unpriced (bool | Unset): Splits the settlement couldn't value: no leg in USD and no price.
    """

    account_id: list[UUID] | Unset = UNSET
    category_id: list[str] | Unset = UNSET
    commodity_id: list[UUID] | Unset = UNSET
    counterparty_id: list[str] | Unset = UNSET
    from_: datetime.date | None | Unset = UNSET
    kind: list[KindName] | Unset = UNSET
    line: LineKind | None | Unset = UNSET
    owner_id: list[UUID] | Unset = UNSET
    party_id: None | Unset | UUID = UNSET
    purpose_id: list[str] | Unset = UNSET
    q: None | str | Unset = UNSET
    settles: None | str | Unset = UNSET
    tag_id: list[UUID] | Unset = UNSET
    to: datetime.date | None | Unset = UNSET
    unpriced: bool | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        account_id: list[str] | Unset = UNSET
        if not isinstance(self.account_id, Unset):
            account_id = []
            for account_id_item_data in self.account_id:
                account_id_item = str(account_id_item_data)
                account_id.append(account_id_item)

        category_id: list[str] | Unset = UNSET
        if not isinstance(self.category_id, Unset):
            category_id = self.category_id

        commodity_id: list[str] | Unset = UNSET
        if not isinstance(self.commodity_id, Unset):
            commodity_id = []
            for commodity_id_item_data in self.commodity_id:
                commodity_id_item = str(commodity_id_item_data)
                commodity_id.append(commodity_id_item)

        counterparty_id: list[str] | Unset = UNSET
        if not isinstance(self.counterparty_id, Unset):
            counterparty_id = self.counterparty_id

        from_: None | str | Unset
        if isinstance(self.from_, Unset):
            from_ = UNSET
        elif isinstance(self.from_, datetime.date):
            from_ = self.from_.isoformat()
        else:
            from_ = self.from_

        kind: list[str] | Unset = UNSET
        if not isinstance(self.kind, Unset):
            kind = []
            for kind_item_data in self.kind:
                kind_item = kind_item_data.value
                kind.append(kind_item)

        line: None | str | Unset
        if isinstance(self.line, Unset):
            line = UNSET
        elif isinstance(self.line, LineKind):
            line = self.line.value
        else:
            line = self.line

        owner_id: list[str] | Unset = UNSET
        if not isinstance(self.owner_id, Unset):
            owner_id = []
            for owner_id_item_data in self.owner_id:
                owner_id_item = str(owner_id_item_data)
                owner_id.append(owner_id_item)

        party_id: None | str | Unset
        if isinstance(self.party_id, Unset):
            party_id = UNSET
        elif isinstance(self.party_id, UUID):
            party_id = str(self.party_id)
        else:
            party_id = self.party_id

        purpose_id: list[str] | Unset = UNSET
        if not isinstance(self.purpose_id, Unset):
            purpose_id = self.purpose_id

        q: None | str | Unset
        if isinstance(self.q, Unset):
            q = UNSET
        else:
            q = self.q

        settles: None | str | Unset
        if isinstance(self.settles, Unset):
            settles = UNSET
        else:
            settles = self.settles

        tag_id: list[str] | Unset = UNSET
        if not isinstance(self.tag_id, Unset):
            tag_id = []
            for tag_id_item_data in self.tag_id:
                tag_id_item = str(tag_id_item_data)
                tag_id.append(tag_id_item)

        to: None | str | Unset
        if isinstance(self.to, Unset):
            to = UNSET
        elif isinstance(self.to, datetime.date):
            to = self.to.isoformat()
        else:
            to = self.to

        unpriced = self.unpriced

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({})
        if account_id is not UNSET:
            field_dict["account_id"] = account_id
        if category_id is not UNSET:
            field_dict["category_id"] = category_id
        if commodity_id is not UNSET:
            field_dict["commodity_id"] = commodity_id
        if counterparty_id is not UNSET:
            field_dict["counterparty_id"] = counterparty_id
        if from_ is not UNSET:
            field_dict["from"] = from_
        if kind is not UNSET:
            field_dict["kind"] = kind
        if line is not UNSET:
            field_dict["line"] = line
        if owner_id is not UNSET:
            field_dict["owner_id"] = owner_id
        if party_id is not UNSET:
            field_dict["party_id"] = party_id
        if purpose_id is not UNSET:
            field_dict["purpose_id"] = purpose_id
        if q is not UNSET:
            field_dict["q"] = q
        if settles is not UNSET:
            field_dict["settles"] = settles
        if tag_id is not UNSET:
            field_dict["tag_id"] = tag_id
        if to is not UNSET:
            field_dict["to"] = to
        if unpriced is not UNSET:
            field_dict["unpriced"] = unpriced

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        _account_id = d.pop("account_id", UNSET)
        account_id: list[UUID] | Unset = UNSET
        if _account_id is not UNSET:
            account_id = []
            for account_id_item_data in _account_id:
                account_id_item = UUID(account_id_item_data)

                account_id.append(account_id_item)

        category_id = cast(list[str], d.pop("category_id", UNSET))

        _commodity_id = d.pop("commodity_id", UNSET)
        commodity_id: list[UUID] | Unset = UNSET
        if _commodity_id is not UNSET:
            commodity_id = []
            for commodity_id_item_data in _commodity_id:
                commodity_id_item = UUID(commodity_id_item_data)

                commodity_id.append(commodity_id_item)

        counterparty_id = cast(list[str], d.pop("counterparty_id", UNSET))

        def _parse_from_(data: object) -> datetime.date | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                from_type_0 = datetime.date.fromisoformat(data)

                return from_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(datetime.date | None | Unset, data)

        from_ = _parse_from_(d.pop("from", UNSET))

        _kind = d.pop("kind", UNSET)
        kind: list[KindName] | Unset = UNSET
        if _kind is not UNSET:
            kind = []
            for kind_item_data in _kind:
                kind_item = KindName(kind_item_data)

                kind.append(kind_item)

        def _parse_line(data: object) -> LineKind | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                line_type_0 = LineKind(data)

                return line_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(LineKind | None | Unset, data)

        line = _parse_line(d.pop("line", UNSET))

        _owner_id = d.pop("owner_id", UNSET)
        owner_id: list[UUID] | Unset = UNSET
        if _owner_id is not UNSET:
            owner_id = []
            for owner_id_item_data in _owner_id:
                owner_id_item = UUID(owner_id_item_data)

                owner_id.append(owner_id_item)

        def _parse_party_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                party_id_type_0 = UUID(data)

                return party_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        party_id = _parse_party_id(d.pop("party_id", UNSET))

        purpose_id = cast(list[str], d.pop("purpose_id", UNSET))

        def _parse_q(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        q = _parse_q(d.pop("q", UNSET))

        def _parse_settles(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        settles = _parse_settles(d.pop("settles", UNSET))

        _tag_id = d.pop("tag_id", UNSET)
        tag_id: list[UUID] | Unset = UNSET
        if _tag_id is not UNSET:
            tag_id = []
            for tag_id_item_data in _tag_id:
                tag_id_item = UUID(tag_id_item_data)

                tag_id.append(tag_id_item)

        def _parse_to(data: object) -> datetime.date | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                to_type_0 = datetime.date.fromisoformat(data)

                return to_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(datetime.date | None | Unset, data)

        to = _parse_to(d.pop("to", UNSET))

        unpriced = d.pop("unpriced", UNSET)

        transaction_filter = cls(
            account_id=account_id,
            category_id=category_id,
            commodity_id=commodity_id,
            counterparty_id=counterparty_id,
            from_=from_,
            kind=kind,
            line=line,
            owner_id=owner_id,
            party_id=party_id,
            purpose_id=purpose_id,
            q=q,
            settles=settles,
            tag_id=tag_id,
            to=to,
            unpriced=unpriced,
        )

        transaction_filter.additional_properties = d
        return transaction_filter

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
