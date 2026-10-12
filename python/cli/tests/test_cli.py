"""The CLI's configuration and what it shows, without a server."""

import datetime
from pathlib import Path
from uuid import UUID

import pytest

from exchequer_cli import config
from exchequer_cli.imports import Names, decisions_for, row_line, statement_line
from exchequer_sdk.models import (
    Decision,
    LedgerBalance,
    RowCounts,
    RowStatus,
    SkippedLine,
    StagedRow,
    StagedRowRaw,
    StagedStatement,
)

CARD = UUID(int=1)


def test_config_from_env_or_file(tmp_path: Path):
    token = tmp_path / "token"
    token.write_text("  secret\n")
    c = config.load({"EXCHEQUER_URL": "https://x.example/", "EXCHEQUER_TOKEN_FILE": str(token)})
    assert (c.url, c.token()) == ("https://x.example", "secret")

    file = tmp_path / "config.toml"
    file.write_text(f'url = "https://y.example"\ntoken_file = "{token}"\n')
    assert config.load({}, file).url == "https://y.example"
    with pytest.raises(config.ConfigError):
        config.load({}, tmp_path / "missing.toml")
    token.write_text("")
    with pytest.raises(config.ConfigError, match="empty"):
        config.load({}, file).token()


def test_a_statement_shows_its_account_counts_and_balance():
    names = Names(accounts={CARD: "Alex · Card"})
    s = StagedStatement(
        commodity_id=UUID(int=9),
        counts=RowCounts(new=3, duplicate=1, near_duplicate=0, transfer_match=0, needs_account=0),
        position=0,
        skipped=[SkippedLine(line=4, reason="unreadable date")],
        account_id=CARD,
        account_suggested=False,
        number_last4="2222",
        ledger=LedgerBalance(amount="-50.00", date=datetime.date(2026, 3, 31)),
    )
    assert statement_line(s, names) == (
        "  …2222 → Alex · Card: 3 new, 1 duplicate; bank balance -50.00 on 2026-03-31; "
        "1 unreadable lines"
    )
    s.account_suggested = True
    assert statement_line(s, names).startswith("  …2222 → Alex · Card (suggested): ")
    s.account_id = None
    s.number_last4 = None
    assert statement_line(s, names).startswith("  no number → NO ACCOUNT YET")


def test_a_row_and_what_it_can_become():
    r = StagedRow(
        amount="-80.00",
        date=datetime.date(2026, 3, 2),
        decided=False,
        description="GROCER",
        position=0,
        raw=StagedRowRaw(),
        statement=0,
        status=RowStatus.NEAR_DUPLICATE,
        chosen=[],
        suggestions=[],
        missing=[],
    )
    assert row_line(r, "2026-03-01 'Groceries'") == (
        "2026-03-02        -80.00  GROCER" + " " * 34 + "  near duplicate of 2026-03-01 'Groceries'"
    )
    assert decisions_for(RowStatus.NEW) == [Decision.NEW, Decision.SKIP]
    assert decisions_for(RowStatus.DUPLICATE)[0] == Decision.MATCH
    assert Decision.NEW not in decisions_for(RowStatus.DUPLICATE)
    assert decisions_for(RowStatus.TRANSFER_MATCH)[0] == Decision.MATCH


def test_accounts_are_found_by_full_or_short_name():
    names = Names(accounts={CARD: "Alex · Card", UUID(int=2): "Sam · Card"})
    assert names.find("Alex · Card") == CARD
    with pytest.raises(Exception, match="no single account"):
        names.find("Card")
