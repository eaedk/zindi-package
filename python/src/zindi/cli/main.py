"""Command definitions for ``zindi``.

Every command that works on a competition accepts its id as an argument. Leave it out
after running ``zindi use <competition-id>`` in your project folder.
"""

from __future__ import annotations

import json
import os
import platform
import sys
from collections.abc import Iterator
from contextlib import contextmanager, suppress
from datetime import datetime
from pathlib import Path
from typing import Annotated, Any

import typer
from rich.console import Console

from .. import __version__, auth, config
from .._display import rich_table
from .._http import base_url_from_env
from ..client import Client
from ..errors import AuthenticationError, SecretCodeRequiredError, ZindiError
from ..models import ResultList, _Model

app = typer.Typer(
    name="zindi",
    help="Take part in [bold]Zindi[/] competitions from your terminal: browse, join, download, "
    "submit and follow the leaderboard.",
    no_args_is_help=True,
    rich_markup_mode="rich",
    context_settings={"help_option_names": ["-h", "--help"]},
    add_completion=True,
)
team_app = typer.Typer(help="Create a team, invite teammates, or disband it.", no_args_is_help=True)
app.add_typer(team_app, name="team")

out = Console()
err = Console(stderr=True)

# -- shared options ------------------------------------------------------------------------------

CompetitionArg = Annotated[
    str | None,
    typer.Argument(
        help="Competition id, e.g. [cyan]Crop-Yield-Forecast-Challenge[/]. "
        "Defaults to the one set with [bold]zindi use[/].",
        show_default=False,
    ),
]
CompetitionOpt = Annotated[
    str | None,
    typer.Option(
        "--competition",
        "-c",
        help="Competition id (defaults to [bold]zindi use[/]).",
        show_default=False,
    ),
]
JsonOpt = Annotated[bool, typer.Option("--json", help="Print machine-readable JSON.")]
LimitOpt = Annotated[
    int | None, typer.Option("--limit", "-n", help="Maximum number of rows.", min=1)
]


def _client() -> Client:
    return Client()


def _json_default(value: Any) -> Any:
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, _Model):
        return value.to_dict()
    return str(value)


def _print_json(data: Any) -> None:
    if isinstance(data, ResultList):
        data = data.to_records()
    elif isinstance(data, _Model):
        data = data.to_dict()
    out.print_json(json.dumps(data, default=_json_default, ensure_ascii=False))


def _emit(data: Any, as_json: bool, *, highlight: str | None = None) -> None:
    if as_json:
        _print_json(data)
    elif isinstance(data, ResultList):
        out.print(rich_table(data, highlight=highlight))
    else:
        out.print(data)


@contextmanager
def _errors() -> Iterator[None]:
    """Turn exceptions into friendly messages and exit codes."""
    try:
        yield
    except AuthenticationError as exc:
        err.print(f"[red]✘[/] {exc}")
        raise typer.Exit(code=2) from None
    except ZindiError as exc:
        err.print(f"[red]✘[/] {exc}")
        raise typer.Exit(code=1) from None
    except KeyboardInterrupt:
        err.print("\n[yellow]Interrupted.[/]")
        raise typer.Exit(code=130) from None


def _version_callback(value: bool) -> None:
    if value:
        out.print(f"zindi {__version__}")
        raise typer.Exit()


@app.callback()
def _root(
    version: Annotated[
        bool,
        typer.Option(
            "--version", "-V", callback=_version_callback, is_eager=True, help="Show the version."
        ),
    ] = False,
) -> None:
    """Zindi from the command line."""


# -- account -------------------------------------------------------------------------------------


