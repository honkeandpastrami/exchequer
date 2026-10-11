from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define

from ..models.account_kind import AccountKind
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.credit_limit import CreditLimit


T = TypeVar("T", bound="NewAccount")


@_attrs_define
class NewAccount:
    """
    Attributes:
        kind (AccountKind): Affects display only (sign convention, icon), never a split's kind.
        name (str):
        owner_id (UUID):
        archived (bool | Unset):
        credit_limit (CreditLimit | None | Unset):
        liquid (bool | None | Unset): Defaults to `false` for brokerage accounts, `true` otherwise.
    """

    kind: AccountKind
    name: str
    owner_id: UUID
    archived: bool | Unset = UNSET
    credit_limit: CreditLimit | None | Unset = UNSET
    liquid: bool | None | Unset = UNSET

    def to_dict(self) -> dict[str, Any]:
        from ..models.credit_limit import CreditLimit  # noqa: PLC0415

        kind = self.kind.value

        name = self.name

        owner_id = str(self.owner_id)

        archived = self.archived

        credit_limit: dict[str, Any] | None | Unset
        if isinstance(self.credit_limit, Unset):
            credit_limit = UNSET
        elif isinstance(self.credit_limit, CreditLimit):
            credit_limit = self.credit_limit.to_dict()
        else:
            credit_limit = self.credit_limit

        liquid: bool | None | Unset
        if isinstance(self.liquid, Unset):
            liquid = UNSET
        else:
            liquid = self.liquid

        field_dict: dict[str, Any] = {}

        field_dict.update(
            {
                "kind": kind,
                "name": name,
                "owner_id": owner_id,
            }
        )
        if archived is not UNSET:
            field_dict["archived"] = archived
        if credit_limit is not UNSET:
            field_dict["credit_limit"] = credit_limit
        if liquid is not UNSET:
            field_dict["liquid"] = liquid

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.credit_limit import CreditLimit  # noqa: PLC0415

        d = dict(src_dict)
        kind = AccountKind(d.pop("kind"))

        name = d.pop("name")

        owner_id = UUID(d.pop("owner_id"))

        archived = d.pop("archived", UNSET)

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

        def _parse_liquid(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        liquid = _parse_liquid(d.pop("liquid", UNSET))

        new_account = cls(
            kind=kind,
            name=name,
            owner_id=owner_id,
            archived=archived,
            credit_limit=credit_limit,
            liquid=liquid,
        )

        return new_account
