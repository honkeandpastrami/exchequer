"""`exchequer`: the command line (design §16.2)."""

from __future__ import annotations

from pathlib import Path
from typing import Annotated

import typer

from exchequer_cli import config
from exchequer_cli.imports import ApiError, Importer
from exchequer_sdk import AuthenticatedClient

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


def main() -> None:
    app()
