"""`exchequer prices refresh` against a fake server."""

import json

import httpx

from exchequer_cli.prices import refresh
from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.models import RefreshRequest


def status(running: bool, stored: int = 0, errors: list[str] | None = None) -> dict:
    out = {
        "sources": ["tiingo", "ecb"],
        "running": running,
        "started_at": "2026-10-24T10:00:00Z",
        "stored": stored,
        "errors": errors or [],
    }
    if not running:
        out["finished_at"] = "2026-10-24T10:00:05Z"
    return out


def test_refresh_waits_until_the_server_is_done():
    asked: list[str] = []
    polls = [
        httpx.Response(200, json=status(True)),
        httpx.Response(200, json=status(False, 3, ["ecb: rate-limited"])),
    ]

    def handle(req: httpx.Request) -> httpx.Response:
        asked.append(f"{req.method} {req.url.path}?{req.url.query.decode()}".rstrip("?"))
        if req.method == "POST":
            assert json.loads(req.content) == {"value_in": "USD"}
            return httpx.Response(202, json=status(True))
        return polls.pop(0)

    client = AuthenticatedClient(
        base_url="https://books.example.org/x",
        token="exq_test",
        httpx_args={"transport": httpx.MockTransport(handle)},
    )
    r = refresh(client, RefreshRequest(value_in="USD"))
    assert not r.running
    assert r.stored == 3
    assert r.errors == ["ecb: rate-limited"]
    assert asked == ["POST /x/api/v1/prices/refresh"] + ["GET /x/api/v1/prices/refresh?wait=25"] * 2
