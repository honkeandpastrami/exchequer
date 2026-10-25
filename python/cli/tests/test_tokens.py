"""`exchequer tokens` and `exchequer sessions` against a fake server."""

import json

import httpx
import pytest

from exchequer_cli import tokens
from exchequer_cli.imports import ApiError
from exchequer_sdk import AuthenticatedClient

ALEX = "00000000-0000-0000-0000-00000000000a"
SCRIPT = "00000000-0000-0000-0000-000000000002"
OLD = "00000000-0000-0000-0000-000000000003"
CLI_TOKEN = "00000000-0000-0000-0000-000000000004"
CLI = "0190a000-0000-7000-8000-0000c11c11c1"
NAS = "0190a000-0000-7000-8000-00000000a5a5"
DESKTOP = "0190a000-0000-7000-8000-00000011a5a5"


def token(id: str, name: str, **more) -> dict:
    return {
        "id": id,
        "name": name,
        "user_id": ALEX,
        "created_at": "2026-10-01T12:00:00Z",
        "expires_at": "2099-01-01T12:00:00Z",
        **more,
    }


def session(id: str, token_id: str, kind: str, **more) -> dict:
    return {
        "id": id,
        "token_id": token_id,
        "kind": kind,
        "user_id": ALEX,
        "started_at": "2026-10-01T12:00:00Z",
        "last_used_at": "2026-10-24T09:00:00Z",
        "expires_at": "2099-01-01T12:00:00Z",
        **more,
    }


TOKENS = [
    token(SCRIPT, "backup", last_used_at="2026-10-24T09:00:00Z"),
    token(OLD, "old", revoked_at="2026-10-02T12:00:00Z"),
]
SESSIONS = [
    session(
        CLI, CLI_TOKEN, "cli", signed_in_with="iPhone", client="exchequer-cli 1.4.0", host="laptop"
    ),
    session(NAS, SCRIPT, "script", signed_in_with="backup", client="backup.py", host="nas"),
    session(DESKTOP, SCRIPT, "script", signed_in_with="backup"),
]


def server() -> tuple[AuthenticatedClient, list[str]]:
    asked: list[str] = []

    users = [{"id": ALEX, "name": "Alex", "created_at": "2026-09-01T00:00:00Z", "passkeys": []}]
    answers = {
        "GET /tokens": httpx.Response(200, json=TOKENS),
        "GET /sessions": httpx.Response(200, json=SESSIONS),
        "GET /users": httpx.Response(200, json=users),
        "GET /me": httpx.Response(200, json={"session": SESSIONS[0]}),
        "POST /revoke-all": httpx.Response(200, json={"revoked": 2}),
    }

    def handle(req: httpx.Request) -> httpx.Response:
        asked.append(f"{req.method} {req.url.path.removeprefix('/x/api/v1')}")
        if asked[-1] == "POST /tokens":
            made = token(SCRIPT, json.loads(req.content)["name"])
            return httpx.Response(201, json={**made, "token": "exq_new"})
        if asked[-1].startswith(("DELETE /tokens/", "DELETE /sessions/")):
            return httpx.Response(204)
        return answers.get(asked[-1], httpx.Response(404))

    client = AuthenticatedClient(
        base_url="https://books.example.org/x",
        token="exq_test",
        httpx_args={"transport": httpx.MockTransport(handle)},
    )
    return client, asked


def test_the_list_is_scripts_tokens():
    client, _ = server()
    lines = tokens.table(client)
    assert lines[0].split() == ["NAME", "USER", "CREATED", "LAST", "USED", "EXPIRES"]
    assert len(lines) == 2, "the revoked one is left out"
    assert "backup" in lines[1] and "Alex" in lines[1] and "2026-10-24" in lines[1]
    everything = tokens.table(client, everything=True)
    assert everything[2].endswith("revoked 2026-10-02")


def test_a_token_is_revoked_by_name():
    client, asked = server()
    assert tokens.revoke(client, "backup").id.hex.endswith("2")
    assert asked[-1] == f"DELETE /tokens/{SCRIPT}"
    with pytest.raises(ApiError, match="no token named"):
        tokens.revoke(client, "nope")


def test_create_and_revoke_all():
    client, asked = server()
    assert tokens.create(client, "cron", 30).token == "exq_new"
    assert tokens.revoke_all(client) == 2
    assert asked == ["POST /tokens", "POST /revoke-all"]


def test_sessions_show_who_with_what_and_from_where():
    client, _ = server()
    lines = tokens.sessions(client)
    assert lines[0].split()[:4] == ["ID", "USER", "KIND", "SIGNED"]
    assert lines[1].startswith("c11c11c1") and "iPhone" in lines[1]
    assert "exchequer-cli 1.4.0 · laptop" in lines[1] and lines[1].endswith("this CLI")
    assert "backup.py · nas" in lines[2]
    assert "—" in lines[3], "a client that said nothing"


def test_a_session_is_signed_out_by_the_end_of_its_id():
    client, asked = server()
    nas = tokens.find_session(client, "0000A5A5")
    assert [tokens.short(s) for s in tokens.others_on_token(client, nas)] == ["0011a5a5"]
    assert tokens.others_on_token(client, tokens.find_session(client, "c11c11c1")) == []
    tokens.sign_out(client, nas)
    assert asked[-1] == f"DELETE /sessions/{NAS}"
    with pytest.raises(ApiError, match="no session"):
        tokens.find_session(client, "ffff")
    with pytest.raises(ApiError, match="ambiguous"):
        tokens.find_session(client, "a5a5")
