"""`exchequer tokens` against a fake server."""

import json

import httpx
import pytest

from exchequer_cli import tokens
from exchequer_cli.imports import ApiError
from exchequer_sdk import AuthenticatedClient

ALEX = "00000000-0000-0000-0000-00000000000a"
MINE = "00000000-0000-0000-0000-000000000001"
SCRIPT = "00000000-0000-0000-0000-000000000002"
OLD = "00000000-0000-0000-0000-000000000003"


def token(id: str, name: str, kind: str, **more) -> dict:
    return {
        "id": id,
        "name": name,
        "kind": kind,
        "user_id": ALEX,
        "created_at": "2026-10-01T12:00:00Z",
        "expires_at": "2099-01-01T12:00:00Z",
        **more,
    }


TOKENS = [
    token(MINE, "Alex · cli on laptop · a1b2c3", "cli", last_used_at="2026-10-24T09:00:00Z"),
    token(SCRIPT, "backup", "script"),
    token(OLD, "old", "script", revoked_at="2026-10-02T12:00:00Z"),
]


def server() -> tuple[AuthenticatedClient, list[str]]:
    asked: list[str] = []

    users = [{"id": ALEX, "name": "Alex", "created_at": "2026-09-01T00:00:00Z", "passkeys": []}]
    answers = {
        "GET /tokens": httpx.Response(200, json=TOKENS),
        "GET /users": httpx.Response(200, json=users),
        "GET /me": httpx.Response(200, json={"token": TOKENS[0]}),
        "DELETE /tokens": httpx.Response(200, json={"revoked": 2}),
    }

    def handle(req: httpx.Request) -> httpx.Response:
        asked.append(f"{req.method} {req.url.path.removeprefix('/x/api/v1')}")
        if asked[-1] == "POST /tokens":
            made = token(SCRIPT, json.loads(req.content)["name"], "script")
            return httpx.Response(201, json={**made, "token": "exq_new"})
        if asked[-1].startswith("DELETE /tokens/"):
            return httpx.Response(204)
        return answers.get(asked[-1], httpx.Response(404))

    client = AuthenticatedClient(
        base_url="https://books.example.org/x",
        token="exq_test",
        httpx_args={"transport": httpx.MockTransport(handle)},
    )
    return client, asked


def test_the_list_shows_what_passkeys_show():
    client, _ = server()
    lines = tokens.table(client)
    assert lines[0].split() == ["NAME", "USER", "KIND", "CREATED", "LAST", "USED", "EXPIRES"]
    assert len(lines) == 3, "the revoked one is left out"
    assert "Alex" in lines[1] and "2026-10-24" in lines[1] and lines[1].endswith("this CLI")
    assert "backup" in lines[2] and "—" in lines[2], "never used"
    everything = tokens.table(client, everything=True)
    assert everything[3].endswith("revoked 2026-10-02")


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
    assert asked == ["POST /tokens", "DELETE /tokens"]
