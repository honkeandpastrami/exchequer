"""`exchequer tokens` and `exchequer sessions` (design §10). A token is made
for a script; a session is a browser, the CLI or a script using a token. Each
token belongs to a user and expires; any can be revoked, or all at once."""

from __future__ import annotations

import datetime
from uuid import UUID

from exchequer_cli.imports import ApiError, ok
from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.sessions import sessions_list, sessions_sign_out
from exchequer_sdk.api.tokens import tokens_create, tokens_list, tokens_remove, tokens_revoke_all
from exchequer_sdk.api.users import users_list, users_me
from exchequer_sdk.models import CreatedToken, NewToken, Session, Token

KINDS = {"browser": "Browser", "cli": "CLI", "script": "Script"}


def _day(t: object) -> str:
    return t.astimezone().date().isoformat() if isinstance(t, datetime.datetime) else "—"


def _text(v: object) -> str:
    return v if isinstance(v, str) and v else "—"


def _columns(rows: list[tuple[str, ...]]) -> list[str]:
    widths = [max(len(r[i]) for r in rows) for i in range(len(rows[0]))]
    return ["  ".join(c.ljust(w) for c, w in zip(r, widths, strict=True)).rstrip() for r in rows]


def _users(client: AuthenticatedClient) -> dict[UUID, str]:
    return {u.id: u.name for u in ok(users_list.sync_detailed(client=client), "list users")}


def working(t: Token, now: datetime.datetime) -> bool:
    return not isinstance(t.revoked_at, datetime.datetime) and t.expires_at > now


def table(client: AuthenticatedClient, *, everything: bool = False) -> list[str]:
    """Scripts' tokens still working (`everything`: revoked and expired too)."""
    tokens = ok(tokens_list.sync_detailed(client=client), "list tokens")
    users = _users(client)
    now = datetime.datetime.now(datetime.UTC)
    shown = [t for t in tokens if everything or working(t, now)]
    if not shown:
        return ["No tokens."]
    rows = [("NAME", "USER", "CREATED", "LAST USED", "EXPIRES", "")]
    for t in shown:
        if isinstance(t.revoked_at, datetime.datetime):
            state = f"revoked {_day(t.revoked_at)}"
        elif t.expires_at <= now:
            state = "expired"
        else:
            state = ""
        user = users.get(t.user_id, "—") if isinstance(t.user_id, UUID) else "—"
        used = _day(t.last_used_at)
        rows.append((t.name, user, _day(t.created_at), used, _day(t.expires_at), state))
    return _columns(rows)


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
    ok(tokens_remove.sync_detailed(client=client, id=token.id), "revoke", 204)
    return token


def revoke_all(client: AuthenticatedClient) -> int:
    """Signs everyone out and revokes every token, this CLI's too; returns how
    many were working."""
    return ok(tokens_revoke_all.sync_detailed(client=client), "revoke all").revoked


def short(s: Session) -> str:
    """What `sessions sign-out` takes: the end of its ID (the start is a time)."""
    return s.id.hex[-8:]


def _from(s: Session) -> str:
    return " · ".join(v for v in (s.client, s.host) if isinstance(v, str) and v) or "—"


def sessions(client: AuthenticatedClient) -> list[str]:
    """The sessions whose tokens still work, last used first."""
    found = ok(sessions_list.sync_detailed(client=client), "list sessions")
    if not found:
        return ["No sessions."]
    users = _users(client)
    me = ok(users_me.sync_detailed(client=client), "who am I")
    mine = me.session.id if isinstance(me.session, Session) else None
    rows = [("ID", "USER", "KIND", "SIGNED IN WITH", "STARTED", "LAST USED", "FROM", "EXPIRES", "")]
    for s in found:
        user = users.get(s.user_id, "—") if isinstance(s.user_id, UUID) else "—"
        rows.append(
            (
                short(s),
                user,
                KINDS[s.kind.value],
                _text(s.signed_in_with),
                _day(s.started_at),
                _day(s.last_used_at),
                _from(s),
                _day(s.expires_at),
                "this CLI" if s.id == mine else "",
            )
        )
    return _columns(rows)


def find_session(client: AuthenticatedClient, which: str) -> Session:
    """The session with this ID, or whose ID ends so."""
    found = ok(sessions_list.sync_detailed(client=client), "list sessions")
    which = which.strip().lower().replace("-", "")
    matches = [s for s in found if s.id.hex.endswith(which)] if len(which) >= 4 else []
    if len(matches) != 1:
        raise ApiError(404, f"no session {which!r}" if not matches else f"{which!r} is ambiguous")
    return matches[0]


def others_on_token(client: AuthenticatedClient, session: Session) -> list[Session]:
    """The other sessions that end with it: they use its token (a script's)."""
    found = ok(sessions_list.sync_detailed(client=client), "list sessions")
    return [s for s in found if s.id != session.id and s.token_id == session.token_id]


def sign_out(client: AuthenticatedClient, session: Session) -> None:
    ok(sessions_sign_out.sync_detailed(client=client, id=session.id), "sign out", 204)
