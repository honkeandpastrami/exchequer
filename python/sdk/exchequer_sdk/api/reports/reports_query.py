from http import HTTPStatus
from typing import Any

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.problem import Problem
from ...models.query_request import QueryRequest
from ...models.query_result import QueryResult
from ...types import Response


def _get_kwargs(
    *,
    body: QueryRequest,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}

    _kwargs: dict[str, Any] = {
        "method": "post",
        "url": "/api/v1/query",
    }

    _kwargs["json"] = body.to_dict()

    headers["Content-Type"] = "application/json"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Problem | QueryResult | None:
    if response.status_code == 200:
        response_200 = QueryResult.from_dict(response.json())

        return response_200

    if response.status_code == 422:
        response_422 = Problem.from_dict(response.json())

        return response_422

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Response[Problem | QueryResult]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient,
    body: QueryRequest,
) -> Response[Problem | QueryResult]:
    """Aggregate postings: filter × group by × bucket (design §7). Read-only,
    despite being a POST (the filters don't fit a query string).

    Args:
        body (QueryRequest): Filters combine with AND; the values inside one filter with OR.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | QueryResult]
    """

    kwargs = _get_kwargs(
        body=body,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    *,
    client: AuthenticatedClient,
    body: QueryRequest,
) -> Problem | QueryResult | None:
    """Aggregate postings: filter × group by × bucket (design §7). Read-only,
    despite being a POST (the filters don't fit a query string).

    Args:
        body (QueryRequest): Filters combine with AND; the values inside one filter with OR.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | QueryResult
    """

    return sync_detailed(
        client=client,
        body=body,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    body: QueryRequest,
) -> Response[Problem | QueryResult]:
    """Aggregate postings: filter × group by × bucket (design §7). Read-only,
    despite being a POST (the filters don't fit a query string).

    Args:
        body (QueryRequest): Filters combine with AND; the values inside one filter with OR.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | QueryResult]
    """

    kwargs = _get_kwargs(
        body=body,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient,
    body: QueryRequest,
) -> Problem | QueryResult | None:
    """Aggregate postings: filter × group by × bucket (design §7). Read-only,
    despite being a POST (the filters don't fit a query string).

    Args:
        body (QueryRequest): Filters combine with AND; the values inside one filter with OR.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | QueryResult
    """

    return (
        await asyncio_detailed(
            client=client,
            body=body,
        )
    ).parsed
