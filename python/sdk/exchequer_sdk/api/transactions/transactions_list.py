import datetime
from http import HTTPStatus
from typing import Any
from uuid import UUID

import httpx

from ... import errors
from ...client import AuthenticatedClient, Client
from ...models.kind_name import KindName
from ...models.line_kind import LineKind
from ...models.problem import Problem
from ...models.transaction_page import TransactionPage
from ...types import UNSET, Response, Unset


def _get_kwargs(
    *,
    from_: datetime.date | Unset = UNSET,
    to: datetime.date | Unset = UNSET,
    account_id: list[UUID] | Unset = UNSET,
    owner_id: list[UUID] | Unset = UNSET,
    purpose_id: list[str] | Unset = UNSET,
    category_id: list[str] | Unset = UNSET,
    counterparty_id: list[str] | Unset = UNSET,
    kind: list[KindName] | Unset = UNSET,
    commodity_id: list[UUID] | Unset = UNSET,
    tag_id: list[UUID] | Unset = UNSET,
    q: str | Unset = UNSET,
    settles: str | Unset = UNSET,
    party_id: UUID | Unset = UNSET,
    line: LineKind | Unset = UNSET,
    unpriced: bool | Unset = UNSET,
    id: list[UUID] | Unset = UNSET,
    external_source: str | Unset = UNSET,
    external_ref: str | Unset = UNSET,
    import_batch_id: UUID | Unset = UNSET,
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

    json_account_id: list[str] | Unset = UNSET
    if not isinstance(account_id, Unset):
        json_account_id = []
        for account_id_item_data in account_id:
            account_id_item = str(account_id_item_data)
            json_account_id.append(account_id_item)

    params["account_id"] = json_account_id

    json_owner_id: list[str] | Unset = UNSET
    if not isinstance(owner_id, Unset):
        json_owner_id = []
        for owner_id_item_data in owner_id:
            owner_id_item = str(owner_id_item_data)
            json_owner_id.append(owner_id_item)

    params["owner_id"] = json_owner_id

    json_purpose_id: list[str] | Unset = UNSET
    if not isinstance(purpose_id, Unset):
        json_purpose_id = purpose_id

    params["purpose_id"] = json_purpose_id

    json_category_id: list[str] | Unset = UNSET
    if not isinstance(category_id, Unset):
        json_category_id = category_id

    params["category_id"] = json_category_id

    json_counterparty_id: list[str] | Unset = UNSET
    if not isinstance(counterparty_id, Unset):
        json_counterparty_id = counterparty_id

    params["counterparty_id"] = json_counterparty_id

    json_kind: list[str] | Unset = UNSET
    if not isinstance(kind, Unset):
        json_kind = []
        for kind_item_data in kind:
            kind_item = kind_item_data.value
            json_kind.append(kind_item)

    params["kind"] = json_kind

    json_commodity_id: list[str] | Unset = UNSET
    if not isinstance(commodity_id, Unset):
        json_commodity_id = []
        for commodity_id_item_data in commodity_id:
            commodity_id_item = str(commodity_id_item_data)
            json_commodity_id.append(commodity_id_item)

    params["commodity_id"] = json_commodity_id

    json_tag_id: list[str] | Unset = UNSET
    if not isinstance(tag_id, Unset):
        json_tag_id = []
        for tag_id_item_data in tag_id:
            tag_id_item = str(tag_id_item_data)
            json_tag_id.append(tag_id_item)

    params["tag_id"] = json_tag_id

    params["q"] = q

    params["settles"] = settles

    json_party_id: str | Unset = UNSET
    if not isinstance(party_id, Unset):
        json_party_id = str(party_id)
    params["party_id"] = json_party_id

    json_line: str | Unset = UNSET
    if not isinstance(line, Unset):
        json_line = line.value

    params["line"] = json_line

    params["unpriced"] = unpriced

    json_id: list[str] | Unset = UNSET
    if not isinstance(id, Unset):
        json_id = []
        for id_item_data in id:
            id_item = str(id_item_data)
            json_id.append(id_item)

    params["id"] = json_id

    params["external_source"] = external_source

    params["external_ref"] = external_ref

    json_import_batch_id: str | Unset = UNSET
    if not isinstance(import_batch_id, Unset):
        json_import_batch_id = str(import_batch_id)
    params["import_batch_id"] = json_import_batch_id

    params["limit"] = limit

    params["cursor"] = cursor

    params = {k: v for k, v in params.items() if v is not UNSET and v is not None}

    _kwargs: dict[str, Any] = {
        "method": "get",
        "url": "/api/v1/transactions",
        "params": params,
    }

    return _kwargs


def _parse_response(
    *, client: AuthenticatedClient | Client, response: httpx.Response
) -> Problem | TransactionPage | None:
    if response.status_code == 200:
        response_200 = TransactionPage.from_dict(response.json())

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
) -> Response[Problem | TransactionPage]:
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
    account_id: list[UUID] | Unset = UNSET,
    owner_id: list[UUID] | Unset = UNSET,
    purpose_id: list[str] | Unset = UNSET,
    category_id: list[str] | Unset = UNSET,
    counterparty_id: list[str] | Unset = UNSET,
    kind: list[KindName] | Unset = UNSET,
    commodity_id: list[UUID] | Unset = UNSET,
    tag_id: list[UUID] | Unset = UNSET,
    q: str | Unset = UNSET,
    settles: str | Unset = UNSET,
    party_id: UUID | Unset = UNSET,
    line: LineKind | Unset = UNSET,
    unpriced: bool | Unset = UNSET,
    id: list[UUID] | Unset = UNSET,
    external_source: str | Unset = UNSET,
    external_ref: str | Unset = UNSET,
    import_batch_id: UUID | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> Response[Problem | TransactionPage]:
    """List the transactions with a split that passes the filter, newest first,
    each with what of it counts (`matched`), and the totals over every page.

    Args:
        from_ (datetime.date | Unset):
        to (datetime.date | Unset):
        account_id (list[UUID] | Unset):
        owner_id (list[UUID] | Unset):
        purpose_id (list[str] | Unset):
        category_id (list[str] | Unset):
        counterparty_id (list[str] | Unset):
        kind (list[KindName] | Unset):
        commodity_id (list[UUID] | Unset):
        tag_id (list[UUID] | Unset):
        q (str | Unset):
        settles (str | Unset):
        party_id (UUID | Unset):
        line (LineKind | Unset):
        unpriced (bool | Unset):
        id (list[UUID] | Unset):
        external_source (str | Unset):
        external_ref (str | Unset):
        import_batch_id (UUID | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | TransactionPage]
    """

    kwargs = _get_kwargs(
        from_=from_,
        to=to,
        account_id=account_id,
        owner_id=owner_id,
        purpose_id=purpose_id,
        category_id=category_id,
        counterparty_id=counterparty_id,
        kind=kind,
        commodity_id=commodity_id,
        tag_id=tag_id,
        q=q,
        settles=settles,
        party_id=party_id,
        line=line,
        unpriced=unpriced,
        id=id,
        external_source=external_source,
        external_ref=external_ref,
        import_batch_id=import_batch_id,
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
    account_id: list[UUID] | Unset = UNSET,
    owner_id: list[UUID] | Unset = UNSET,
    purpose_id: list[str] | Unset = UNSET,
    category_id: list[str] | Unset = UNSET,
    counterparty_id: list[str] | Unset = UNSET,
    kind: list[KindName] | Unset = UNSET,
    commodity_id: list[UUID] | Unset = UNSET,
    tag_id: list[UUID] | Unset = UNSET,
    q: str | Unset = UNSET,
    settles: str | Unset = UNSET,
    party_id: UUID | Unset = UNSET,
    line: LineKind | Unset = UNSET,
    unpriced: bool | Unset = UNSET,
    id: list[UUID] | Unset = UNSET,
    external_source: str | Unset = UNSET,
    external_ref: str | Unset = UNSET,
    import_batch_id: UUID | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> Problem | TransactionPage | None:
    """List the transactions with a split that passes the filter, newest first,
    each with what of it counts (`matched`), and the totals over every page.

    Args:
        from_ (datetime.date | Unset):
        to (datetime.date | Unset):
        account_id (list[UUID] | Unset):
        owner_id (list[UUID] | Unset):
        purpose_id (list[str] | Unset):
        category_id (list[str] | Unset):
        counterparty_id (list[str] | Unset):
        kind (list[KindName] | Unset):
        commodity_id (list[UUID] | Unset):
        tag_id (list[UUID] | Unset):
        q (str | Unset):
        settles (str | Unset):
        party_id (UUID | Unset):
        line (LineKind | Unset):
        unpriced (bool | Unset):
        id (list[UUID] | Unset):
        external_source (str | Unset):
        external_ref (str | Unset):
        import_batch_id (UUID | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | TransactionPage
    """

    return sync_detailed(
        client=client,
        from_=from_,
        to=to,
        account_id=account_id,
        owner_id=owner_id,
        purpose_id=purpose_id,
        category_id=category_id,
        counterparty_id=counterparty_id,
        kind=kind,
        commodity_id=commodity_id,
        tag_id=tag_id,
        q=q,
        settles=settles,
        party_id=party_id,
        line=line,
        unpriced=unpriced,
        id=id,
        external_source=external_source,
        external_ref=external_ref,
        import_batch_id=import_batch_id,
        limit=limit,
        cursor=cursor,
    ).parsed


async def asyncio_detailed(
    *,
    client: AuthenticatedClient,
    from_: datetime.date | Unset = UNSET,
    to: datetime.date | Unset = UNSET,
    account_id: list[UUID] | Unset = UNSET,
    owner_id: list[UUID] | Unset = UNSET,
    purpose_id: list[str] | Unset = UNSET,
    category_id: list[str] | Unset = UNSET,
    counterparty_id: list[str] | Unset = UNSET,
    kind: list[KindName] | Unset = UNSET,
    commodity_id: list[UUID] | Unset = UNSET,
    tag_id: list[UUID] | Unset = UNSET,
    q: str | Unset = UNSET,
    settles: str | Unset = UNSET,
    party_id: UUID | Unset = UNSET,
    line: LineKind | Unset = UNSET,
    unpriced: bool | Unset = UNSET,
    id: list[UUID] | Unset = UNSET,
    external_source: str | Unset = UNSET,
    external_ref: str | Unset = UNSET,
    import_batch_id: UUID | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> Response[Problem | TransactionPage]:
    """List the transactions with a split that passes the filter, newest first,
    each with what of it counts (`matched`), and the totals over every page.

    Args:
        from_ (datetime.date | Unset):
        to (datetime.date | Unset):
        account_id (list[UUID] | Unset):
        owner_id (list[UUID] | Unset):
        purpose_id (list[str] | Unset):
        category_id (list[str] | Unset):
        counterparty_id (list[str] | Unset):
        kind (list[KindName] | Unset):
        commodity_id (list[UUID] | Unset):
        tag_id (list[UUID] | Unset):
        q (str | Unset):
        settles (str | Unset):
        party_id (UUID | Unset):
        line (LineKind | Unset):
        unpriced (bool | Unset):
        id (list[UUID] | Unset):
        external_source (str | Unset):
        external_ref (str | Unset):
        import_batch_id (UUID | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Response[Problem | TransactionPage]
    """

    kwargs = _get_kwargs(
        from_=from_,
        to=to,
        account_id=account_id,
        owner_id=owner_id,
        purpose_id=purpose_id,
        category_id=category_id,
        counterparty_id=counterparty_id,
        kind=kind,
        commodity_id=commodity_id,
        tag_id=tag_id,
        q=q,
        settles=settles,
        party_id=party_id,
        line=line,
        unpriced=unpriced,
        id=id,
        external_source=external_source,
        external_ref=external_ref,
        import_batch_id=import_batch_id,
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
    account_id: list[UUID] | Unset = UNSET,
    owner_id: list[UUID] | Unset = UNSET,
    purpose_id: list[str] | Unset = UNSET,
    category_id: list[str] | Unset = UNSET,
    counterparty_id: list[str] | Unset = UNSET,
    kind: list[KindName] | Unset = UNSET,
    commodity_id: list[UUID] | Unset = UNSET,
    tag_id: list[UUID] | Unset = UNSET,
    q: str | Unset = UNSET,
    settles: str | Unset = UNSET,
    party_id: UUID | Unset = UNSET,
    line: LineKind | Unset = UNSET,
    unpriced: bool | Unset = UNSET,
    id: list[UUID] | Unset = UNSET,
    external_source: str | Unset = UNSET,
    external_ref: str | Unset = UNSET,
    import_batch_id: UUID | Unset = UNSET,
    limit: int | Unset = UNSET,
    cursor: str | Unset = UNSET,
) -> Problem | TransactionPage | None:
    """List the transactions with a split that passes the filter, newest first,
    each with what of it counts (`matched`), and the totals over every page.

    Args:
        from_ (datetime.date | Unset):
        to (datetime.date | Unset):
        account_id (list[UUID] | Unset):
        owner_id (list[UUID] | Unset):
        purpose_id (list[str] | Unset):
        category_id (list[str] | Unset):
        counterparty_id (list[str] | Unset):
        kind (list[KindName] | Unset):
        commodity_id (list[UUID] | Unset):
        tag_id (list[UUID] | Unset):
        q (str | Unset):
        settles (str | Unset):
        party_id (UUID | Unset):
        line (LineKind | Unset):
        unpriced (bool | Unset):
        id (list[UUID] | Unset):
        external_source (str | Unset):
        external_ref (str | Unset):
        import_batch_id (UUID | Unset):
        limit (int | Unset):
        cursor (str | Unset):

    Raises:
        errors.UnexpectedStatus: If the server returns an undocumented status code and Client.raise_on_unexpected_status is True.
        httpx.TimeoutException: If the request takes longer than Client.timeout.

    Returns:
        Problem | TransactionPage
    """

    return (
        await asyncio_detailed(
            client=client,
            from_=from_,
            to=to,
            account_id=account_id,
            owner_id=owner_id,
            purpose_id=purpose_id,
            category_id=category_id,
            counterparty_id=counterparty_id,
            kind=kind,
            commodity_id=commodity_id,
            tag_id=tag_id,
            q=q,
            settles=settles,
            party_id=party_id,
            line=line,
            unpriced=unpriced,
            id=id,
            external_source=external_source,
            external_ref=external_ref,
            import_batch_id=import_batch_id,
            limit=limit,
            cursor=cursor,
        )
    ).parsed
