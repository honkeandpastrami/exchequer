from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar

from attrs import define as _attrs_define
from attrs import field as _attrs_field

if TYPE_CHECKING:
    from ..models.invite import Invite
    from ..models.user import User


T = TypeVar("T", bound="Invited")


@_attrs_define
class Invited:
    """An invite made: send `invite.code`'s link to `user`.

    Attributes:
        added (bool): Whether the user was added just now.
        invite (Invite): A one-time link's secret: the web app opens it at `#/invite/<code>`.
        user (User): A household member who signs in.
    """

    added: bool
    invite: Invite
    user: User
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        added = self.added

        invite = self.invite.to_dict()

        user = self.user.to_dict()

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "added": added,
                "invite": invite,
                "user": user,
            }
        )

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.invite import Invite  # noqa: PLC0415
        from ..models.user import User  # noqa: PLC0415

        d = dict(src_dict)
        added = d.pop("added")

        invite = Invite.from_dict(d.pop("invite"))

        user = User.from_dict(d.pop("user"))

        invited = cls(
            added=added,
            invite=invite,
            user=user,
        )

        invited.additional_properties = d
        return invited

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
