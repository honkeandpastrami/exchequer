from http import HTTPStatus
from typing import Any
from urllib.parse import quote
from uuid import UUID

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.import_ import Import
from ...models.problem import Problem
from ...models.review import Review
from ...types import Response


def _get_kwargs(
    id: UUID,
    *,
    body: Review,
    if_match: str,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}
    headers["If-Match"] = if_match

    _kwargs: dict[str, Any] = {
        "method": "post",
        "url": "/api/v1/imports/{id}/review".format(
            id=quote(str(id), safe=""),
        ),
    }

    _kwargs["json"] = body.to_dict()

    headers["Content-Type"] = "application/json"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Import | Problem | None:
    if response.status_code == 200:
        response_200 = Import.from_dict(response.json())

        return response_200

    if response.status_code == 404:
        response_404 = Problem.from_dict(response.json())

        return response_404

    if response.status_code == 412:
        response_412 = Problem.from_dict(response.json())

        return response_412

    if response.status_code == 422:
        response_422 = Problem.from_dict(response.json())

        return response_422

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[Import | Problem]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    id: UUID,
    *,
    client: AuthenticatedClient,
    body: Review,
    if_match: str,
) -> Response[Import | Problem]:
    """Tick rows as reviewed, or untick them (design §6.7): what a ticked row
    takes counts as yours, so an import whose rows you ticked is ready to
    commit. A row that needs you, or isn't added, isn't ticked. Each tick
    goes by itself when its row changes under it.

    Args:
        id (UUID):
        if_match (str):
        body (Review): Rows to tick as reviewed, or untick.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Import | Problem]
    """

    kwargs = _get_kwargs(
        id=id,
        body=body,
        if_match=if_match,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    id: UUID,
    *,
    client: AuthenticatedClient,
    body: Review,
    if_match: str,
) -> Import | Problem | None:
    """Tick rows as reviewed, or untick them (design §6.7): what a ticked row
    takes counts as yours, so an import whose rows you ticked is ready to
    commit. A row that needs you, or isn't added, isn't ticked. Each tick
    goes by itself when its row changes under it.

    Args:
        id (UUID):
        if_match (str):
        body (Review): Rows to tick as reviewed, or untick.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Import | Problem
    """

    return sync_detailed(
        id=id,
        client=client,
        body=body,
        if_match=if_match,
    ).parsed


async def asyncio_detailed(
    id: UUID,
    *,
    client: AuthenticatedClient,
    body: Review,
    if_match: str,
) -> Response[Import | Problem]:
    """Tick rows as reviewed, or untick them (design §6.7): what a ticked row
    takes counts as yours, so an import whose rows you ticked is ready to
    commit. A row that needs you, or isn't added, isn't ticked. Each tick
    goes by itself when its row changes under it.

    Args:
        id (UUID):
        if_match (str):
        body (Review): Rows to tick as reviewed, or untick.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Import | Problem]
    """

    kwargs = _get_kwargs(
        id=id,
        body=body,
        if_match=if_match,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    id: UUID,
    *,
    client: AuthenticatedClient,
    body: Review,
    if_match: str,
) -> Import | Problem | None:
    """Tick rows as reviewed, or untick them (design §6.7): what a ticked row
    takes counts as yours, so an import whose rows you ticked is ready to
    commit. A row that needs you, or isn't added, isn't ticked. Each tick
    goes by itself when its row changes under it.

    Args:
        id (UUID):
        if_match (str):
        body (Review): Rows to tick as reviewed, or untick.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Import | Problem
    """

    return (
        await asyncio_detailed(
            id=id,
            client=client,
            body=body,
            if_match=if_match,
        )
    ).parsed
