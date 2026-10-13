"""`exchequer`: the command line (design §16.2)."""

from __future__ import annotations

import datetime
from collections.abc import Callable
from pathlib import Path
from typing import Annotated

import typer

from exchequer_cli import config
from exchequer_cli.imports import ApiError, Importer
from exchequer_cli.rules import Rules, error_lines, month_range
from exchequer_sdk import AuthenticatedClient
from exchequer_sdk.models import RuleScript

app = typer.Typer(no_args_is_help=True, add_completion=False)


class TerminalUi:
    def say(self, text: str) -> None:
        typer.echo(text)

    def choose(self, prompt: str, options: list[str]) -> int | None:
        typer.echo(prompt)
        for i, o in enumerate(options, 1):
            typer.echo(f"    {i:>2}. {o}")
        typer.echo("     0. none of these (leave it staged)")
        while True:
            n = typer.prompt("  Number", type=int)
            if n == 0:
                return None
            if 1 <= n <= len(options):
                return n - 1

    def confirm(self, prompt: str, default: bool) -> bool:
        return typer.confirm(prompt, default=default)

    def key(self, prompt: str, keys: str, default: str) -> str:
        while True:
            k = typer.prompt(prompt, default=default, show_default=True).strip().lower()
            if len(k) == 1 and k in keys:
                return k


@app.callback()
def main_callback() -> None:
    """Exchequer, from the command line."""


@app.command("import")
def import_(
    files: Annotated[list[Path], typer.Argument(exists=True, dir_okay=False, readable=True)],
    account: Annotated[
        str | None,
        typer.Option(help="The account the files are for, as `Owner · Name`, if they don't say."),
    ] = None,
    yes: Annotated[
        bool, typer.Option("--yes", "-y", help="Take every default; ask nothing.")
    ] = False,
) -> None:
    """Upload bank files (OFX, QFX, QBO, CSV), review them, and commit."""
    try:
        cfg = config.load()
        client = AuthenticatedClient(base_url=cfg.url, token=cfg.token(), timeout=None)
        with client:
            code = Importer(client, TerminalUi(), yes=yes).run(files, account)
    except (config.ConfigError, ApiError) as e:
        typer.echo(f"error: {e}", err=True)
        raise typer.Exit(2) from e
    raise typer.Exit(code)


rules_app = typer.Typer(
    no_args_is_help=True, help="The rules script: suggestions you write (docs/ai.md)."
)
app.add_typer(rules_app, name="rules")

Note = Annotated[str | None, typer.Option("--note", "-m", help="Why, like a commit message.")]


def _rules(run: Callable[[Rules], int]) -> None:
    try:
        cfg = config.load()
        client = AuthenticatedClient(base_url=cfg.url, token=cfg.token(), timeout=None)
        with client:
            code = run(Rules(client, typer.echo))
    except (config.ConfigError, ApiError) as e:
        typer.echo(f"error: {e}", err=True)
        raise typer.Exit(2) from e
    raise typer.Exit(code)


def _print(lines: list[str]) -> int:
    for line in lines:
        typer.echo(line)
    return 0


@rules_app.command("show")
def rules_show(
    version: Annotated[
        int | None, typer.Argument(help="A version; the active one if omitted.")
    ] = None,
) -> None:
    """Print the script."""
    _rules(lambda r: _print([r.show(version)]))


@rules_app.command("edit")
def rules_edit(note: Note = None) -> None:
    """Edit the active script in $EDITOR; upload it on save. A refused script
    is reopened at its first error."""
    _rules(lambda r: r.edit(note))


@rules_app.command("upload")
def rules_upload(
    file: Annotated[Path, typer.Argument(exists=True, dir_okay=False, readable=True)],
    note: Note = None,
) -> None:
    """Upload a file as the new version."""

    def run(r: Rules) -> int:
        _, expected = r.active()
        result = r.upload(file.read_text(), note, expected)
        if isinstance(result, RuleScript):
            typer.echo(f"v{result.version} is active")
            return 0
        _print(["refused:", *error_lines(result)])
        return 1

    _rules(run)


@rules_app.command("log")
def rules_log() -> None:
    """List the versions, newest (the active one) first."""
    _rules(lambda r: _print(r.log()))


@rules_app.command("diff")
def rules_diff(
    a: Annotated[int, typer.Argument(help="The older version.")],
    b: Annotated[
        int | None, typer.Argument(help="The newer one; the active one if omitted.")
    ] = None,
) -> None:
    """Show what changed between two versions."""
    _rules(lambda r: _print(r.diff(a, b)))


@rules_app.command("rollback")
def rules_rollback(
    version: Annotated[int, typer.Argument(help="The version to go back to.")],
    note: Note = None,
) -> None:
    """Make an old version's script active again, as a new version."""
    _rules(lambda r: r.rollback(version, note))


@rules_app.command("dry-run")
def rules_dry_run(
    month: Annotated[
        str | None, typer.Option(help="A month, YYYY-MM (instead of --from and --to).")
    ] = None,
    start: Annotated[str | None, typer.Option("--from", help="First day, YYYY-MM-DD.")] = None,
    end: Annotated[str | None, typer.Option("--to", help="Last day, YYYY-MM-DD.")] = None,
    file: Annotated[
        Path | None,
        typer.Option(
            exists=True, dir_okay=False, help="A candidate script; the active one if omitted."
        ),
    ] = None,
) -> None:
    """Show what a script would label differently in the ledger's past."""

    def run(r: Rules) -> int:
        if month:
            first, last = month_range(month)
        elif start and end:
            first, last = datetime.date.fromisoformat(start), datetime.date.fromisoformat(end)
        else:
            raise ApiError(0, "give --month, or --from and --to")
        code, lines = r.dry_run(first, last, file.read_text() if file else None)
        _print(lines)
        return code

    _rules(run)


def main() -> None:
    app()
