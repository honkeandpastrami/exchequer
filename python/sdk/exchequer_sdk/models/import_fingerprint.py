from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar
from uuid import UUID

from attrs import define as _attrs_define

T = TypeVar("T", bound="ImportFingerprint")


@_attrs_define
class ImportFingerprint:
    """
    Attributes:
        account_id (UUID):
        fingerprint (str): The row's ID: the bank's own (OFX `FITID`), or the hash of its date,
            description and amount (design §6.3). 1-200 characters.
        transaction_id (UUID): The transaction the row became; it must have a split on the account.
    """

    account_id: UUID
    fingerprint: str
    transaction_id: UUID

    def to_dict(self) -> dict[str, Any]:
        account_id = str(self.account_id)

        fingerprint = self.fingerprint

        transaction_id = str(self.transaction_id)

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "account_id": account_id,
                "fingerprint": fingerprint,
                "transaction_id": transaction_id,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        account_id = UUID(d.pop("account_id"))

        fingerprint = d.pop("fingerprint")

        transaction_id = UUID(d.pop("transaction_id"))

        import_fingerprint = cls(
            account_id=account_id,
            fingerprint=fingerprint,
            transaction_id=transaction_id,
        )

        return import_fingerprint
