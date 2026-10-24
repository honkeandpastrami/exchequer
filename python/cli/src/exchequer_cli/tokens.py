"""`exchequer tokens`: the server's tokens (design §10). Each belongs to a user
and expires; any can be revoked, or all at once."""

from __future__ import annotations

import datetime
from uuid import UUID

from exchequer_cli.imports import ApiError, ok
from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.tokens import tokens_create, tokens_list, tokens_revoke, tokens_revoke_all
from exchequer_sdk.api.users import users_list, users_me
from exchequer_sdk.models import CreatedToken, NewToken, Token


def _day(t: datetime.datetime | None) -> str:
    return t.astimezone().date().isoformat() if isinstance(t, datetime.datetime) else "—"


def working(t: Token, now: datetime.datetime) -> bool:
    return not isinstance(t.revoked_at, datetime.datetime) and t.expires_at > now


def table(client: AuthenticatedClient, *, everything: bool = False) -> list[str]:
    """The tokens still working (`everything`: revoked and expired too)."""
    tokens = ok(tokens_list.sync_detailed(client=client), "list tokens")
    users = {u.id: u.name for u in ok(users_list.sync_detailed(client=client), "list users")}
    me = ok(users_me.sync_detailed(client=client), "who am I")
    mine = me.token.id if isinstance(me.token, Token) else None
    now = datetime.datetime.now(datetime.UTC)
    shown = [t for t in tokens if everything or working(t, now)]
    if not shown:
        return ["No tokens."]
    rows = [("NAME", "USER", "KIND", "CREATED", "LAST USED", "EXPIRES", "")]
    for t in shown:
        if isinstance(t.revoked_at, datetime.datetime):
            state = f"revoked {_day(t.revoked_at)}"
        elif t.expires_at <= now:
            state = "expired"
        else:
            state = "this CLI" if t.id == mine else ""
        user = users.get(t.user_id, "—") if isinstance(t.user_id, UUID) else "—"
        kind = t.kind.value
        used = _day(t.last_used_at)
        rows.append((t.name, user, kind, _day(t.created_at), used, _day(t.expires_at), state))
    widths = [max(len(r[i]) for r in rows) for i in range(len(rows[0]))]
    return ["  ".join(c.ljust(w) for c, w in zip(r, widths, strict=True)).rstrip() for r in rows]


def create(client: AuthenticatedClient, name: str, days: int) -> CreatedToken:
    body = NewToken(name=name, ttl_days=days)
    return ok(tokens_create.sync_detailed(client=client, body=body), "create", 201)


def revoke(client: AuthenticatedClient, which: str) -> Token:
    """Revokes the token named `which` (or with that ID); returns it."""
    tokens = ok(tokens_list.sync_detailed(client=client), "list tokens")
    found = [t for t in tokens if t.name == which or str(t.id) == which]
    if not found:
        raise ApiError(404, f"no token named {which!r}")
    token = found[0]
    ok(tokens_revoke.sync_detailed(client=client, id=token.id), "revoke", 204)
    return token


def revoke_all(client: AuthenticatedClient) -> int:
    """Revokes every token, this CLI's too; returns how many were working."""
    return ok(tokens_revoke_all.sync_detailed(client=client), "revoke all").revoked
