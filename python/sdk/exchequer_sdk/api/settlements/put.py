from http import HTTPStatus
from typing import Any
from urllib.parse import quote

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.problem import Problem
from ...models.settlement import Settlement
from ...models.settlement_params import SettlementParams
from ...types import Response


def _get_kwargs(
    month: str,
    *,
    body: SettlementParams,
    if_match: str,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}
    headers["If-Match"] = if_match

    _kwargs: dict[str, Any] = {
        "method": "put",
        "url": "/api/v1/settlements/{month}".format(
            month=quote(str(month), safe=""),
        ),
    }

    _kwargs["json"] = body.to_dict()

    headers["Content-Type"] = "application/json"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Problem | Settlement | None:
    if response.status_code == 200:
        response_200 = Settlement.from_dict(response.json())

        return response_200

    if response.status_code == 409:
        response_409 = Problem.from_dict(response.json())

        return response_409

    if response.status_code == 412:
        response_412 = Problem.from_dict(response.json())

        return response_412

    if response.status_code == 422:
        response_422 = Problem.from_dict(response.json())

        return response_422

    if response.status_code == 428:
        response_428 = Problem.from_dict(response.json())

        return response_428

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Response[Problem | Settlement]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    month: str,
    *,
    client: AuthenticatedClient,
    body: SettlementParams,
    if_match: str,
) -> Response[Problem | Settlement]:
    """Set a month's stipends and funders. `If-Match` is the ETag from GET
    (`"0"` for a month never set).

    Args:
        month (str):
        if_match (str):
        body (SettlementParams):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | Settlement]
    """

    kwargs = _get_kwargs(
        month=month,
        body=body,
        if_match=if_match,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    month: str,
    *,
    client: AuthenticatedClient,
    body: SettlementParams,
    if_match: str,
) -> Problem | Settlement | None:
    """Set a month's stipends and funders. `If-Match` is the ETag from GET
    (`"0"` for a month never set).

    Args:
        month (str):
        if_match (str):
        body (SettlementParams):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | Settlement
    """

    return sync_detailed(
        month=month,
        client=client,
        body=body,
        if_match=if_match,
    ).parsed


async def asyncio_detailed(
    month: str,
    *,
    client: AuthenticatedClient,
    body: SettlementParams,
    if_match: str,
) -> Response[Problem | Settlement]:
    """Set a month's stipends and funders. `If-Match` is the ETag from GET
    (`"0"` for a month never set).

    Args:
        month (str):
        if_match (str):
        body (SettlementParams):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | Settlement]
    """

    kwargs = _get_kwargs(
        month=month,
        body=body,
        if_match=if_match,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    month: str,
    *,
    client: AuthenticatedClient,
    body: SettlementParams,
    if_match: str,
) -> Problem | Settlement | None:
    """Set a month's stipends and funders. `If-Match` is the ETag from GET
    (`"0"` for a month never set).

    Args:
        month (str):
        if_match (str):
        body (SettlementParams):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | Settlement
    """

    return (
        await asyncio_detailed(
            month=month,
            client=client,
            body=body,
            if_match=if_match,
        )
    ).parsed
