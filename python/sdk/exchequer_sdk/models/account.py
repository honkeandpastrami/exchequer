from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.account_kind import AccountKind
from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.credit_limit import CreditLimit
    from ..models.number import Number


T = TypeVar("T", bound="Account")


@_attrs_define
class Account:
    """
    Attributes:
        archived (bool): Kept for history, hidden from everyday views.
        id (UUID):
        kind (AccountKind): Affects display only (sign convention, icon), never a split's kind.
        liquid (bool): Investment accounts and mortgages are not liquid: activity inside them
            is never shared, and money into or out of them is an expense or
            income (design §4.1).
        name (str):
        numbers (list[Number]): Numbers bank files name this account by, oldest first.
        owner_id (UUID): The party the account belongs to.
        credit_limit (CreditLimit | None | Unset):
    """

    archived: bool
    id: UUID
    kind: AccountKind
    liquid: bool
    name: str
    numbers: list[Number]
    owner_id: UUID
    credit_limit: CreditLimit | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.credit_limit import CreditLimit  # noqa: PLC0415

        archived = self.archived

        id = str(self.id)

        kind = self.kind.value

        liquid = self.liquid

        name = self.name

        numbers = []
        for numbers_item_data in self.numbers:
            numbers_item = numbers_item_data.to_dict()
            numbers.append(numbers_item)

        owner_id = str(self.owner_id)

        credit_limit: dict[str, Any] | None | Unset
        if isinstance(self.credit_limit, Unset):
            credit_limit = UNSET
        elif isinstance(self.credit_limit, CreditLimit):
            credit_limit = self.credit_limit.to_dict()
        else:
            credit_limit = self.credit_limit

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "archived": archived,
                "id": id,
                "kind": kind,
                "liquid": liquid,
                "name": name,
                "numbers": numbers,
                "owner_id": owner_id,
            }
        )
        if credit_limit is not UNSET:
            field_dict["credit_limit"] = credit_limit

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.credit_limit import CreditLimit  # noqa: PLC0415
        from ..models.number import Number  # noqa: PLC0415

        d = dict(src_dict)
        archived = d.pop("archived")

        id = UUID(d.pop("id"))

        kind = AccountKind(d.pop("kind"))

        liquid = d.pop("liquid")

        name = d.pop("name")

        numbers = []
        _numbers = d.pop("numbers")
        for numbers_item_data in _numbers:
            numbers_item = Number.from_dict(numbers_item_data)

            numbers.append(numbers_item)

        owner_id = UUID(d.pop("owner_id"))

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

        account = cls(
            archived=archived,
            id=id,
            kind=kind,
            liquid=liquid,
            name=name,
            numbers=numbers,
            owner_id=owner_id,
            credit_limit=credit_limit,
        )

        account.additional_properties = d
        return account

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
