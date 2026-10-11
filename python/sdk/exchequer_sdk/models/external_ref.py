from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar

from attrs import define as _attrs_define

T = TypeVar("T", bound="ExternalRef")


@_attrs_define
class ExternalRef:
    """Where a transaction came from outside Exchequer, such as its ID in the
    system it was migrated from. Unique per `source`.

        Attributes:
            ref (str): 1-200 characters. Example: 1234.
            source (str): Lowercase letters, digits, `_` or `-`; e.g. `firefly`. Example: firefly.
    """

    ref: str
    source: str

    def to_dict(self) -> dict[str, Any]:
        ref = self.ref

        source = self.source

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "ref": ref,
                "source": source,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        ref = d.pop("ref")

        source = d.pop("source")

        external_ref = cls(
            ref=ref,
            source=source,
        )

        return external_ref
