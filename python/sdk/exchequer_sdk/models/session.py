from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, TypeVar, cast
from uuid import UUID

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..models.session_kind import SessionKind
from ..types import UNSET, Unset

T = TypeVar("T", bound="Session")


@_attrs_define
class Session:
    """A client using a token that still works.

    Attributes:
        expires_at (datetime.datetime): When its token stops working, unless revoked first.
        id (UUID):
        kind (SessionKind): What signed a session in.
        last_used_at (datetime.datetime): To the minute.
        started_at (datetime.datetime):
        token_id (UUID): The token it uses: signing out revokes it, ending every session
            using it.
        client (None | str | Unset): What it last used: `Safari on iPhone`, `exchequer-cli 1.4.0`. As the
            client reports it (`User-Agent`).
        host (None | str | Unset): The host it last used it from, as the client reports it
            (`Exchequer-Host`).
        ip (None | str | Unset): The address its last request came from, as the reverse proxy reports
            it (`X-Forwarded-For`).
        signed_in_with (None | str | Unset): The passkey's name, or the script's token's; none if the passkey was
            deleted, or for a session older than this record.
        user_id (None | Unset | UUID):
    """

    expires_at: datetime.datetime
    id: UUID
    kind: SessionKind
    last_used_at: datetime.datetime
    started_at: datetime.datetime
    token_id: UUID
    client: None | str | Unset = UNSET
    host: None | str | Unset = UNSET
    ip: None | str | Unset = UNSET
    signed_in_with: None | str | Unset = UNSET
    user_id: None | Unset | UUID = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        expires_at = self.expires_at.isoformat()

        id = str(self.id)

        kind = self.kind.value

        last_used_at = self.last_used_at.isoformat()

        started_at = self.started_at.isoformat()

        token_id = str(self.token_id)

        client: None | str | Unset
        if isinstance(self.client, Unset):
            client = UNSET
        else:
            client = self.client

        host: None | str | Unset
        if isinstance(self.host, Unset):
            host = UNSET
        else:
            host = self.host

        ip: None | str | Unset
        if isinstance(self.ip, Unset):
            ip = UNSET
        else:
            ip = self.ip

        signed_in_with: None | str | Unset
        if isinstance(self.signed_in_with, Unset):
            signed_in_with = UNSET
        else:
            signed_in_with = self.signed_in_with

        user_id: None | str | Unset
        if isinstance(self.user_id, Unset):
            user_id = UNSET
        elif isinstance(self.user_id, UUID):
            user_id = str(self.user_id)
        else:
            user_id = self.user_id

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "expires_at": expires_at,
                "id": id,
                "kind": kind,
                "last_used_at": last_used_at,
                "started_at": started_at,
                "token_id": token_id,
            }
        )
        if client is not UNSET:
            field_dict["client"] = client
        if host is not UNSET:
            field_dict["host"] = host
        if ip is not UNSET:
            field_dict["ip"] = ip
        if signed_in_with is not UNSET:
            field_dict["signed_in_with"] = signed_in_with
        if user_id is not UNSET:
            field_dict["user_id"] = user_id

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        expires_at = datetime.datetime.fromisoformat(d.pop("expires_at"))

        id = UUID(d.pop("id"))

        kind = SessionKind(d.pop("kind"))

        last_used_at = datetime.datetime.fromisoformat(d.pop("last_used_at"))

        started_at = datetime.datetime.fromisoformat(d.pop("started_at"))

        token_id = UUID(d.pop("token_id"))

        def _parse_client(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        client = _parse_client(d.pop("client", UNSET))

        def _parse_host(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        host = _parse_host(d.pop("host", UNSET))

        def _parse_ip(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        ip = _parse_ip(d.pop("ip", UNSET))

        def _parse_signed_in_with(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        signed_in_with = _parse_signed_in_with(d.pop("signed_in_with", UNSET))

        def _parse_user_id(data: object) -> None | Unset | UUID:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, str):
                    raise TypeError()
                user_id_type_0 = UUID(data)

                return user_id_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | Unset | UUID, data)

        user_id = _parse_user_id(d.pop("user_id", UNSET))

        session = cls(
            expires_at=expires_at,
            id=id,
            kind=kind,
            last_used_at=last_used_at,
            started_at=started_at,
            token_id=token_id,
            client=client,
            host=host,
            ip=ip,
            signed_in_with=signed_in_with,
            user_id=user_id,
        )

        session.additional_properties = d
        return session

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
