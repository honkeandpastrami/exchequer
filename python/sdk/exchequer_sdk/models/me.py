from __future__ import annotations

from collections.abc import Mapping
from typing import TYPE_CHECKING, Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

if TYPE_CHECKING:
    from ..models.token import Token
    from ..models.user import User


T = TypeVar("T", bound="Me")


@_attrs_define
class Me:
    """Who the request's token belongs to.

    Attributes:
        token (None | Token | Unset):
        user (None | Unset | User):
    """

    token: None | Token | Unset = UNSET
    user: None | Unset | User = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        from ..models.token import Token  # noqa: PLC0415
        from ..models.user import User  # noqa: PLC0415

        token: dict[str, Any] | None | Unset
        if isinstance(self.token, Unset):
            token = UNSET
        elif isinstance(self.token, Token):
            token = self.token.to_dict()
        else:
            token = self.token

        user: dict[str, Any] | None | Unset
        if isinstance(self.user, Unset):
            user = UNSET
        elif isinstance(self.user, User):
            user = self.user.to_dict()
        else:
            user = self.user

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({})
        if token is not UNSET:
            field_dict["token"] = token
        if user is not UNSET:
            field_dict["user"] = user

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.token import Token  # noqa: PLC0415
        from ..models.user import User  # noqa: PLC0415

        d = dict(src_dict)

        def _parse_token(data: object) -> None | Token | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                token_type_0 = Token.from_dict(data)

                return token_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Token | Unset, data)

        token = _parse_token(d.pop("token", UNSET))

        def _parse_user(data: object) -> None | Unset | User:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                user_type_0 = User.from_dict(data)

                return user_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | User, data)

        user = _parse_user(d.pop("user", UNSET))

        me = cls(
            token=token,
            user=user,
        )

        me.additional_properties = d
        return me

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
