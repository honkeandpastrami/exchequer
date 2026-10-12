from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.statement_kind import StatementKind
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.ledger_balance import LedgerBalance
    from ..models.row_counts import RowCounts
    from ..models.skipped_line import SkippedLine


T = TypeVar("T", bound="StagedStatement")


@_attrs_define
class StagedStatement:
    """One account's part of the file.

    Attributes:
        commodity_id (UUID):
        counts (RowCounts): How many rows have each status.
        position (int):
        skipped (list[SkippedLine]):
        account_id (None | Unset | UUID): `null` until the account is known: from the number in the file, or
            set by the client.
        kind (None | StatementKind | Unset):
        ledger (LedgerBalance | None | Unset):
        number_last4 (None | str | Unset): The last four of the account number in the file (CSVs have none). Example:
            6789.
    """

    commodity_id: UUID
    counts: RowCounts
    position: int
    skipped: list[SkippedLine]
    account_id: None | Unset | UUID = UNSET
    kind: None | StatementKind | Unset = UNSET
    ledger: LedgerBalance | None | Unset = UNSET
    number_last4: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.ledger_balance import LedgerBalance  # noqa: PLC0415

        commodity_id = str(self.commodity_id)

        counts = self.counts.to_dict()

        position = self.position

        skipped = []
        for skipped_item_data in self.skipped:
            skipped_item = skipped_item_data.to_dict()
            skipped.append(skipped_item)

        account_id: None | str | Unset
        if isinstance(self.account_id, Unset):
            account_id = UNSET
        elif isinstance(self.account_id, UUID):
            account_id = str(self.account_id)
        else:
            account_id = self.account_id

        kind: None | str | Unset
        if isinstance(self.kind, Unset):
            kind = UNSET
        elif isinstance(self.kind, StatementKind):
            kind = self.kind.value
        else:
            kind = self.kind

        ledger: dict[str, Any] | None | Unset
        if isinstance(self.ledger, Unset):
            ledger = UNSET
        elif isinstance(self.ledger, LedgerBalance):
            ledger = self.ledger.to_dict()
        else:
            ledger = self.ledger

        number_last4: None | str | Unset
        if isinstance(self.number_last4, Unset):
            number_last4 = UNSET
        else:
            number_last4 = self.number_last4

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "commodity_id": commodity_id,
                "counts": counts,
                "position": position,
                "skipped": skipped,
            }
        )
        if account_id is not UNSET:
            field_dict["account_id"] = account_id
        if kind is not UNSET:
            field_dict["kind"] = kind
        if ledger is not UNSET:
            field_dict["ledger"] = ledger
        if number_last4 is not UNSET:
            field_dict["number_last4"] = number_last4

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.ledger_balance import LedgerBalance  # noqa: PLC0415
        from ..models.row_counts import RowCounts  # noqa: PLC0415
        from ..models.skipped_line import SkippedLine  # noqa: PLC0415

        d = dict(src_dict)
        commodity_id = UUID(d.pop("commodity_id"))

        counts = RowCounts.from_dict(d.pop("counts"))

        position = d.pop("position")

        skipped = []
        _skipped = d.pop("skipped")
        for skipped_item_data in _skipped:
            skipped_item = SkippedLine.from_dict(skipped_item_data)

            skipped.append(skipped_item)

        def _parse_account_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                account_id_type_0 = UUID(data)

                return account_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        account_id = _parse_account_id(d.pop("account_id", UNSET))

        def _parse_kind(data: object) -> None | StatementKind | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                kind_type_0 = StatementKind(data)

                return kind_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | StatementKind | Unset, data)

        kind = _parse_kind(d.pop("kind", UNSET))

        def _parse_ledger(data: object) -> LedgerBalance | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                ledger_type_0 = LedgerBalance.from_dict(data)

                return ledger_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(LedgerBalance | None | Unset, data)

        ledger = _parse_ledger(d.pop("ledger", UNSET))

        def _parse_number_last4(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        number_last4 = _parse_number_last4(d.pop("number_last4", UNSET))

        staged_statement = cls(
            commodity_id=commodity_id,
            counts=counts,
            position=position,
            skipped=skipped,
            account_id=account_id,
            kind=kind,
            ledger=ledger,
            number_last4=number_last4,
        )

        staged_statement.additional_properties = d
        return staged_statement

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
