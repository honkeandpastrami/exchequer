"""`exchequer login`: signing in through the browser (design §10). The CLI shows
a link and a code, a signed-in user approves it in the web app, and the CLI
gets a token of its own, written to the token file and never printed."""

from __future__ import annotations

import os
import time
from collections.abc import Callable
from pathlib import Path

from exchequer_cli.imports import ApiError, ok
from exchequer_sdk import Client
from exchequer_sdk.api.auth import auth_device_token, auth_start_device
from exchequer_sdk.models import PollDevice, StartDevice


def write_token(path: Path, token: str) -> None:
    """Replaces the token file at once, readable only by its owner."""
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    tmp = path.with_name(f".{path.name}.tmp")
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(token + "\n")
    os.replace(tmp, path)


def login(
    client: Client,
    url: str,
    token_file: Path,
    *,
    name: str,
    days: int,
    say: Callable[[str], None],
    open_url: Callable[[str], object] = lambda _: None,
    sleep: Callable[[float], None] = time.sleep,
) -> int:
    body = StartDevice(client=name, ttl_days=days)
    start = ok(auth_start_device.sync_detailed(client=client, body=body), "start signing in")
    link = f"{url}/#/device/{start.user_code}"
    say(f"Open {link}")
    say(f"and approve the code {start.user_code} (signed in as yourself).")
    open_url(link)
    poll = PollDevice(device_code=start.device_code)
    while True:
        sleep(start.interval)
        r = auth_device_token.sync_detailed(client=client, body=poll)
        if r.status_code == 404:
            say("The code expired before it was approved; run exchequer login again.")
            return 1
        token = ok(r, "sign in").token
        if isinstance(token, str):
            write_token(token_file, token)
            say(f"Signed in for {days} days. The token is in {token_file}.")
            return 0


__all__ = ["ApiError", "login", "write_token"]
