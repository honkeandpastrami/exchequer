"""`exchequer`: the command line (design §16.2)."""

from __future__ import annotations

import datetime
import socket
import webbrowser
from collections.abc import Callable
from importlib.metadata import version
from pathlib import Path
from typing import Annotated, Any

import typer

from exchequer_cli import config, tokens
from exchequer_cli.imports import ApiError, Importer
from exchequer_cli.login import login as sign_in
from exchequer_cli.prices import refresh
from exchequer_cli.rules import Rules, error_lines, month_range
from exchequer_sdk import AuthenticatedClient, Client
from exchequer_sdk.models import RefreshRequest, RuleScript
from exchequer_sdk.types import UNSET

app = typer.Typer(no_args_is_help=True, add_completion=False)

# What the server's sessions show this CLI as (`exchequer sessions list`);
# the SDK adds the host.
AGENT = {"User-Agent": f"exchequer-cli/{version('exchequer-cli')}"}


def _client(cfg: config.Config, **more: Any) -> AuthenticatedClient:
    return AuthenticatedClient(base_url=cfg.url, token=cfg.token(), headers=AGENT, **more)


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


def _show_version(show: bool) -> None:
    if show:
        typer.echo(version("exchequer-cli"))
        raise typer.Exit()


@app.callback()
def main_callback(
    show_version: Annotated[
        bool,
        typer.Option(
            "--version", callback=_show_version, is_eager=True, help="Print the version and exit."
        ),
    ] = False,
) -> None:
    """Exchequer, from the command line."""


@app.command()
def login(
    name: Annotated[
        str | None,
        typer.Option(help="What the approver sees; default `cli on <host>`."),
    ] = None,
    days: Annotated[int, typer.Option(help="How many days the token works, 1-365.")] = 90,
) -> None:
    """Sign in through the web app: approve a code, and the token file is written."""
    try:
        cfg = config.load()
        with Client(base_url=cfg.url, headers=AGENT) as client:
            code = sign_in(
                client,
                cfg.url,
                cfg.token_file,
                name=name or f"cli on {socket.gethostname().split('.')[0]}",
                days=days,
                say=typer.echo,
                open_url=webbrowser.open,
            )
    except (config.ConfigError, ApiError) as e:
        typer.echo(f"error: {e}", err=True)
        raise typer.Exit(2) from e
    raise typer.Exit(code)


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
        client = _client(cfg, timeout=None)
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
        client = _client(cfg, timeout=None)
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


prices_app = typer.Typer(no_args_is_help=True, help="Prices (design §3.4).")
app.add_typer(prices_app, name="prices")


@prices_app.command("refresh")
def prices_refresh_(
    start: Annotated[str | None, typer.Option("--from", help="First day, YYYY-MM-DD.")] = None,
    end: Annotated[
        str | None, typer.Option("--to", help="Last day, YYYY-MM-DD; default today.")
    ] = None,
    value_in: Annotated[str, typer.Option(help="Prices in this commodity.")] = "USD",
) -> None:
    """Fetch every commodity's prices again, from the configured sources in order."""
    try:
        cfg = config.load()
        client = _client(cfg, timeout=None)
        body = RefreshRequest(
            value_in=value_in,
            from_=datetime.date.fromisoformat(start) if start else UNSET,
            to=datetime.date.fromisoformat(end) if end else UNSET,
        )
        with client:
            r = refresh(client, body)
    except (config.ConfigError, ApiError, ValueError) as e:
        typer.echo(f"error: {e}", err=True)
        raise typer.Exit(2) from e
    sources = ", ".join(r.sources) or "none configured"
    typer.echo(f"{r.stored} prices stored (sources: {sources})")
    for e in r.errors:
        typer.echo(f"  {e}")
    raise typer.Exit(1 if r.errors and not r.stored else 0)


tokens_app = typer.Typer(
    no_args_is_help=True,
    help="Scripts' tokens (design §10). Browsers and CLIs are sessions.",
)
app.add_typer(tokens_app, name="tokens")
sessions_app = typer.Typer(
    no_args_is_help=True,
    help="Sessions: browsers, CLIs and scripts using tokens (design §10).",
)
app.add_typer(sessions_app, name="sessions")


def _tokens(run: Callable[[AuthenticatedClient], int]) -> None:
    try:
        cfg = config.load()
        with _client(cfg) as client:
            code = run(client)
    except (config.ConfigError, ApiError) as e:
        typer.echo(f"error: {e}", err=True)
        raise typer.Exit(2) from e
    raise typer.Exit(code)


@tokens_app.command("list")
def tokens_list(
    everything: Annotated[
        bool, typer.Option("--all", help="Revoked and expired ones too.")
    ] = False,
) -> None:
    """List scripts' tokens still working: whose, created, last used, expires."""
    _tokens(lambda c: _print(tokens.table(c, everything=everything)))


@tokens_app.command("create")
def tokens_create(
    name: Annotated[str, typer.Argument(help="What uses it, e.g. `backup script`.")],
    days: Annotated[int, typer.Option(help="How many days it works, 1-365.")],
) -> None:
    """Make a token for a script; it's yours. Prints it, once, on stdout."""

    def run(c: AuthenticatedClient) -> int:
        t = tokens.create(c, name, days)
        typer.echo(f"Expires {t.expires_at.astimezone():%Y-%m-%d %H:%M}; shown once:", err=True)
        typer.echo(t.token)
        return 0

    _tokens(run)


@tokens_app.command("revoke")
def tokens_revoke(
    token: Annotated[str, typer.Argument(help="Its name (or ID).")],
) -> None:
    """Revoke a script's token: it stops working at once, and its sessions end."""

    def run(c: AuthenticatedClient) -> int:
        typer.echo(f"Revoked {tokens.revoke(c, token).name}.")
        return 0

    _tokens(run)


@sessions_app.command("list")
def sessions_list() -> None:
    """List sessions: whose, signed in with what, when, and from where."""
    _tokens(lambda c: _print(tokens.sessions(c)))


@sessions_app.command("sign-out")
def sessions_sign_out(
    session: Annotated[str, typer.Argument(help="Its ID, as `sessions list` shows it.")],
    yes: Annotated[bool, typer.Option("--yes", "-y", help="Don't ask.")] = False,
) -> None:
    """Sign a session out: its token is revoked."""

    def run(c: AuthenticatedClient) -> int:
        s = tokens.find_session(c, session)
        others = tokens.others_on_token(c, s)
        if others and not yes:
            also = ", ".join(tokens.short(o) for o in others)
            if not typer.confirm(f"Its token is shared: {also} sign out too. Go on?"):
                return 1
        tokens.sign_out(c, s)
        typer.echo(f"Signed out {tokens.short(s)}.")
        return 0

    _tokens(run)


@app.command("revoke-all")
def revoke_all(
    yes: Annotated[bool, typer.Option("--yes", "-y", help="Don't ask.")] = False,
) -> None:
    """The kill switch: sign everyone out and revoke all tokens, this CLI's too."""
    if not yes and not typer.confirm(
        "Sign everyone out and revoke all tokens? Every browser, CLI and script "
        "stops working, this one too; passkeys still sign in."
    ):
        raise typer.Exit(1)

    def run(c: AuthenticatedClient) -> int:
        n = tokens.revoke_all(c)
        typer.echo(f"Revoked {n} tokens. Run exchequer login to sign this CLI in again.")
        return 0

    _tokens(run)


def main() -> None:
    app()