@app.command()
def login(
    username: Annotated[
        str | None, typer.Option("--username", "-u", help="Your Zindi username.")
    ] = None,
    token: Annotated[
        str | None,
        typer.Option("--token", help="Save an auth token instead of signing in with a password."),
    ] = None,
    password_stdin: Annotated[
        bool,
        typer.Option(
            "--password-stdin", help="Read the password from standard input (for scripts)."
        ),
    ] = False,
) -> None:
    """Log in and save your auth token (never your password) for next time."""
    with _errors():
        if token:
            client = Client(token=token)
            user = client.me()
            path = config.save_credentials(user.username, token)
        else:
            password = sys.stdin.readline().rstrip("\r\n") if password_stdin else None
            client = Client(use_saved_credentials=False)
            user = client.login(username, password, save=True)
            path = config.credentials_path()
        out.print(f"[green]✔[/] Logged in as [bold]{user.username}[/]. Token saved to {path}")


@app.command()
def logout() -> None:
    """Delete the saved credentials."""
    removed = config.delete_credentials()
    out.print("[green]✔[/] Logged out." if removed else "You were not logged in.")


@app.command()
def whoami(as_json: JsonOpt = False) -> None:
    """Show who you are logged in as, and where the credentials come from."""
    with _errors():
        client = _client()
        if not client.is_authenticated:
            raise AuthenticationError("You are not logged in. Run `zindi login`.")
        user = client.me()
        if as_json:
            _print_json({**user.to_dict(), "credentials_source": client.credentials_source})
            return
        out.print(user)
        out.print(f"[dim]credentials: {client.credentials_source}[/]")


@app.command("user")
def user_cmd(username: str, as_json: JsonOpt = False) -> None:
    """Show the public profile of a Zindi user."""
    with _errors():
        _emit(_client().user(username), as_json)


# -- competitions --------------------------------------------------------------------------------


@app.command("competitions")
def competitions_cmd(
    status: Annotated[str, typer.Option("--status", help="open, closed or all.")] = "open",
    all_: Annotated[bool, typer.Option("--all", "-a", help="Same as --status all.")] = False,
    kind: Annotated[
        str | None, typer.Option("--kind", "-k", help="competition or hackathon.")
    ] = None,
    reward: Annotated[
        str | None, typer.Option("--reward", "-r", help="prize, points or knowledge.")
    ] = None,
    search: Annotated[str | None, typer.Option("--search", "-s", help="Search in titles.")] = None,
    limit: LimitOpt = 30,
    as_json: JsonOpt = False,
) -> None:
    """List competitions (open ones by default)."""
    status = "all" if all_ else status
    for name, value, allowed in (
        ("--status", status, ("open", "closed", "all")),
        ("--kind", kind, ("competition", "hackathon", None)),
        ("--reward", reward, ("prize", "points", "knowledge", None)),
    ):
        if value not in allowed:
            raise typer.BadParameter(
                f"must be one of {', '.join(a for a in allowed if a)}", param_hint=name
            )
    with _errors():
        comps = _client().competitions(
            status=status,  # type: ignore[arg-type]
            kind=kind,  # type: ignore[arg-type]
            reward=reward,  # type: ignore[arg-type]
            search=search,
            limit=limit,
        )
        _emit(comps, as_json)
        if not as_json and comps:
            err.print("[dim]Next: zindi show <id> · zindi join <id> · zindi use <id>[/]")


@app.command()
def show(
    competition: CompetitionArg = None,
    rules: Annotated[bool, typer.Option("--rules", help="Also print the rules.")] = False,
    as_json: JsonOpt = False,
) -> None:
    """Show a competition: reward, deadline, metric, data files, rules."""
    with _errors():
        comp = _client().competition(competition)
        if as_json:
            _print_json(comp)
            return
        out.print(comp)
        if rules and comp.rules:
            out.rule("Rules")
            out.print(comp.rules)


@app.command()
def use(competition: str) -> None:
    """Remember a competition for this project folder (writes zindi.toml)."""
    with _errors():
        comp = _client().competition(competition)
        path = config.write_project_competition(comp.id)
        out.print(
            f"[green]✔[/] Using [bold]{comp.title}[/] ({comp.id}) in this folder. Saved to {path}"
        )


