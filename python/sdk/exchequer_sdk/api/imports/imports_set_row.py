from http import HTTPStatus
from typing import Any
from urllib.parse import quote
from uuid import UUID

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.problem import Problem
from ...models.row_update import RowUpdate
from ...models.staged_row import StagedRow
from ...types import Response


def _get_kwargs(
    id: UUID,
    statement: int,
    position: int,
    *,
    body: RowUpdate,
    if_match: str,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}
    headers["If-Match"] = if_match

    _kwargs: dict[str, Any] = {
        "method": "patch",
        "url": "/api/v1/imports/{id}/rows/{statement}/{position}".format(
            id=quote(str(id), safe=""),
            statement=quote(str(statement), safe=""),
            position=quote(str(position), safe=""),
        ),
    }

    _kwargs["json"] = body.to_dict()

    headers["Content-Type"] = "application/json"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Problem | StagedRow | None:
    if response.status_code == 200:
        response_200 = StagedRow.from_dict(response.json())

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


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[Problem | StagedRow]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    id: UUID,
    statement: int,
    position: int,
    *,
    client: AuthenticatedClient,
    body: RowUpdate,
    if_match: str,
) -> Response[Problem | StagedRow]:
    """Decide what committing does with a row, and label the transaction it
    will become.

    Args:
        id (UUID):
        statement (int):
        position (int):
        if_match (str):
        body (RowUpdate): Fields to change; omitted ones stay as they are, `null` clears one.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | StagedRow]
    """

    kwargs = _get_kwargs(
        id=id,
        statement=statement,
        position=position,
        body=body,
        if_match=if_match,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    id: UUID,
    statement: int,
    position: int,
    *,
    client: AuthenticatedClient,
    body: RowUpdate,
    if_match: str,
) -> Problem | StagedRow | None:
    """Decide what committing does with a row, and label the transaction it
    will become.

    Args:
        id (UUID):
        statement (int):
        position (int):
        if_match (str):
        body (RowUpdate): Fields to change; omitted ones stay as they are, `null` clears one.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | StagedRow
    """

    return sync_detailed(
        id=id,
        statement=statement,
        position=position,
        client=client,
        body=body,
        if_match=if_match,
    ).parsed


async def asyncio_detailed(
    id: UUID,
    statement: int,
    position: int,
    *,
    client: AuthenticatedClient,
    body: RowUpdate,
    if_match: str,
) -> Response[Problem | StagedRow]:
    """Decide what committing does with a row, and label the transaction it
    will become.

    Args:
        id (UUID):
        statement (int):
        position (int):
        if_match (str):
        body (RowUpdate): Fields to change; omitted ones stay as they are, `null` clears one.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | StagedRow]
    """

    kwargs = _get_kwargs(
        id=id,
        statement=statement,
        position=position,
        body=body,
        if_match=if_match,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    id: UUID,
    statement: int,
    position: int,
    *,
    client: AuthenticatedClient,
    body: RowUpdate,
    if_match: str,
) -> Problem | StagedRow | None:
    """Decide what committing does with a row, and label the transaction it
    will become.

    Args:
        id (UUID):
        statement (int):
        position (int):
        if_match (str):
        body (RowUpdate): Fields to change; omitted ones stay as they are, `null` clears one.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | StagedRow
    """

    return (
        await asyncio_detailed(
            id=id,
            statement=statement,
            position=position,
            client=client,
            body=body,
            if_match=if_match,
        )
    ).parsed
