import datetime
from http import HTTPStatus
from typing import Any

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.dry_run import DryRun
from ...models.dry_run_request import DryRunRequest
from ...models.problem import Problem
from ...types import UNSET, Response


def _get_kwargs(
    *,
    body: DryRunRequest,
    from_: datetime.date,
    to: datetime.date,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}

    params: dict[str, Any] = {}

    json_from_ = from_.isoformat()
    params["from"] = json_from_

    json_to = to.isoformat()
    params["to"] = json_to

    params = {k: v for k, v in params.items() if v is not UNSET and v is not None}

    _kwargs: dict[str, Any] = {
        "method": "post",
        "url": "/api/v1/rules/dry-run",
        "params": params,
    }

    _kwargs["json"] = body.to_dict()

    headers["Content-Type"] = "application/json"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> DryRun | Problem | None:
    if response.status_code == 200:
        response_200 = DryRun.from_dict(response.json())

        return response_200

    if response.status_code == 422:
        response_422 = Problem.from_dict(response.json())

        return response_422

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(*, client: AuthenticatedClient | Client, response: httpx.Response) -> Response[DryRun | Problem]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient,
    body: DryRunRequest,
    from_: datetime.date,
    to: datetime.date,
) -> Response[DryRun | Problem]:
    """What a script would set for the ledger's transactions in a date range
    (those between one of your accounts and the outside, as an imported row
    becomes), compared with what they have. Runs the active script, or a
    candidate given in the body (validated first); stores nothing. The
    bank's own fields (`row.raw`) are empty here, and `fn account` isn't run.

    Args:
        from_ (datetime.date):
        to (datetime.date):
        body (DryRunRequest):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[DryRun | Problem]
    """

    kwargs = _get_kwargs(
        body=body,
        from_=from_,
        to=to,
    )

    response = client.get_httpx_client().request(
        **kwargs,
    )

    return _build_response(client=client, response=response)


def sync(
    *,
    client: AuthenticatedClient,
    body: DryRunRequest,
    from_: datetime.date,
    to: datetime.date,
) -> DryRun | Problem | None:
    """What a script would set for the ledger's transactions in a date range
    (those between one of your accounts and the outside, as an imported row
    becomes), compared with what they have. Runs the active script, or a
    candidate given in the body (validated first); stores nothing. The
    bank's own fields (`row.raw`) are empty here, and `fn account` isn't run.

    Args:
        from_ (datetime.date):
        to (datetime.date):
        body (DryRunRequest):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        DryRun | Problem
    """

    return sync_detailed(
        client=client,
        body=body,
        from_=from_,
        to=to,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    body: DryRunRequest,
    from_: datetime.date,
    to: datetime.date,
) -> Response[DryRun | Problem]:
    """What a script would set for the ledger's transactions in a date range
    (those between one of your accounts and the outside, as an imported row
    becomes), compared with what they have. Runs the active script, or a
    candidate given in the body (validated first); stores nothing. The
    bank's own fields (`row.raw`) are empty here, and `fn account` isn't run.

    Args:
        from_ (datetime.date):
        to (datetime.date):
        body (DryRunRequest):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[DryRun | Problem]
    """

    kwargs = _get_kwargs(
        body=body,
        from_=from_,
        to=to,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient,
    body: DryRunRequest,
    from_: datetime.date,
    to: datetime.date,
) -> DryRun | Problem | None:
    """What a script would set for the ledger's transactions in a date range
    (those between one of your accounts and the outside, as an imported row
    becomes), compared with what they have. Runs the active script, or a
    candidate given in the body (validated first); stores nothing. The
    bank's own fields (`row.raw`) are empty here, and `fn account` isn't run.

    Args:
        from_ (datetime.date):
        to (datetime.date):
        body (DryRunRequest):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        DryRun | Problem
    """

    return (
        await asyncio_detailed(
            client=client,
            body=body,
            from_=from_,
            to=to,
        )
    ).parsed
