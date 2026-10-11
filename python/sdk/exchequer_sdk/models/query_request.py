from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define

from ..models.bucket_name import BucketName
from ..models.group_by import GroupBy
from ..models.kind_name import KindName
from ..models.measure import Measure
from ..types import UNSET, Unset

T = TypeVar("T", bound="QueryRequest")


@_attrs_define
class QueryRequest:
    """Filters combine with AND; the values inside one filter with OR.

    Attributes:
        account_ids (list[UUID] | Unset):
        bucket (BucketName | None | Unset):
        category_ids (list[UUID] | Unset): Flow only.
        counterparty_ids (list[UUID] | Unset): Flow only.
        from_ (datetime.date | None | Unset): First day, inclusive. For a balance, where buckets start (default: `to`).
        group_by (list[GroupBy] | Unset): Rows are always per commodity too. A balance groups by `account` or
            `owner` only.
        kinds (list[KindName] | Unset): Flow only.
        measure (Measure | Unset):
        owner_ids (list[UUID] | Unset):
        purpose_ids (list[UUID] | Unset): Flow only. Effective purpose, as in `group_by: purpose`.
        tags (list[str] | Unset): Flow only: transactions with any of these tags.
        text (None | str | Unset): Flow only: case-insensitive text in the description or notes.
        to (datetime.date | None | Unset): Last day, inclusive. Required for a balance.
    """

    account_ids: list[UUID] | Unset = UNSET
    bucket: BucketName | None | Unset = UNSET
    category_ids: list[UUID] | Unset = UNSET
    counterparty_ids: list[UUID] | Unset = UNSET
    from_: datetime.date | None | Unset = UNSET
    group_by: list[GroupBy] | Unset = UNSET
    kinds: list[KindName] | Unset = UNSET
    measure: Measure | Unset = UNSET
    owner_ids: list[UUID] | Unset = UNSET
    purpose_ids: list[UUID] | Unset = UNSET
    tags: list[str] | Unset = UNSET
    text: None | str | Unset = UNSET
    to: datetime.date | None | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        account_ids: list[str] | Unset = UNSET
        if not isinstance(self.account_ids, Unset):
            account_ids = []
            for account_ids_item_data in self.account_ids:
                account_ids_item = str(account_ids_item_data)
                account_ids.append(account_ids_item)

        bucket: None | str | Unset
        if isinstance(self.bucket, Unset):
            bucket = UNSET
        elif isinstance(self.bucket, BucketName):
            bucket = self.bucket.value
        else:
            bucket = self.bucket

        category_ids: list[str] | Unset = UNSET
        if not isinstance(self.category_ids, Unset):
            category_ids = []
            for category_ids_item_data in self.category_ids:
                category_ids_item = str(category_ids_item_data)
                category_ids.append(category_ids_item)

        counterparty_ids: list[str] | Unset = UNSET
        if not isinstance(self.counterparty_ids, Unset):
            counterparty_ids = []
            for counterparty_ids_item_data in self.counterparty_ids:
                counterparty_ids_item = str(counterparty_ids_item_data)
                counterparty_ids.append(counterparty_ids_item)

        from_: None | str | Unset
        if isinstance(self.from_, Unset):
            from_ = UNSET
        elif isinstance(self.from_, datetime.date):
            from_ = self.from_.isoformat()
        else:
            from_ = self.from_

        group_by: list[str] | Unset = UNSET
        if not isinstance(self.group_by, Unset):
            group_by = []
            for group_by_item_data in self.group_by:
                group_by_item = group_by_item_data.value
                group_by.append(group_by_item)

        kinds: list[str] | Unset = UNSET
        if not isinstance(self.kinds, Unset):
            kinds = []
            for kinds_item_data in self.kinds:
                kinds_item = kinds_item_data.value
                kinds.append(kinds_item)

        measure: str | Unset = UNSET
        if not isinstance(self.measure, Unset):
            measure = self.measure.value

        owner_ids: list[str] | Unset = UNSET
        if not isinstance(self.owner_ids, Unset):
            owner_ids = []
            for owner_ids_item_data in self.owner_ids:
                owner_ids_item = str(owner_ids_item_data)
                owner_ids.append(owner_ids_item)

        purpose_ids: list[str] | Unset = UNSET
        if not isinstance(self.purpose_ids, Unset):
            purpose_ids = []
            for purpose_ids_item_data in self.purpose_ids:
                purpose_ids_item = str(purpose_ids_item_data)
                purpose_ids.append(purpose_ids_item)

        tags: list[str] | Unset = UNSET
        if not isinstance(self.tags, Unset):
            tags = self.tags

        text: None | str | Unset
        if isinstance(self.text, Unset):
            text = UNSET
        else:
            text = self.text

        to: None | str | Unset
        if isinstance(self.to, Unset):
            to = UNSET
        elif isinstance(self.to, datetime.date):
            to = self.to.isoformat()
        else:
            to = self.to

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if account_ids is not UNSET:
            field_dict["account_ids"] = account_ids
        if bucket is not UNSET:
            field_dict["bucket"] = bucket
        if category_ids is not UNSET:
            field_dict["category_ids"] = category_ids
        if counterparty_ids is not UNSET:
            field_dict["counterparty_ids"] = counterparty_ids
        if from_ is not UNSET:
            field_dict["from"] = from_
        if group_by is not UNSET:
            field_dict["group_by"] = group_by
        if kinds is not UNSET:
            field_dict["kinds"] = kinds
        if measure is not UNSET:
            field_dict["measure"] = measure
        if owner_ids is not UNSET:
            field_dict["owner_ids"] = owner_ids
        if purpose_ids is not UNSET:
            field_dict["purpose_ids"] = purpose_ids
        if tags is not UNSET:
            field_dict["tags"] = tags
        if text is not UNSET:
            field_dict["text"] = text
        if to is not UNSET:
            field_dict["to"] = to

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        _account_ids = d.pop("account_ids", UNSET)
        account_ids: list[UUID] | Unset = UNSET
        if _account_ids is not UNSET:
            account_ids = []
            for account_ids_item_data in _account_ids:
                account_ids_item = UUID(account_ids_item_data)

                account_ids.append(account_ids_item)

        def _parse_bucket(data: object) -> BucketName | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                bucket_type_0 = BucketName(data)

                return bucket_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(BucketName | None | Unset, data)

        bucket = _parse_bucket(d.pop("bucket", UNSET))

        _category_ids = d.pop("category_ids", UNSET)
        category_ids: list[UUID] | Unset = UNSET
        if _category_ids is not UNSET:
            category_ids = []
            for category_ids_item_data in _category_ids:
                category_ids_item = UUID(category_ids_item_data)

                category_ids.append(category_ids_item)

        _counterparty_ids = d.pop("counterparty_ids", UNSET)
        counterparty_ids: list[UUID] | Unset = UNSET
        if _counterparty_ids is not UNSET:
            counterparty_ids = []
            for counterparty_ids_item_data in _counterparty_ids:
                counterparty_ids_item = UUID(counterparty_ids_item_data)

                counterparty_ids.append(counterparty_ids_item)

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

        _group_by = d.pop("group_by", UNSET)
        group_by: list[GroupBy] | Unset = UNSET
        if _group_by is not UNSET:
            group_by = []
            for group_by_item_data in _group_by:
                group_by_item = GroupBy(group_by_item_data)

                group_by.append(group_by_item)

        _kinds = d.pop("kinds", UNSET)
        kinds: list[KindName] | Unset = UNSET
        if _kinds is not UNSET:
            kinds = []
            for kinds_item_data in _kinds:
                kinds_item = KindName(kinds_item_data)

                kinds.append(kinds_item)

        _measure = d.pop("measure", UNSET)
        measure: Measure | Unset
        if isinstance(_measure, Unset):
            measure = UNSET
        else:
            measure = Measure(_measure)

        _owner_ids = d.pop("owner_ids", UNSET)
        owner_ids: list[UUID] | Unset = UNSET
        if _owner_ids is not UNSET:
            owner_ids = []
            for owner_ids_item_data in _owner_ids:
                owner_ids_item = UUID(owner_ids_item_data)

                owner_ids.append(owner_ids_item)

        _purpose_ids = d.pop("purpose_ids", UNSET)
        purpose_ids: list[UUID] | Unset = UNSET
        if _purpose_ids is not UNSET:
            purpose_ids = []
            for purpose_ids_item_data in _purpose_ids:
                purpose_ids_item = UUID(purpose_ids_item_data)

                purpose_ids.append(purpose_ids_item)

        tags = cast(list[str], d.pop("tags", UNSET))

        def _parse_text(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        text = _parse_text(d.pop("text", UNSET))

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

        query_request = cls(
            account_ids=account_ids,
            bucket=bucket,
            category_ids=category_ids,
            counterparty_ids=counterparty_ids,
            from_=from_,
            group_by=group_by,
            kinds=kinds,
            measure=measure,
            owner_ids=owner_ids,
            purpose_ids=purpose_ids,
            tags=tags,
            text=text,
            to=to,
        )

        return query_request
