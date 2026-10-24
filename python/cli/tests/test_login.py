"""`exchequer login` against a fake server."""

import json
import stat
from pathlib import Path

import httpx

from exchequer_cli.login import login, write_token
from exchequer_sdk import Client

DEVICE = {
    "device_code": "exd_secret",
    "user_code": "BCDF-GHJK",
    "expires_at": "2026-10-24T10:00:00Z",
    "interval": 2,
}


def server(polls: list[httpx.Response]) -> tuple[Client, list[dict]]:
    """Answers the start, then each poll in turn; records the bodies sent."""
    sent: list[dict] = []

    def handle(req: httpx.Request) -> httpx.Response:
        sent.append({"path": req.url.path, **json.loads(req.content)})
        if req.url.path.endswith("/auth/device"):
            return httpx.Response(200, json=DEVICE)
        return polls.pop(0)

    client = Client(
        base_url="https://books.example.org/x",
        httpx_args={"transport": httpx.MockTransport(handle)},
    )
    return client, sent


def test_login_waits_for_approval_and_writes_the_token(tmp_path: Path):
    waiting = httpx.Response(200, json={"token": None})
    approved = httpx.Response(200, json={"token": "exq_new"})
    client, sent = server([waiting, waiting, approved])
    said: list[str] = []
    opened: list[str] = []
    token_file = tmp_path / "conf" / "token"
    code = login(
        client,
        "https://books.example.org/x",
        token_file,
        name="cli on laptop",
        say=said.append,
        open_url=opened.append,
        sleep=lambda _: None,
    )
    assert code == 0
    assert sent[0] == {"path": "/x/api/v1/auth/device", "client": "cli on laptop"}
    assert sent[1] == {"path": "/x/api/v1/auth/device/token", "device_code": "exd_secret"}
    assert opened == ["https://books.example.org/x/#/device/BCDF-GHJK"]
    assert any("BCDF-GHJK" in s for s in said)
    assert not any("exq_new" in s for s in said), "the token is never printed"
    assert token_file.read_text() == "exq_new\n"
    assert stat.S_IMODE(token_file.stat().st_mode) == 0o600


def test_login_gives_up_when_the_code_expires(tmp_path: Path):
    gone = httpx.Response(404, json={"type": "x", "title": "Not found", "status": 404})
    client, _ = server([gone])
    said: list[str] = []
    token_file = tmp_path / "token"
    code = login(
        client, "https://b.example", token_file, name="cli", say=said.append, sleep=lambda _: None
    )
    assert code == 1
    assert not token_file.exists()


def test_the_token_file_is_replaced_whole(tmp_path: Path):
    f = tmp_path / "token"
    f.write_text("exq_old\n")
    f.chmod(0o644)
    write_token(f, "exq_new")
    assert f.read_text() == "exq_new\n"
    assert stat.S_IMODE(f.stat().st_mode) == 0o600
    assert [p.name for p in tmp_path.iterdir()] == ["token"]
