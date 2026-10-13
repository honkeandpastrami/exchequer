from http import HTTPStatus
from typing import Any

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.backtest import Backtest
from ...models.problem import Problem
from ...types import UNSET, Response, Unset


def _get_kwargs(
    *,
    from_: str,
    to: str | Unset = UNSET,
    method: str | Unset = UNSET,
    remote: bool | Unset = UNSET,
) -> dict[str, Any]:

    params: dict[str, Any] = {}

    params["from"] = from_

    params["to"] = to

    params["method"] = method

    params["remote"] = remote

    params = {k: v for k, v in params.items() if v is not UNSET and v is not None}

    _kwargs: dict[str, Any] = {
        "method": "get",
        "url": "/api/v1/suggesters/backtest",
        "params": params,
    }

    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Backtest | Problem | None:
    if response.status_code == 200:
        response_200 = Backtest.from_dict(response.json())

        return response_200

    if response.status_code == 422:
        response_422 = Problem.from_dict(response.json())

        return response_422

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[Backtest | Problem]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient,
    from_: str,
    to: str | Unset = UNSET,
    method: str | Unset = UNSET,
    remote: bool | Unset = UNSET,
) -> Response[Backtest | Problem]:
    """Measure a local AI method, alone or with remote AI after it, on the
    ledger's own past (docs/ai.md). For each month, the method learns from
    everything before it, then suggests labels for the month's transactions
    (between one of your accounts and the outside) as if they were being
    imported, and an account for each account's transactions; remote AI, if
    asked, is shown what's before the month and asked about what the method
    left unsure. The guesses are compared with the ledger. Reads only;
    nothing is stored.

    Args:
        from_ (str):
        to (str | Unset):
        method (str | Unset):
        remote (bool | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Backtest | Problem]
    """

    kwargs = _get_kwargs(
        from_=from_,
        to=to,
        method=method,
        remote=remote,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    *,
    client: AuthenticatedClient,
    from_: str,
    to: str | Unset = UNSET,
    method: str | Unset = UNSET,
    remote: bool | Unset = UNSET,
) -> Backtest | Problem | None:
    """Measure a local AI method, alone or with remote AI after it, on the
    ledger's own past (docs/ai.md). For each month, the method learns from
    everything before it, then suggests labels for the month's transactions
    (between one of your accounts and the outside) as if they were being
    imported, and an account for each account's transactions; remote AI, if
    asked, is shown what's before the month and asked about what the method
    left unsure. The guesses are compared with the ledger. Reads only;
    nothing is stored.

    Args:
        from_ (str):
        to (str | Unset):
        method (str | Unset):
        remote (bool | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Backtest | Problem
    """

    return sync_detailed(
        client=client,
        from_=from_,
        to=to,
        method=method,
        remote=remote,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    from_: str,
    to: str | Unset = UNSET,
    method: str | Unset = UNSET,
    remote: bool | Unset = UNSET,
) -> Response[Backtest | Problem]:
    """Measure a local AI method, alone or with remote AI after it, on the
    ledger's own past (docs/ai.md). For each month, the method learns from
    everything before it, then suggests labels for the month's transactions
    (between one of your accounts and the outside) as if they were being
    imported, and an account for each account's transactions; remote AI, if
    asked, is shown what's before the month and asked about what the method
    left unsure. The guesses are compared with the ledger. Reads only;
    nothing is stored.

    Args:
        from_ (str):
        to (str | Unset):
        method (str | Unset):
        remote (bool | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Backtest | Problem]
    """

    kwargs = _get_kwargs(
        from_=from_,
        to=to,
        method=method,
        remote=remote,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient,
    from_: str,
    to: str | Unset = UNSET,
    method: str | Unset = UNSET,
    remote: bool | Unset = UNSET,
) -> Backtest | Problem | None:
    """Measure a local AI method, alone or with remote AI after it, on the
    ledger's own past (docs/ai.md). For each month, the method learns from
    everything before it, then suggests labels for the month's transactions
    (between one of your accounts and the outside) as if they were being
    imported, and an account for each account's transactions; remote AI, if
    asked, is shown what's before the month and asked about what the method
    left unsure. The guesses are compared with the ledger. Reads only;
    nothing is stored.

    Args:
        from_ (str):
        to (str | Unset):
        method (str | Unset):
        remote (bool | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Backtest | Problem
    """

    return (
        await asyncio_detailed(
            client=client,
            from_=from_,
            to=to,
            method=method,
            remote=remote,
        )
    ).parsed
