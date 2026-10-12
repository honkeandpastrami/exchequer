from http import HTTPStatus
from typing import Any
from urllib.parse import quote
from uuid import UUID

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.committed import Committed
from ...models.problem import Problem
from ...types import Response


def _get_kwargs(
    id: UUID,
    *,
    if_match: str,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}
    headers["If-Match"] = if_match

    _kwargs: dict[str, Any] = {
        "method": "post",
        "url": "/api/v1/imports/{id}/commit".format(
            id=quote(str(id), safe=""),
        ),
    }

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Committed | Problem | None:
    if response.status_code == 200:
        response_200 = Committed.from_dict(response.json())

        return response_200

    if response.status_code == 404:
        response_404 = Problem.from_dict(response.json())

        return response_404

    if response.status_code == 409:
        response_409 = Problem.from_dict(response.json())

        return response_409

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


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[Committed | Problem]:
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
    if_match: str,
) -> Response[Committed | Problem]:
    """Commit the import: all or nothing. Every row's status is worked out
    again first; if any changed (the ledger moved on since), nothing is
    committed and the import comes back with a 409 to review again. Then each
    row's decision is carried out: new transactions go into one `file`
    import batch; transfer matches turn the matched transaction into a
    transfer; every row that's now in the ledger is recorded as imported, so
    the same row is a duplicate next time. Each statement's ledger balance
    becomes a balance check. The staged import is then gone.

    Args:
        id (UUID):
        if_match (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Committed | Problem]
    """

    kwargs = _get_kwargs(
        id=id,
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
    if_match: str,
) -> Committed | Problem | None:
    """Commit the import: all or nothing. Every row's status is worked out
    again first; if any changed (the ledger moved on since), nothing is
    committed and the import comes back with a 409 to review again. Then each
    row's decision is carried out: new transactions go into one `file`
    import batch; transfer matches turn the matched transaction into a
    transfer; every row that's now in the ledger is recorded as imported, so
    the same row is a duplicate next time. Each statement's ledger balance
    becomes a balance check. The staged import is then gone.

    Args:
        id (UUID):
        if_match (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Committed | Problem
    """

    return sync_detailed(
        id=id,
        client=client,
        if_match=if_match,
    ).parsed


async def asyncio_detailed(
    id: UUID,
    *,
    client: AuthenticatedClient,
    if_match: str,
) -> Response[Committed | Problem]:
    """Commit the import: all or nothing. Every row's status is worked out
    again first; if any changed (the ledger moved on since), nothing is
    committed and the import comes back with a 409 to review again. Then each
    row's decision is carried out: new transactions go into one `file`
    import batch; transfer matches turn the matched transaction into a
    transfer; every row that's now in the ledger is recorded as imported, so
    the same row is a duplicate next time. Each statement's ledger balance
    becomes a balance check. The staged import is then gone.

    Args:
        id (UUID):
        if_match (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Committed | Problem]
    """

    kwargs = _get_kwargs(
        id=id,
        if_match=if_match,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    id: UUID,
    *,
    client: AuthenticatedClient,
    if_match: str,
) -> Committed | Problem | None:
    """Commit the import: all or nothing. Every row's status is worked out
    again first; if any changed (the ledger moved on since), nothing is
    committed and the import comes back with a 409 to review again. Then each
    row's decision is carried out: new transactions go into one `file`
    import batch; transfer matches turn the matched transaction into a
    transfer; every row that's now in the ledger is recorded as imported, so
    the same row is a duplicate next time. Each statement's ledger balance
    becomes a balance check. The staged import is then gone.

    Args:
        id (UUID):
        if_match (str):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Committed | Problem
    """

    return (
        await asyncio_detailed(
            id=id,
            client=client,
            if_match=if_match,
        )
    ).parsed
