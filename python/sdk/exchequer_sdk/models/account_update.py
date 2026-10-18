from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define

from ..models.account_kind import AccountKind
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.credit_limit import CreditLimit


T = TypeVar("T", bound="AccountUpdate")


@_attrs_define
class AccountUpdate:
    """Fields to change; omitted ones stay as they are. The owner can't change.

    Attributes:
        archived (bool | None | Unset):
        credit_limit (CreditLimit | None | Unset):
        kind (AccountKind | None | Unset):
        liquid (bool | None | Unset): A mortgage is never liquid.
        name (None | str | Unset):
    """

    archived: bool | None | Unset = UNSET
    credit_limit: CreditLimit | None | Unset = UNSET
    kind: AccountKind | None | Unset = UNSET
    liquid: bool | None | Unset = UNSET
    name: None | str | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        from ..models.credit_limit import CreditLimit  # noqa: PLC0415

        archived: bool | None | Unset
        if isinstance(self.archived, Unset):
            archived = UNSET
        else:
            archived = self.archived

        credit_limit: dict[str, Any] | None | Unset
        if isinstance(self.credit_limit, Unset):
            credit_limit = UNSET
        elif isinstance(self.credit_limit, CreditLimit):
            credit_limit = self.credit_limit.to_dict()
        else:
            credit_limit = self.credit_limit

        kind: None | str | Unset
        if isinstance(self.kind, Unset):
            kind = UNSET
        elif isinstance(self.kind, AccountKind):
            kind = self.kind.value
        else:
            kind = self.kind

        liquid: bool | None | Unset
        if isinstance(self.liquid, Unset):
            liquid = UNSET
        else:
            liquid = self.liquid

        name: None | str | Unset
        if isinstance(self.name, Unset):
            name = UNSET
        else:
            name = self.name

        field_dict: dict[str, Any] = {}

        field_dict.update({})
        if archived is not UNSET:
            field_dict["archived"] = archived
        if credit_limit is not UNSET:
            field_dict["credit_limit"] = credit_limit
        if kind is not UNSET:
            field_dict["kind"] = kind
        if liquid is not UNSET:
            field_dict["liquid"] = liquid
        if name is not UNSET:
            field_dict["name"] = name

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.credit_limit import CreditLimit  # noqa: PLC0415

        d = dict(src_dict)

        def _parse_archived(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        archived = _parse_archived(d.pop("archived", UNSET))

        def _parse_credit_limit(data: object) -> CreditLimit | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                credit_limit_type_0 = CreditLimit.from_dict(data)

                return credit_limit_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(CreditLimit | None | Unset, data)

        credit_limit = _parse_credit_limit(d.pop("credit_limit", UNSET))

        def _parse_kind(data: object) -> AccountKind | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                kind_type_0 = AccountKind(data)

                return kind_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(AccountKind | None | Unset, data)

        kind = _parse_kind(d.pop("kind", UNSET))

        def _parse_liquid(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        liquid = _parse_liquid(d.pop("liquid", UNSET))

        def _parse_name(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        name = _parse_name(d.pop("name", UNSET))

        account_update = cls(
            archived=archived,
            credit_limit=credit_limit,
            kind=kind,
            liquid=liquid,
            name=name,
        )

        return account_update
