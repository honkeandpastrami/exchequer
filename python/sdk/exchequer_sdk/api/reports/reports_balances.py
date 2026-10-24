import datetime
from http import HTTPStatus
from typing import Any

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.balances import Balances
from ...models.problem import Problem
from ...types import UNSET, Response, Unset


def _get_kwargs(
    *,
    date: datetime.date,
    value_in: str | Unset = UNSET,
    price_date: datetime.date | Unset = UNSET,
) -> dict[str, Any]:

    params: dict[str, Any] = {}

    json_date = date.isoformat()
    params["date"] = json_date

    params["value_in"] = value_in

    json_price_date: str | Unset = UNSET
    if not isinstance(price_date, Unset):
        json_price_date = price_date.isoformat()
    params["price_date"] = json_price_date

    params = {k: v for k, v in params.items() if v is not UNSET and v is not None}

    _kwargs: dict[str, Any] = {
        "method": "get",
        "url": "/api/v1/reports/balances",
        "params": params,
    }

    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Balances | Problem | None:
    if response.status_code == 200:
        response_200 = Balances.from_dict(response.json())

        return response_200

    if response.status_code == 422:
        response_422 = Problem.from_dict(response.json())

        return response_422

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[Balances | Problem]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient,
    date: datetime.date,
    value_in: str | Unset = UNSET,
    price_date: datetime.date | Unset = UNSET,
) -> Response[Balances | Problem]:
    """Each account's balance in each commodity at the end of a day, and with
    `value_in`, its value at stored prices (missing ones fetched after it answers).

    Args:
        date (datetime.date):
        value_in (str | Unset):
        price_date (datetime.date | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Balances | Problem]
    """

    kwargs = _get_kwargs(
        date=date,
        value_in=value_in,
        price_date=price_date,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    *,
    client: AuthenticatedClient,
    date: datetime.date,
    value_in: str | Unset = UNSET,
    price_date: datetime.date | Unset = UNSET,
) -> Balances | Problem | None:
    """Each account's balance in each commodity at the end of a day, and with
    `value_in`, its value at stored prices (missing ones fetched after it answers).

    Args:
        date (datetime.date):
        value_in (str | Unset):
        price_date (datetime.date | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Balances | Problem
    """

    return sync_detailed(
        client=client,
        date=date,
        value_in=value_in,
        price_date=price_date,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    date: datetime.date,
    value_in: str | Unset = UNSET,
    price_date: datetime.date | Unset = UNSET,
) -> Response[Balances | Problem]:
    """Each account's balance in each commodity at the end of a day, and with
    `value_in`, its value at stored prices (missing ones fetched after it answers).

    Args:
        date (datetime.date):
        value_in (str | Unset):
        price_date (datetime.date | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Balances | Problem]
    """

    kwargs = _get_kwargs(
        date=date,
        value_in=value_in,
        price_date=price_date,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient,
    date: datetime.date,
    value_in: str | Unset = UNSET,
    price_date: datetime.date | Unset = UNSET,
) -> Balances | Problem | None:
    """Each account's balance in each commodity at the end of a day, and with
    `value_in`, its value at stored prices (missing ones fetched after it answers).

    Args:
        date (datetime.date):
        value_in (str | Unset):
        price_date (datetime.date | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Balances | Problem
    """

    return (
        await asyncio_detailed(
            client=client,
            date=date,
            value_in=value_in,
            price_date=price_date,
        )
    ).parsed
