from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.kind_name import KindName
from ..types import UNSET, Unset

T = TypeVar("T", bound="Row")


@_attrs_define
class Row:
    """One row. A grouped field is present (`null` for "none", e.g. no
    category); a field not grouped by is absent.

        Attributes:
            amount (str): At the commodity's precision. Positive: into your accounts. Example: -80.00.
            commodity_id (UUID):
            account_id (None | Unset | UUID):
            bucket (datetime.date | None | Unset):
            category_id (None | Unset | UUID):
            counterparty_id (None | Unset | UUID):
            kind (KindName | None | Unset):
            owner_id (None | Unset | UUID):
            purpose_id (None | Unset | UUID):
            tag (None | str | Unset):
    """

    amount: str
    commodity_id: UUID
    account_id: None | Unset | UUID = UNSET
    bucket: datetime.date | None | Unset = UNSET
    category_id: None | Unset | UUID = UNSET
    counterparty_id: None | Unset | UUID = UNSET
    kind: KindName | None | Unset = UNSET
    owner_id: None | Unset | UUID = UNSET
    purpose_id: None | Unset | UUID = UNSET
    tag: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        amount = self.amount

        commodity_id = str(self.commodity_id)

        account_id: None | str | Unset
        if isinstance(self.account_id, Unset):
            account_id = UNSET
        elif isinstance(self.account_id, UUID):
            account_id = str(self.account_id)
        else:
            account_id = self.account_id

        bucket: None | str | Unset
        if isinstance(self.bucket, Unset):
            bucket = UNSET
        elif isinstance(self.bucket, datetime.date):
            bucket = self.bucket.isoformat()
        else:
            bucket = self.bucket

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

        kind: None | str | Unset
        if isinstance(self.kind, Unset):
            kind = UNSET
        elif isinstance(self.kind, KindName):
            kind = self.kind.value
        else:
            kind = self.kind

        owner_id: None | str | Unset
        if isinstance(self.owner_id, Unset):
            owner_id = UNSET
        elif isinstance(self.owner_id, UUID):
            owner_id = str(self.owner_id)
        else:
            owner_id = self.owner_id

        purpose_id: None | str | Unset
        if isinstance(self.purpose_id, Unset):
            purpose_id = UNSET
        elif isinstance(self.purpose_id, UUID):
            purpose_id = str(self.purpose_id)
        else:
            purpose_id = self.purpose_id

        tag: None | str | Unset
        if isinstance(self.tag, Unset):
            tag = UNSET
        else:
            tag = self.tag

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "amount": amount,
                "commodity_id": commodity_id,
            }
        )
        if account_id is not UNSET:
            field_dict["account_id"] = account_id
        if bucket is not UNSET:
            field_dict["bucket"] = bucket
        if category_id is not UNSET:
            field_dict["category_id"] = category_id
        if counterparty_id is not UNSET:
            field_dict["counterparty_id"] = counterparty_id
        if kind is not UNSET:
            field_dict["kind"] = kind
        if owner_id is not UNSET:
            field_dict["owner_id"] = owner_id
        if purpose_id is not UNSET:
            field_dict["purpose_id"] = purpose_id
        if tag is not UNSET:
            field_dict["tag"] = tag

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        amount = d.pop("amount")

        commodity_id = UUID(d.pop("commodity_id"))

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

        def _parse_bucket(data: object) -> datetime.date | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                bucket_type_0 = datetime.date.fromisoformat(data)

                return bucket_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(datetime.date | None | Unset, data)

        bucket = _parse_bucket(d.pop("bucket", UNSET))

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

        def _parse_kind(data: object) -> KindName | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                kind_type_0 = KindName(data)

                return kind_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(KindName | None | Unset, data)

        kind = _parse_kind(d.pop("kind", UNSET))

        def _parse_owner_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                owner_id_type_0 = UUID(data)

                return owner_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        owner_id = _parse_owner_id(d.pop("owner_id", UNSET))

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

        def _parse_tag(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        tag = _parse_tag(d.pop("tag", UNSET))

        row = cls(
            amount=amount,
            commodity_id=commodity_id,
            account_id=account_id,
            bucket=bucket,
            category_id=category_id,
            counterparty_id=counterparty_id,
            kind=kind,
            owner_id=owner_id,
            purpose_id=purpose_id,
            tag=tag,
        )

        row.additional_properties = d
        return row

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
