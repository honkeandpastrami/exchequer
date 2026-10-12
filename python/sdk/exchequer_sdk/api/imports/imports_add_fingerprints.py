from http import HTTPStatus
from typing import Any

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.import_fingerprints_added import ImportFingerprintsAdded
from ...models.new_import_fingerprints import NewImportFingerprints
from ...models.problem import Problem
from ...types import Response


def _get_kwargs(
    *,
    body: NewImportFingerprints,
) -> dict[str, Any]:
    headers: dict[str, Any] = {}

    _kwargs: dict[str, Any] = {
        "method": "post",
        "url": "/api/v1/import-fingerprints",
    }

    _kwargs["json"] = body.to_dict()

    headers["Content-Type"] = "application/json"

    _kwargs["headers"] = headers
    return _kwargs


def _parse_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> ImportFingerprintsAdded | Problem | None:
    if response.status_code == 200:
        response_200 = ImportFingerprintsAdded.from_dict(response.json())

        return response_200

    if response.status_code == 422:
        response_422 = Problem.from_dict(response.json())

        return response_422

    if client.raise_on_unexpected_status:
        raise errors.UnexpectedStatus(response.status_code, response.content)
    else:
        return None


def _build_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Response[ImportFingerprintsAdded | Problem]:
    return Response(
        status_code=HTTPStatus(response.status_code),
        content=response.content,
        headers=response.headers,
        parsed=_parse_response(client=client, response=response),
    )


def sync_detailed(
    *,
    client: AuthenticatedClient,
    body: NewImportFingerprints,
) -> Response[ImportFingerprintsAdded | Problem]:
    """Record that rows were already imported. Re-runnable: what's recorded
    stays as it is, and items recorded for another transaction are returned
    as conflicts.

    Args:
        body (NewImportFingerprints):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[ImportFingerprintsAdded | Problem]
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
    body: NewImportFingerprints,
) -> ImportFingerprintsAdded | Problem | None:
    """Record that rows were already imported. Re-runnable: what's recorded
    stays as it is, and items recorded for another transaction are returned
    as conflicts.

    Args:
        body (NewImportFingerprints):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        ImportFingerprintsAdded | Problem
    """

    return sync_detailed(
        client=client,
        body=body,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    body: NewImportFingerprints,
) -> Response[ImportFingerprintsAdded | Problem]:
    """Record that rows were already imported. Re-runnable: what's recorded
    stays as it is, and items recorded for another transaction are returned
    as conflicts.

    Args:
        body (NewImportFingerprints):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[ImportFingerprintsAdded | Problem]
    """

    kwargs = _get_kwargs(
        body=body,
    )

    response = await client.get_async_httpx_client().request(**kwargs)

    return _build_response(client=client, response=response)


async def asyncio(
    *,
    client: AuthenticatedClient,
    body: NewImportFingerprints,
) -> ImportFingerprintsAdded | Problem | None:
    """Record that rows were already imported. Re-runnable: what's recorded
    stays as it is, and items recorded for another transaction are returned
    as conflicts.

    Args:
        body (NewImportFingerprints):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        ImportFingerprintsAdded | Problem
    """

    return (
        await asyncio_detailed(
            client=client,
            body=body,
        )
    ).parsed
