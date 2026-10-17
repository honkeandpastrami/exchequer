from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define

from ..types import UNSET, Unset

T = TypeVar("T", bound="RefreshRequest")


@_attrs_define
class RefreshRequest:
    """
    Attributes:
        from_ (datetime.date | None | Unset): Default: `to`.
        to (datetime.date | None | Unset): Default (and at most): today in the server's time zone.
        value_in (None | str | Unset): A commodity code. Default: `USD`.
    """

    from_: datetime.date | None | Unset = UNSET
    to: datetime.date | None | Unset = UNSET
    value_in: None | str | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        from_: None | str | Unset
        if isinstance(self.from_, Unset):
            from_ = UNSET
        elif isinstance(self.from_, datetime.date):
            from_ = self.from_.isoformat()
        else:
            from_ = self.from_

        to: None | str | Unset
        if isinstance(self.to, Unset):
            to = UNSET
        elif isinstance(self.to, datetime.date):
            to = self.to.isoformat()
        else:
            to = self.to

        value_in: None | str | Unset
        if isinstance(self.value_in, Unset):
            value_in = UNSET
        else:
            value_in = self.value_in

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if from_ is not UNSET:
            field_dict["from"] = from_
        if to is not UNSET:
            field_dict["to"] = to
        if value_in is not UNSET:
            field_dict["value_in"] = value_in

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)

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

        def _parse_value_in(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        value_in = _parse_value_in(d.pop("value_in", UNSET))

        refresh_request = cls(
            from_=from_,
            to=to,
            value_in=value_in,
        )

        return refresh_request
