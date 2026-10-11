from http import HTTPStatus
from typing import Any
from urllib.parse import quote

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.problem import Problem
from ...models.statement import Statement
from ...types import Response


def _get_kwargs(
    month: str,
) -> dict[str, Any]:

    _kwargs: dict[str, Any] = {
        "method": "get",
        "url": "/api/v1/settlements/{month}/statement".format(
            month=quote(str(month), safe=""),
        ),
    }

    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Problem | Statement | None:
    if response.status_code == 200:
        response_200 = Statement.from_dict(response.json())

        return response_200

    if response.status_code == 409:
        response_409 = Problem.from_dict(response.json())

        return response_409

    if response.status_code == 422:
        response_422 = Problem.from_dict(response.json())

        return response_422

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[Problem | Statement]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    month: str,
    *,
    client: AuthenticatedClient,
) -> Response[Problem | Statement]:
    """The month's statement: each edge's claims, stipend, due, paid,
    outstanding and lines (design §4.3). Always computed from current data.

    Args:
        month (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | Statement]
    """

    kwargs = _get_kwargs(
        month=month,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    month: str,
    *,
    client: AuthenticatedClient,
) -> Problem | Statement | None:
    """The month's statement: each edge's claims, stipend, due, paid,
    outstanding and lines (design §4.3). Always computed from current data.

    Args:
        month (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | Statement
    """

    return sync_detailed(
        month=month,
        client=client,
    ).parsed


async def asyncio_detailed(
    month: str,
    *,
    client: AuthenticatedClient,
) -> Response[Problem | Statement]:
    """The month's statement: each edge's claims, stipend, due, paid,
    outstanding and lines (design §4.3). Always computed from current data.

    Args:
        month (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | Statement]
    """

    kwargs = _get_kwargs(
        month=month,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    month: str,
    *,
    client: AuthenticatedClient,
) -> Problem | Statement | None:
    """The month's statement: each edge's claims, stipend, due, paid,
    outstanding and lines (design §4.3). Always computed from current data.

    Args:
        month (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | Statement
    """

    return (
        await asyncio_detailed(
            month=month,
            client=client,
        )
    ).parsed
