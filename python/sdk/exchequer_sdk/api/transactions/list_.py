import datetime
from http import HTTPStatus
from typing import Any
from uuid import UUID

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.page_transaction import PageTransaction
from ...types import UNSET, Response, Unset


def _get_kwargs(
    *,
    from_: datetime.date | Unset = UNSET,
    to: datetime.date | Unset = UNSET,
    account_id: UUID | Unset = UNSET,
    counterparty_id: UUID | Unset = UNSET,
    category_id: UUID | Unset = UNSET,
    external_source: str | Unset = UNSET,
    external_ref: str | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> dict[str, Any]:

    params: dict[str, Any] = {}

    json_from_: str | Unset = UNSET
    if not isinstance(from_, Unset):
        json_from_ = from_.isoformat()
    params["from"] = json_from_

    json_to: str | Unset = UNSET
    if not isinstance(to, Unset):
        json_to = to.isoformat()
    params["to"] = json_to

    json_account_id: str | Unset = UNSET
    if not isinstance(account_id, Unset):
        json_account_id = str(account_id)
    params["account_id"] = json_account_id

    json_counterparty_id: str | Unset = UNSET
    if not isinstance(counterparty_id, Unset):
        json_counterparty_id = str(counterparty_id)
    params["counterparty_id"] = json_counterparty_id

    json_category_id: str | Unset = UNSET
    if not isinstance(category_id, Unset):
        json_category_id = str(category_id)
    params["category_id"] = json_category_id

    params["external_source"] = external_source

    params["external_ref"] = external_ref

    params["limit"] = limit

    params["cursor"] = cursor

    params = {k: v for k, v in params.items() if v is not UNSET and v is not None}

    _kwargs: dict[str, Any] = {
        "method": "get",
        "url": "/api/v1/transactions",
        "params": params,
    }

    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> PageTransaction | None:
    if response.status_code == 200:
        response_200 = PageTransaction.from_dict(response.json())

        return response_200

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[PageTransaction]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient,
    from_: datetime.date | Unset = UNSET,
    to: datetime.date | Unset = UNSET,
    account_id: UUID | Unset = UNSET,
    counterparty_id: UUID | Unset = UNSET,
    category_id: UUID | Unset = UNSET,
    external_source: str | Unset = UNSET,
    external_ref: str | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> Response[PageTransaction]:
    """List transactions, newest first.

    Args:
        from_ (datetime.date | Unset):
        to (datetime.date | Unset):
        account_id (UUID | Unset):
        counterparty_id (UUID | Unset):
        category_id (UUID | Unset):
        external_source (str | Unset):
        external_ref (str | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[PageTransaction]
    """

    kwargs = _get_kwargs(
        from_=from_,
        to=to,
        account_id=account_id,
        counterparty_id=counterparty_id,
        category_id=category_id,
        external_source=external_source,
        external_ref=external_ref,
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
    from_: datetime.date | Unset = UNSET,
    to: datetime.date | Unset = UNSET,
    account_id: UUID | Unset = UNSET,
    counterparty_id: UUID | Unset = UNSET,
    category_id: UUID | Unset = UNSET,
    external_source: str | Unset = UNSET,
    external_ref: str | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> PageTransaction | None:
    """List transactions, newest first.

    Args:
        from_ (datetime.date | Unset):
        to (datetime.date | Unset):
        account_id (UUID | Unset):
        counterparty_id (UUID | Unset):
        category_id (UUID | Unset):
        external_source (str | Unset):
        external_ref (str | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        PageTransaction
    """

    return sync_detailed(
        client=client,
        from_=from_,
        to=to,
        account_id=account_id,
        counterparty_id=counterparty_id,
        category_id=category_id,
        external_source=external_source,
        external_ref=external_ref,
        limit=limit,
        cursor=cursor,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    from_: datetime.date | Unset = UNSET,
    to: datetime.date | Unset = UNSET,
    account_id: UUID | Unset = UNSET,
    counterparty_id: UUID | Unset = UNSET,
    category_id: UUID | Unset = UNSET,
    external_source: str | Unset = UNSET,
    external_ref: str | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> Response[PageTransaction]:
    """List transactions, newest first.

    Args:
        from_ (datetime.date | Unset):
        to (datetime.date | Unset):
        account_id (UUID | Unset):
        counterparty_id (UUID | Unset):
        category_id (UUID | Unset):
        external_source (str | Unset):
        external_ref (str | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[PageTransaction]
    """

    kwargs = _get_kwargs(
        from_=from_,
        to=to,
        account_id=account_id,
        counterparty_id=counterparty_id,
        category_id=category_id,
        external_source=external_source,
        external_ref=external_ref,
        limit=limit,
        cursor=cursor,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient,
    from_: datetime.date | Unset = UNSET,
    to: datetime.date | Unset = UNSET,
    account_id: UUID | Unset = UNSET,
    counterparty_id: UUID | Unset = UNSET,
    category_id: UUID | Unset = UNSET,
    external_source: str | Unset = UNSET,
    external_ref: str | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> PageTransaction | None:
    """List transactions, newest first.

    Args:
        from_ (datetime.date | Unset):
        to (datetime.date | Unset):
        account_id (UUID | Unset):
        counterparty_id (UUID | Unset):
        category_id (UUID | Unset):
        external_source (str | Unset):
        external_ref (str | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        PageTransaction
    """

    return (
        await asyncio_detailed(
            client=client,
            from_=from_,
            to=to,
            account_id=account_id,
            counterparty_id=counterparty_id,
            category_id=category_id,
            external_source=external_source,
            external_ref=external_ref,
            limit=limit,
            cursor=cursor,
        )
    ).parsed
