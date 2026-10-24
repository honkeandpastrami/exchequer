from http import HTTPStatus
from typing import Any

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.problem import Problem
from ...models.refreshed import Refreshed
from ...types import UNSET, Response, Unset


def _get_kwargs(
    *,
    wait: int | Unset = UNSET,
) -> dict[str, Any]:

    params: dict[str, Any] = {}

    params["wait"] = wait

    params = {k: v for k, v in params.items() if v is not UNSET and v is not None}

    _kwargs: dict[str, Any] = {
        "method": "get",
        "url": "/api/v1/prices/refresh",
        "params": params,
    }

    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Problem | Refreshed | None:
    if response.status_code == 200:
        response_200 = Refreshed.from_dict(response.json())

        return response_200

    if response.status_code == 404:
        response_404 = Problem.from_dict(response.json())

        return response_404

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[Problem | Refreshed]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient,
    wait: int | Unset = UNSET,
) -> Response[Problem | Refreshed]:
    """The latest refresh since the server started: how it went, once it's
    finished or the wait is over.

    Args:
        wait (int | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | Refreshed]
    """

    kwargs = _get_kwargs(
        wait=wait,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    *,
    client: AuthenticatedClient,
    wait: int | Unset = UNSET,
) -> Problem | Refreshed | None:
    """The latest refresh since the server started: how it went, once it's
    finished or the wait is over.

    Args:
        wait (int | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | Refreshed
    """

    return sync_detailed(
        client=client,
        wait=wait,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    wait: int | Unset = UNSET,
) -> Response[Problem | Refreshed]:
    """The latest refresh since the server started: how it went, once it's
    finished or the wait is over.

    Args:
        wait (int | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | Refreshed]
    """

    kwargs = _get_kwargs(
        wait=wait,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient,
    wait: int | Unset = UNSET,
) -> Problem | Refreshed | None:
    """The latest refresh since the server started: how it went, once it's
    finished or the wait is over.

    Args:
        wait (int | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | Refreshed
    """

    return (
        await asyncio_detailed(
            client=client,
            wait=wait,
        )
    ).parsed
