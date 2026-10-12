from http import HTTPStatus
from typing import Any
from urllib.parse import quote
from uuid import UUID

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.page_staged_row import PageStagedRow
from ...models.problem import Problem
from ...models.row_status import RowStatus
from ...types import UNSET, Response, Unset


def _get_kwargs(
    id: UUID,
    *,
    statement: int | Unset = UNSET,
    status: RowStatus | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> dict[str, Any]:

    params: dict[str, Any] = {}

    params["statement"] = statement

    json_status: str | Unset = UNSET
    if not isinstance(status, Unset):
        json_status = status.value

    params["status"] = json_status

    params["limit"] = limit

    params["cursor"] = cursor

    params = {k: v for k, v in params.items() if v is not UNSET and v is not None}

    _kwargs: dict[str, Any] = {
        "method": "get",
        "url": "/api/v1/imports/{id}/rows".format(
            id=quote(str(id), safe=""),
        ),
        "params": params,
    }

    return _kwargs


def _parse_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> PageStagedRow | Problem | None:
    if response.status_code == 200:
        response_200 = PageStagedRow.from_dict(response.json())

        return response_200

    if response.status_code == 404:
        response_404 = Problem.from_dict(response.json())

        return response_404

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Response[PageStagedRow | Problem]:
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
    statement: int | Unset = UNSET,
    status: RowStatus | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> Response[PageStagedRow | Problem]:
    """The import's rows, in file order.

    Args:
        id (UUID):
        statement (int | Unset):
        status (RowStatus | Unset): What a row would become.
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[PageStagedRow | Problem]
    """

    kwargs = _get_kwargs(
        id=id,
        statement=statement,
        status=status,
        limit=limit,
        cursor=cursor,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    id: UUID,
    *,
    client: AuthenticatedClient,
    statement: int | Unset = UNSET,
    status: RowStatus | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> PageStagedRow | Problem | None:
    """The import's rows, in file order.

    Args:
        id (UUID):
        statement (int | Unset):
        status (RowStatus | Unset): What a row would become.
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        PageStagedRow | Problem
    """

    return sync_detailed(
        id=id,
        client=client,
        statement=statement,
        status=status,
        limit=limit,
        cursor=cursor,
    ).parsed


async def asyncio_detailed(
    id: UUID,
    *,
    client: AuthenticatedClient,
    statement: int | Unset = UNSET,
    status: RowStatus | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> Response[PageStagedRow | Problem]:
    """The import's rows, in file order.

    Args:
        id (UUID):
        statement (int | Unset):
        status (RowStatus | Unset): What a row would become.
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[PageStagedRow | Problem]
    """

    kwargs = _get_kwargs(
        id=id,
        statement=statement,
        status=status,
        limit=limit,
        cursor=cursor,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    id: UUID,
    *,
    client: AuthenticatedClient,
    statement: int | Unset = UNSET,
    status: RowStatus | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> PageStagedRow | Problem | None:
    """The import's rows, in file order.

    Args:
        id (UUID):
        statement (int | Unset):
        status (RowStatus | Unset): What a row would become.
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        PageStagedRow | Problem
    """

    return (
        await asyncio_detailed(
            id=id,
            client=client,
            statement=statement,
            status=status,
            limit=limit,
            cursor=cursor,
        )
    ).parsed
