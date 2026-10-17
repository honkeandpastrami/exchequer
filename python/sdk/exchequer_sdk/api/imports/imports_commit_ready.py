from http import HTTPStatus
from typing import Any

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.commit_ready import CommitReady
from ...models.ready_committed import ReadyCommitted
from ...types import Response


def _get_kwargs(
    *,
    body: CommitReady,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}

    _kwargs: dict[str, Any] = {
        "method": "post",
        "url": "/api/v1/imports/commit-ready",
    }

    _kwargs["json"] = body.to_dict()

    headers["Content-Type"] = "application/json"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> ReadyCommitted | None:
    if response.status_code == 200:
        response_200 = ReadyCommitted.from_dict(response.json())

        return response_200

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[ReadyCommitted]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient,
    body: CommitReady,
) -> Response[ReadyCommitted]:
    """Commit the imports you saw as ready, oldest first, each on its own, as
    [`commit`] does. Each is checked again just before: one that's gone, no
    longer ready (committing another may have changed it), or changed since
    is skipped, as is one commit refuses; the others go ahead.

    Args:
        body (CommitReady):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[ReadyCommitted]
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
    body: CommitReady,
) -> ReadyCommitted | None:
    """Commit the imports you saw as ready, oldest first, each on its own, as
    [`commit`] does. Each is checked again just before: one that's gone, no
    longer ready (committing another may have changed it), or changed since
    is skipped, as is one commit refuses; the others go ahead.

    Args:
        body (CommitReady):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        ReadyCommitted
    """

    return sync_detailed(
        client=client,
        body=body,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    body: CommitReady,
) -> Response[ReadyCommitted]:
    """Commit the imports you saw as ready, oldest first, each on its own, as
    [`commit`] does. Each is checked again just before: one that's gone, no
    longer ready (committing another may have changed it), or changed since
    is skipped, as is one commit refuses; the others go ahead.

    Args:
        body (CommitReady):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[ReadyCommitted]
    """

    kwargs = _get_kwargs(
        body=body,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient,
    body: CommitReady,
) -> ReadyCommitted | None:
    """Commit the imports you saw as ready, oldest first, each on its own, as
    [`commit`] does. Each is checked again just before: one that's gone, no
    longer ready (committing another may have changed it), or changed since
    is skipped, as is one commit refuses; the others go ahead.

    Args:
        body (CommitReady):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        ReadyCommitted
    """

    return (
        await asyncio_detailed(
            client=client,
            body=body,
        )
    ).parsed
