"""A read-only Firefly III API client (design §11).

The migration must never change the Firefly instance it reads. That is
enforced here, not by convention: every request goes through a transport that
refuses any method but GET before anything reaches the network, and only
`FireflyReader`'s read methods exist. Pagination links are followed only on
the configured host, so the token is never sent anywhere else.
"""

from __future__ import annotations

from collections.abc import Iterator
from datetime import date
from typing import Any
from urllib.parse import urlsplit

import httpx


class WriteRefused(RuntimeError):
    """A request other than GET was attempted against Firefly."""


class _ReadOnlyTransport(httpx.BaseTransport):
    def __init__(self, inner: httpx.BaseTransport) -> None:
        self._inner = inner

    def handle_request(self, request: httpx.Request) -> httpx.Response:
        if request.method != "GET":
            raise WriteRefused(f"refused {request.method} {request.url.path}: Firefly is read-only")
        return self._inner.handle_request(request)

    def close(self) -> None:
        self._inner.close()


class FireflyReader:
    """GET-only access to the parts of the Firefly API the migration needs."""

    def __init__(
        self,
        api_base: str,
        token: str,
        *,
        timeout: int = 120,
        transport: httpx.BaseTransport | None = None,
    ) -> None:
        self._base = api_base.rstrip("/")
        self._origin = _origin(self._base)
        self._http = httpx.Client(
            transport=_ReadOnlyTransport(transport or httpx.HTTPTransport()),
            headers={
                "Authorization": f"Bearer {token}",
                "Accept": "application/vnd.api+json",
            },
            timeout=timeout,
        )

    def close(self) -> None:
        self._http.close()

    def __enter__(self) -> FireflyReader:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def _get(self, url: str, params: dict[str, str] | None = None) -> dict[str, Any]:
        if _origin(url) != self._origin:
            raise ValueError(f"refusing to follow a link to another host: {_origin(url)}")
        response = self._http.get(url, params=params)
        response.raise_for_status()
        body: dict[str, Any] = response.json()
        return body

    def _pages(self, path: str, params: dict[str, str]) -> Iterator[dict[str, Any]]:
        url: str | None = f"{self._base}{path}"
        query: dict[str, str] | None = {"limit": "500", **params}
        while url:
            body = self._get(url, query)
            yield from body.get("data", [])
            url = body.get("links", {}).get("next") or None
            query = None  # the next link carries the query

    def about(self) -> dict[str, Any]:
        data: dict[str, Any] = self._get(f"{self._base}/about")["data"]
        return data

    def currencies(self) -> list[dict[str, Any]]:
        return list(self._pages("/currencies", {}))

    def accounts(self) -> list[dict[str, Any]]:
        """Every account of every type (asset, liability, expense, revenue, ...)."""
        return list(self._pages("/accounts", {"type": "all"}))

    def account_on(self, account_id: int, day: date) -> dict[str, Any]:
        """One account, with its balance as Firefly reports it at the end of `day`."""
        data: dict[str, Any] = self._get(
            f"{self._base}/accounts/{account_id}", {"date": day.isoformat()}
        )["data"]
        return data

    def transactions(self, start: date, end: date) -> list[dict[str, Any]]:
        """Transaction groups dated `start` to `end`, inclusive."""
        return list(
            self._pages(
                "/transactions",
                {"type": "all", "start": start.isoformat(), "end": end.isoformat()},
            )
        )


def _origin(url: str) -> tuple[str, str]:
    parts = urlsplit(url)
    return (parts.scheme, parts.netloc)