@app.command()
def join(
    competition: CompetitionArg = None,
    code: Annotated[
        str | None, typer.Option("--code", help="Secret code of a private competition.")
    ] = None,
) -> None:
    """Join a competition (you accept its rules)."""
    with _errors():
        client = _client()
        try:
            joined = client.join(competition, secret_code=code)
        except SecretCodeRequiredError:
            if not auth.can_prompt():
                raise
            code = typer.prompt("This competition is private. Secret code", hide_input=True)
            joined = client.join(competition, secret_code=code)
        cid = client.resolve_competition(competition)
        out.print(
            f"[green]✔[/] Joined {cid}. Good luck!" if joined else f"You are already in {cid}."
        )


@app.command()
def download(
    competition: CompetitionArg = None,
    output: Annotated[Path, typer.Option("--output", "-o", help="Destination folder.")] = Path(
        "data"
    ),
    file: Annotated[
        list[str] | None, typer.Option("--file", "-f", help="Only this file (repeatable).")
    ] = None,
    unzip: Annotated[
        bool, typer.Option("--unzip", help="Extract .zip files after download.")
    ] = False,
    force: Annotated[bool, typer.Option("--force", help="Download again even if present.")] = False,
) -> None:
    """Download the data files of a competition."""
    with _errors():
        paths = _client().download(competition, output, files=file, unzip=unzip, overwrite=force)
        out.print(f"[green]✔[/] {len(paths)} file(s) in {output}/")
        for path in paths:
            out.print(f"  {path}")


# -- submissions ---------------------------------------------------------------------------------


@app.command()
def submit(
    file: Annotated[Path, typer.Argument(help="The submission file.", exists=True, dir_okay=False)],
    message: Annotated[
        str, typer.Option("--message", "-m", help="A comment to remember this submission.")
    ] = "",
    competition: CompetitionOpt = None,
    wait: Annotated[bool, typer.Option("--wait", "-w", help="Wait for the score.")] = False,
) -> None:
    """Submit a file and, with --wait, print its score."""
    with _errors():
        client = _client()
        sub = client.submit(file, comment=message, competition=competition)
        out.print(f"[green]✔[/] Submitted {file.name} (id {sub.id}).")
        if wait and sub.id:
            with err.status("Waiting for the score…"):
                sub = client.wait_for_score(sub.id, competition=competition)
            if sub.is_failed:
                err.print(f"[red]✘[/] Submission failed: {sub.status_description or sub.status}")
                raise typer.Exit(code=1)
            out.print(f"[bold]Score:[/] {sub.public_score}")
        try:
            remaining = client.limits(competition).remaining_today
        except ZindiError:
            remaining = None
        if remaining is not None:
            err.print(f"[dim]{remaining} submission(s) left today.[/]")


@app.command()
def submissions(
    competition: CompetitionArg = None, limit: LimitOpt = None, as_json: JsonOpt = False
) -> None:
    """List your submissions, newest first."""
    with _errors():
        _emit(_client().submissions(competition, limit=limit), as_json)


@app.command()
def limits(competition: CompetitionArg = None, as_json: JsonOpt = False) -> None:
    """How many submissions you can still make today."""
    with _errors():
        result = _client().limits(competition)
        if as_json:
            _print_json(result)
        else:
            out.print(f"Remaining submissions today: [bold]{result.remaining_today}[/]")


@app.command()
def leaderboard(
    competition: CompetitionArg = None, limit: LimitOpt = 20, as_json: JsonOpt = False
) -> None:
    """Show the leaderboard (your row is highlighted when you are logged in)."""
    with _errors():
        client = _client()
        board = client.leaderboard(competition, limit=limit)
        _emit(board, as_json, highlight=client.username)


@app.command()
def rank(competition: CompetitionArg = None) -> None:
    """Show your rank on the leaderboard."""
    with _errors():
        client = _client()
        value = client.rank(competition)
        if value is None:
            out.print("You are not ranked yet. Make a valid submission first.")
        else:
            out.print(f"You are [bold]#{value}[/] on {client.resolve_competition(competition)}.")


