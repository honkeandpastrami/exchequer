from http import HTTPStatus
from typing import Any, cast
from uuid import UUID

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.import_ import Import
from ...models.problem import Problem
from ...types import UNSET, File, Response, Unset


def _get_kwargs(
    *,
    body: File,
    account_id: UUID | Unset = UNSET,
    file_name: str | Unset = UNSET,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}

    params: dict[str, Any] = {}

    json_account_id: str | Unset = UNSET
    if not isinstance(account_id, Unset):
        json_account_id = str(account_id)
    params["account_id"] = json_account_id

    params["file_name"] = file_name

    params = {k: v for k, v in params.items() if v is not UNSET and v is not None}

    _kwargs: dict[str, Any] = {
        "method": "post",
        "url": "/api/v1/imports",
        "params": params,
    }

    _kwargs["content"] = body.payload
    headers["Content-Type"] = "application/octet-stream"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Any | Import | Problem | None:
    if response.status_code == 201:
        response_201 = Import.from_dict(response.json())

        return response_201

    if response.status_code == 413:
        response_413 = cast(Any, None)
        return response_413

    if response.status_code == 422:
        response_422 = Problem.from_dict(response.json())

        return response_422

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Response[Any | Import | Problem]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient,
    body: File,
    account_id: UUID | Unset = UNSET,
    file_name: str | Unset = UNSET,
) -> Response[Any | Import | Problem]:
    """Upload a bank file (OFX, QFX, QBO or CSV; the format is read from the
    content). Its rows are staged, each with what it would become, and
    suggestions are made: the account of a statement that has none, and every
    row's labels (docs/ai.md). Nothing enters the ledger until the import is
    committed.

    Args:
        account_id (UUID | Unset):
        file_name (str | Unset):
        body (File): A bank file's bytes, as is.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Any | Import | Problem]
    """

    kwargs = _get_kwargs(
        body=body,
        account_id=account_id,
        file_name=file_name,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    *,
    client: AuthenticatedClient,
    body: File,
    account_id: UUID | Unset = UNSET,
    file_name: str | Unset = UNSET,
) -> Any | Import | Problem | None:
    """Upload a bank file (OFX, QFX, QBO or CSV; the format is read from the
    content). Its rows are staged, each with what it would become, and
    suggestions are made: the account of a statement that has none, and every
    row's labels (docs/ai.md). Nothing enters the ledger until the import is
    committed.

    Args:
        account_id (UUID | Unset):
        file_name (str | Unset):
        body (File): A bank file's bytes, as is.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Any | Import | Problem
    """

    return sync_detailed(
        client=client,
        body=body,
        account_id=account_id,
        file_name=file_name,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    body: File,
    account_id: UUID | Unset = UNSET,
    file_name: str | Unset = UNSET,
) -> Response[Any | Import | Problem]:
    """Upload a bank file (OFX, QFX, QBO or CSV; the format is read from the
    content). Its rows are staged, each with what it would become, and
    suggestions are made: the account of a statement that has none, and every
    row's labels (docs/ai.md). Nothing enters the ledger until the import is
    committed.

    Args:
        account_id (UUID | Unset):
        file_name (str | Unset):
        body (File): A bank file's bytes, as is.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Any | Import | Problem]
    """

    kwargs = _get_kwargs(
        body=body,
        account_id=account_id,
        file_name=file_name,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient,
    body: File,
    account_id: UUID | Unset = UNSET,
    file_name: str | Unset = UNSET,
) -> Any | Import | Problem | None:
    """Upload a bank file (OFX, QFX, QBO or CSV; the format is read from the
    content). Its rows are staged, each with what it would become, and
    suggestions are made: the account of a statement that has none, and every
    row's labels (docs/ai.md). Nothing enters the ledger until the import is
    committed.

    Args:
        account_id (UUID | Unset):
        file_name (str | Unset):
        body (File): A bank file's bytes, as is.

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Any | Import | Problem
    """

    return (
        await asyncio_detailed(
            client=client,
            body=body,
            account_id=account_id,
            file_name=file_name,
        )
    ).parsed
