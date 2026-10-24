"""`exchequer prices refresh`: the server fetches in the background (design
§3.4); this starts a refresh and waits for it to finish."""

from __future__ import annotations

from exchequer_cli.imports import ok
from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.prices import prices_refresh, prices_refresh_status
from exchequer_sdk.models import Refreshed, RefreshRequest

# Each request waits this long for the refresh to finish (the server, at most 50).
WAIT_SECONDS = 25


def refresh(client: AuthenticatedClient, body: RefreshRequest) -> Refreshed:
    """Starts a refresh (or joins the one running) and returns how it went."""
    r = ok(prices_refresh.sync_detailed(client=client, body=body), "refresh", 202)
    while r.running:
        r = ok(prices_refresh_status.sync_detailed(client=client, wait=WAIT_SECONDS), "refresh")
    return r
