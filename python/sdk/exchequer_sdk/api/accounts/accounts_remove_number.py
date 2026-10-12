from http import HTTPStatus
from typing import Any
from urllib.parse import quote
from uuid import UUID

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.account import Account
from ...models.problem import Problem
from ...types import Response


def _get_kwargs(
    id: UUID,
    number_id: UUID,
    *,
    if_match: str,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}
    headers["If-Match"] = if_match

    _kwargs: dict[str, Any] = {
        "method": "delete",
        "url": "/api/v1/accounts/{id}/numbers/{number_id}".format(
            id=quote(str(id), safe=""),
            number_id=quote(str(number_id), safe=""),
        ),
    }

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Account | Problem | None:
    if response.status_code == 200:
        response_200 = Account.from_dict(response.json())

        return response_200

    if response.status_code == 404:
        response_404 = Problem.from_dict(response.json())

        return response_404

    if response.status_code == 412:
        response_412 = Problem.from_dict(response.json())

        return response_412

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[Account | Problem]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    id: UUID,
    number_id: UUID,
    *,
    client: AuthenticatedClient,
    if_match: str,
) -> Response[Account | Problem]:
    """Remove one of the account's numbers.

    Args:
        id (UUID):
        number_id (UUID):
        if_match (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Account | Problem]
    """

    kwargs = _get_kwargs(
        id=id,
        number_id=number_id,
        if_match=if_match,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    id: UUID,
    number_id: UUID,
    *,
    client: AuthenticatedClient,
    if_match: str,
) -> Account | Problem | None:
    """Remove one of the account's numbers.

    Args:
        id (UUID):
        number_id (UUID):
        if_match (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Account | Problem
    """

    return sync_detailed(
        id=id,
        number_id=number_id,
        client=client,
        if_match=if_match,
    ).parsed


async def asyncio_detailed(
    id: UUID,
    number_id: UUID,
    *,
    client: AuthenticatedClient,
    if_match: str,
) -> Response[Account | Problem]:
    """Remove one of the account's numbers.

    Args:
        id (UUID):
        number_id (UUID):
        if_match (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Account | Problem]
    """

    kwargs = _get_kwargs(
        id=id,
        number_id=number_id,
        if_match=if_match,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    id: UUID,
    number_id: UUID,
    *,
    client: AuthenticatedClient,
    if_match: str,
) -> Account | Problem | None:
    """Remove one of the account's numbers.

    Args:
        id (UUID):
        number_id (UUID):
        if_match (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Account | Problem
    """

    return (
        await asyncio_detailed(
            id=id,
            number_id=number_id,
            client=client,
            if_match=if_match,
        )
    ).parsed
