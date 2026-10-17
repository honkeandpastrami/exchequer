from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.committed import Committed


T = TypeVar("T", bound="ReadyOutcome")


@_attrs_define
class ReadyOutcome:
    """What committing one of the ready imports did.

    Attributes:
        id (UUID):
        committed (Committed | None | Unset):
        file_name (None | str | Unset):
        skipped (None | str | Unset): Why it wasn't committed: gone, no longer ready, changed since it was
            checked, or refused.
    """

    id: UUID
    committed: Committed | None | Unset = UNSET
    file_name: None | str | Unset = UNSET
    skipped: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.committed import Committed  # noqa: PLC0415

        id = str(self.id)

        committed: dict[str, Any] | None | Unset
        if isinstance(self.committed, Unset):
            committed = UNSET
        elif isinstance(self.committed, Committed):
            committed = self.committed.to_dict()
        else:
            committed = self.committed

        file_name: None | str | Unset
        if isinstance(self.file_name, Unset):
            file_name = UNSET
        else:
            file_name = self.file_name

        skipped: None | str | Unset
        if isinstance(self.skipped, Unset):
            skipped = UNSET
        else:
            skipped = self.skipped

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "id": id,
            }
        )
        if committed is not UNSET:
            field_dict["committed"] = committed
        if file_name is not UNSET:
            field_dict["file_name"] = file_name
        if skipped is not UNSET:
            field_dict["skipped"] = skipped

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.committed import Committed  # noqa: PLC0415

        d = dict(src_dict)
        id = UUID(d.pop("id"))

        def _parse_committed(data: object) -> Committed | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                committed_type_0 = Committed.from_dict(data)

                return committed_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(Committed | None | Unset, data)

        committed = _parse_committed(d.pop("committed", UNSET))

        def _parse_file_name(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        file_name = _parse_file_name(d.pop("file_name", UNSET))

        def _parse_skipped(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        skipped = _parse_skipped(d.pop("skipped", UNSET))

        ready_outcome = cls(
            id=id,
            committed=committed,
            file_name=file_name,
            skipped=skipped,
        )

        ready_outcome.additional_properties = d
        return ready_outcome

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
