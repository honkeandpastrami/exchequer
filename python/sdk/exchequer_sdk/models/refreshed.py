from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="Refreshed")


@_attrs_define
class Refreshed:
    """A refresh: running, or how it went.

    Attributes:
        errors (list[str]): `source: what went wrong`, for each failure; the next source was
            asked instead.
        running (bool): Still fetching: `GET /prices/refresh?wait=…` waits for it.
        sources (list[str]): The configured sources, in the order asked.
        started_at (datetime.datetime):
        stored (int): Prices stored or replaced.
        finished_at (datetime.datetime | None | Unset):
    """

    errors: list[str]
    running: bool
    sources: list[str]
    started_at: datetime.datetime
    stored: int
    finished_at: datetime.datetime | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        errors = self.errors

        running = self.running

        sources = self.sources

        started_at = self.started_at.isoformat()

        stored = self.stored

        finished_at: None | str | Unset
        if isinstance(self.finished_at, Unset):
            finished_at = UNSET
        elif isinstance(self.finished_at, datetime.datetime):
            finished_at = self.finished_at.isoformat()
        else:
            finished_at = self.finished_at

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "errors": errors,
                "running": running,
                "sources": sources,
                "started_at": started_at,
                "stored": stored,
            }
        )
        if finished_at is not UNSET:
            field_dict["finished_at"] = finished_at

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        errors = cast(list[str], d.pop("errors"))

        running = d.pop("running")

        sources = cast(list[str], d.pop("sources"))

        started_at = datetime.datetime.fromisoformat(d.pop("started_at"))

        stored = d.pop("stored")

        def _parse_finished_at(data: object) -> datetime.datetime | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                finished_at_type_0 = datetime.datetime.fromisoformat(data)

                return finished_at_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(datetime.datetime | None | Unset, data)

        finished_at = _parse_finished_at(d.pop("finished_at", UNSET))

        refreshed = cls(
            errors=errors,
            running=running,
            sources=sources,
            started_at=started_at,
            stored=stored,
            finished_at=finished_at,
        )

        refreshed.additional_properties = d
        return refreshed

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
