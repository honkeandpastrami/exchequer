"""Adds the account numbers Firefly has to the accounts an earlier migration
created (`firefly-migrate numbers`). Accounts are found by owner and name;
numbers they already have are left alone. Only last fours are printed.
"""

from __future__ import annotations

from collections.abc import Callable

from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.api.accounts import accounts_get as get_account
from exchequer_sdk.api.accounts import accounts_list as list_accounts
from exchequer_sdk.api.parties import parties_list as list_parties
from firefly_migrate.check import _all
from firefly_migrate.mapping import account_key
from firefly_migrate.transform import masked
from firefly_migrate.writer import WriteError, add_numbers


def fill(
    client: AuthenticatedClient,
    wanted: dict[str, tuple[str, ...]],
    *,
    apply: bool,
    say: Callable[[str], None],
) -> int:
    """Returns how many numbers were added (or would be, without `apply`)."""
    parties = {
        str(p.id): p.name
        for p in _all(lambda c: list_parties.sync(client=client, limit=500, cursor=c))
    }
    accounts = {
        account_key(parties[str(a.owner_id)], a.name): a
        for a in _all(lambda c: list_accounts.sync(client=client, limit=500, cursor=c))
    }
    added = 0
    for key, numbers in sorted(wanted.items()):
        if not numbers:
            continue
        account = accounts.get(key)
        if account is None:
            say(f"- {key}: not in Exchequer, skipped")
            continue
        have = {n.last4 for n in account.numbers}
        new = [n for n in numbers if n[-4:] not in have]
        if not new:
            continue
        say(f"- {key}: {', '.join(masked(n) for n in new)}")
        added += len(new)
        if apply:
            r = get_account.sync_detailed(account.id, client=client)
            if r.status_code != 200:
                raise WriteError(f"account {key}: HTTP {r.status_code}")
            add_numbers(client, account.id, r.headers["etag"], new)
    return added
