"""The Firefly client can only read: nothing but GET ever reaches the network."""

import httpx
import pytest

from firefly_migrate.firefly import FireflyReader, WriteRefused

BASE = "https://firefly.example.com/api/v1"


def reader(handler):
    seen: list[httpx.Request] = []

    def record(request: httpx.Request) -> httpx.Response:
        seen.append(request)
        return handler(request)

    return FireflyReader(BASE, "secret", transport=httpx.MockTransport(record)), seen


@pytest.mark.parametrize("method", ["POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"])
def test_every_other_method_is_refused_before_the_network(method):
    ff, seen = reader(lambda _: httpx.Response(200, json={}))
    with pytest.raises(WriteRefused):
        ff._http.request(method, f"{BASE}/transactions")
    assert seen == []


def test_reads_follow_pagination_with_the_token():
    pages = {
        "1": {"data": [{"id": "1"}], "links": {"next": f"{BASE}/currencies?page=2"}},
        "2": {"data": [{"id": "2"}], "links": {}},
    }

    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(200, json=pages[request.url.params.get("page", "1")])

    ff, seen = reader(handler)
    assert [c["id"] for c in ff.currencies()] == ["1", "2"]
    assert {r.method for r in seen} == {"GET"}
    assert all(r.headers["Authorization"] == "Bearer secret" for r in seen)


def test_the_token_never_follows_a_link_to_another_host():
    ff, seen = reader(
        lambda _: httpx.Response(
            200, json={"data": [], "links": {"next": "https://elsewhere.example/x"}}
        )
    )
    with pytest.raises(ValueError, match="another host"):
        ff.currencies()
    assert len(seen) == 1
