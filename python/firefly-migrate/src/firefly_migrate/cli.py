"""`firefly-migrate COMMAND MAPPING` (design §11).

plan        read Firefly and print the report; writes nothing anywhere.
run         plan, then write everything into an empty Exchequer server.
check       compare Firefly with what Exchequer holds (the acceptance check).
settle      the settlement pass over a migrated ledger: household currency,
            payment links, stipends and funders, closed months; then the
            settlement check. Doesn't read Firefly.
settlement  only the settlement check (implied stipends, design §4.5).

Firefly is only ever read (see `firefly.py`).
"""

from __future__ import annotations

import argparse
import sys
from datetime import date
from pathlib import Path

from exchequer_sdk import AuthenticatedClient
from firefly_migrate import check, mapping, settle, transform
from firefly_migrate.firefly import FireflyReader
from firefly_migrate.report import render
from firefly_migrate.writer import WriteError, Writer


def _reader(m: mapping.Mapping) -> FireflyReader:
    token = mapping.read_token(m.firefly_token_file)
    return FireflyReader(m.firefly_api_base, token, timeout=m.request_timeout)


def _exchequer(m: mapping.Mapping) -> AuthenticatedClient:
    token = mapping.read_token(m.exchequer_token_file)
    return AuthenticatedClient(base_url=m.exchequer_url, token=token, timeout=None)


def _plan(m: mapping.Mapping, ff: FireflyReader) -> tuple[transform.Plan, list[dict]]:
    accounts = ff.accounts()
    # Everything from the first transaction ever: opening balances are sums.
    # Up to today, so what lies past the cutoff is counted in the report.
    groups = ff.transactions(date(1970, 1, 1), max(date.today(), m.through))
    return transform.plan(m, accounts, groups), groups


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="firefly-migrate", description=__doc__.split("\n")[0])
    parser.add_argument("command", choices=["plan", "run", "check", "settle", "settlement"])
    parser.add_argument("mapping", type=Path, help="the mapping file (TOML)")
    args = parser.parse_args(argv)

    try:
        m = mapping.load(args.mapping)
    except mapping.MappingError as e:
        print(e, file=sys.stderr)
        return 2

    if args.command in ("settle", "settlement"):
        return _settlement(m, apply=args.command == "settle")

    with _reader(m) as ff:
        p, groups = _plan(m, ff)
        print(render(p))
        if p.report.errors and args.command == "run":
            print("\nNot writing anything: fix the errors above first.", file=sys.stderr)
            return 1

        if args.command == "run":
            with _exchequer(m) as client:

                def progress(i: int, total: int) -> None:
                    if i % 100 == 0 or i == total:
                        print(f"  wrote {i}/{total} transactions", file=sys.stderr)

                try:
                    Writer(client, m).write(p, progress)
                except WriteError as e:
                    print(f"\nwrite failed: {e}", file=sys.stderr)
                    return 1
            print(f"\nWrote {len(p.transactions)} transactions.")

        if args.command in ("run", "check"):
            dates = check.check_dates(m)
            with _exchequer(m) as client:
                balances, totals = check.exchequer_view(client, dates)
            diffs = check.compare(check.firefly_balances(ff, m, dates, p.accounts), balances)
            cats = check.compare(check.firefly_category_totals(m, groups), totals)
            print(f"\n## Acceptance check ({len(dates)} dates)\n")
            print(f"Balance differences: {len(diffs)}")
            for d in diffs:
                print(f"  - {d}")
            print(f"Category total differences: {len(cats)}")
            for d in cats:
                print(f"  - {d}")
            print("Settlement comparison: `firefly-migrate settlement MAPPING`.")
            return 1 if diffs or cats else 0
    return 0


def _settlement(m: mapping.Mapping, *, apply: bool) -> int:
    with _exchequer(m) as client:
        if apply:
            try:
                out = settle.Settler(client, m).run(lambda line: print(line, file=sys.stderr))
            except settle.SettleError as e:
                print(f"settlement pass failed: {e}", file=sys.stderr)
                return 1
            print(
                f"Household currency {'set' if out.currency_set else 'already set'}; "
                f"{len(out.linked)} payments linked, {len(out.params_set)} months' parameters "
                f"set, {len(out.closed)} months closed.\n"
            )
        last = m.through.isoformat()[:7]
        try:
            rows = settle.history_rows(client, f"{m.year}-01", last)
        except settle.SettleError as e:
            print(f"settlement check failed: {e}", file=sys.stderr)
            return 1
        print(settle.render(rows, m.closed_through))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