@app.command()
def discussions(
    competition: CompetitionArg = None, limit: LimitOpt = 20, as_json: JsonOpt = False
) -> None:
    """List the latest discussion threads of a competition."""
    with _errors():
        _emit(_client().discussions(competition, limit=limit), as_json)


# -- teams ---------------------------------------------------------------------------------------


@team_app.command("show")
def team_show(competition: CompetitionArg = None, as_json: JsonOpt = False) -> None:
    """Show your team."""
    with _errors():
        team = _client().team(competition)
        if team is None:
            out.print("You compete alone in this competition.")
        else:
            _emit(team, as_json)


@team_app.command("create")
def team_create(
    name: str,
    invite: Annotated[
        list[str] | None, typer.Option("--invite", "-i", help="Username to invite (repeatable).")
    ] = None,
    competition: CompetitionOpt = None,
) -> None:
    """Create a team and optionally invite people."""
    with _errors():
        team = _client().create_team(name, competition=competition, invite=invite or [])
        out.print(f"[green]✔[/] Team [bold]{team.title}[/] created.")
        for username in invite or []:
            out.print(f"  invited {username}")


@team_app.command("invite")
def team_invite(usernames: list[str], competition: CompetitionOpt = None) -> None:
    """Invite users to your team."""
    with _errors():
        for username in _client().invite(usernames, competition=competition):
            out.print(f"[green]✔[/] Invited {username}")


@team_app.command("disband")
def team_disband(
    competition: CompetitionArg = None,
    yes: Annotated[bool, typer.Option("--yes", "-y", help="Do not ask for confirmation.")] = False,
) -> None:
    """Disband your team (leaders only)."""
    if not yes:
        typer.confirm("Disband your team? Members will compete alone again.", abort=True)
    with _errors():
        _client().disband_team(competition)
        out.print("[green]✔[/] Team disbanded.")


# -- diagnostics ---------------------------------------------------------------------------------


@app.command()
def doctor() -> None:
    """Check your setup: Python, paths, network, credentials. Paste this in bug reports."""
    import httpx

    def line(ok: bool | None, label: str, detail: str) -> None:
        mark = {True: "[green]✔[/]", False: "[red]✘[/]", None: "[yellow]•[/]"}[ok]
        out.print(f"{mark} [bold]{label}[/]: {detail}")

    line(None, "zindi", __version__)
    line(None, "Python", f"{platform.python_version()} ({sys.executable})")
    line(None, "Platform", f"{platform.system()} {platform.release()} {platform.machine()}")
    env = (
        "Google Colab"
        if "google.colab" in sys.modules
        else "Kaggle"
        if os.environ.get("KAGGLE_KERNEL_RUN_TYPE")
        else "notebook"
        if auth.in_notebook()
        else "terminal"
    )
    line(None, "Environment", env)
    line(None, "Config folder", str(config.config_dir()))
    project = config.find_project_file()
    line(
        None,
        "Project file",
        f"{project} → {config.read_project_competition()}" if project else "none",
    )
    base = base_url_from_env()
    try:
        response = httpx.get(
            f"{base}/competitions", params={"per_page": 1}, timeout=15, follow_redirects=True
        )
        line(response.is_success, "API", f"{base} (HTTP {response.status_code})")
    except httpx.HTTPError as exc:
        line(False, "API", f"{base} unreachable: {exc}")
    found = auth.resolve()
    if found is None:
        line(False, "Credentials", "none. Run `zindi login`.")
        return
    try:
        user = Client().me()
        line(True, "Credentials", f"{user.username} (from {found.source})")
    except ZindiError as exc:
        line(False, "Credentials", f"found in {found.source} but rejected: {exc}")


def main() -> None:
    """Entry point of the ``zindi`` script."""
    if sys.platform == "win32":  # pragma: no cover - legacy Windows consoles
        for stream in (sys.stdout, sys.stderr):
            with suppress(AttributeError, ValueError):
                stream.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    app()
