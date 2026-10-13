from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.balance_check import BalanceCheck


T = TypeVar("T", bound="Committed")


@_attrs_define
class Committed:
    """What a commit did.

    Attributes:
        balance_checks (list[BalanceCheck]): One per statement with a ledger balance: the bank's balance against
            the ledger's.
        created (int): New transactions.
        matched (int): Rows recorded as already in the ledger (duplicates, near-duplicates,
            transfer duplicates).
        replaced (int): Existing transactions replaced by their rows.
        skipped (int):
        transfers (int): Of them, transfers made of a transfer match: two rows, one
            transaction.
        import_batch_id (None | Unset | UUID): The batch the new transactions are in; absent if there were none.
    """

    balance_checks: list[BalanceCheck]
    created: int
    matched: int
    replaced: int
    skipped: int
    transfers: int
    import_batch_id: None | Unset | UUID = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        balance_checks = []
        for balance_checks_item_data in self.balance_checks:
            balance_checks_item = balance_checks_item_data.to_dict()
            balance_checks.append(balance_checks_item)

        created = self.created

        matched = self.matched

        replaced = self.replaced

        skipped = self.skipped

        transfers = self.transfers

        import_batch_id: None | str | Unset
        if isinstance(self.import_batch_id, Unset):
            import_batch_id = UNSET
        elif isinstance(self.import_batch_id, UUID):
            import_batch_id = str(self.import_batch_id)
        else:
            import_batch_id = self.import_batch_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "balance_checks": balance_checks,
                "created": created,
                "matched": matched,
                "replaced": replaced,
                "skipped": skipped,
                "transfers": transfers,
            }
        )
        if import_batch_id is not UNSET:
            field_dict["import_batch_id"] = import_batch_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.balance_check import BalanceCheck  # noqa: PLC0415

        d = dict(src_dict)
        balance_checks = []
        _balance_checks = d.pop("balance_checks")
        for balance_checks_item_data in _balance_checks:
            balance_checks_item = BalanceCheck.from_dict(balance_checks_item_data)

            balance_checks.append(balance_checks_item)

        created = d.pop("created")

        matched = d.pop("matched")

        replaced = d.pop("replaced")

        skipped = d.pop("skipped")

        transfers = d.pop("transfers")

        def _parse_import_batch_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                import_batch_id_type_0 = UUID(data)

                return import_batch_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        import_batch_id = _parse_import_batch_id(d.pop("import_batch_id", UNSET))

        committed = cls(
            balance_checks=balance_checks,
            created=created,
            matched=matched,
            replaced=replaced,
            skipped=skipped,
            transfers=transfers,
            import_batch_id=import_batch_id,
        )

        committed.additional_properties = d
        return committed

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
