from http import HTTPStatus
from typing import Any
from uuid import UUID

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.page_balance_check import PageBalanceCheck
from ...types import UNSET, Response, Unset


def _get_kwargs(
    *,
    account_id: UUID | Unset = UNSET,
    commodity_id: UUID | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> dict[str, Any]:

    params: dict[str, Any] = {}

    json_account_id: str | Unset = UNSET
    if not isinstance(account_id, Unset):
        json_account_id = str(account_id)
    params["account_id"] = json_account_id

    json_commodity_id: str | Unset = UNSET
    if not isinstance(commodity_id, Unset):
        json_commodity_id = str(commodity_id)
    params["commodity_id"] = json_commodity_id

    params["limit"] = limit

    params["cursor"] = cursor

    params = {k: v for k, v in params.items() if v is not UNSET and v is not None}

    _kwargs: dict[str, Any] = {
        "method": "get",
        "url": "/api/v1/balance-checks",
        "params": params,
    }

    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> PageBalanceCheck | None:
    if response.status_code == 200:
        response_200 = PageBalanceCheck.from_dict(response.json())

        return response_200

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[PageBalanceCheck]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient,
    account_id: UUID | Unset = UNSET,
    commodity_id: UUID | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> Response[PageBalanceCheck]:
    """List balance checks, newest first, each compared with the current ledger.

    Args:
        account_id (UUID | Unset):
        commodity_id (UUID | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[PageBalanceCheck]
    """

    kwargs = _get_kwargs(
        account_id=account_id,
        commodity_id=commodity_id,
        limit=limit,
        cursor=cursor,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    *,
    client: AuthenticatedClient,
    account_id: UUID | Unset = UNSET,
    commodity_id: UUID | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> PageBalanceCheck | None:
    """List balance checks, newest first, each compared with the current ledger.

    Args:
        account_id (UUID | Unset):
        commodity_id (UUID | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        PageBalanceCheck
    """

    return sync_detailed(
        client=client,
        account_id=account_id,
        commodity_id=commodity_id,
        limit=limit,
        cursor=cursor,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    account_id: UUID | Unset = UNSET,
    commodity_id: UUID | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> Response[PageBalanceCheck]:
    """List balance checks, newest first, each compared with the current ledger.

    Args:
        account_id (UUID | Unset):
        commodity_id (UUID | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[PageBalanceCheck]
    """

    kwargs = _get_kwargs(
        account_id=account_id,
        commodity_id=commodity_id,
        limit=limit,
        cursor=cursor,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient,
    account_id: UUID | Unset = UNSET,
    commodity_id: UUID | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> PageBalanceCheck | None:
    """List balance checks, newest first, each compared with the current ledger.

    Args:
        account_id (UUID | Unset):
        commodity_id (UUID | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        PageBalanceCheck
    """

    return (
        await asyncio_detailed(
            client=client,
            account_id=account_id,
            commodity_id=commodity_id,
            limit=limit,
            cursor=cursor,
        )
    ).parsed
