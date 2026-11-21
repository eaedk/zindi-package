"""The :class:`Client`: everything you can do on Zindi, from Python."""

from __future__ import annotations

import io
import mimetypes
import os
import time
import zipfile
from collections.abc import Callable, Iterable, Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Literal

import httpx
from rich.console import Console
from rich.progress import (
    BarColumn,
    DownloadColumn,
    Progress,
    TextColumn,
    TimeRemainingColumn,
    TransferSpeedColumn,
)

from . import auth, config
from ._http import Transport
from .errors import PASSWORD_RESET_URL as _RESET_URL
from .errors import (
    AlreadyJoinedError,
    APIError,
    AuthenticationError,
    CompetitionNotSetError,
    ZindiError,
)
from .models import (
    Competition,
    DataFile,
    Discussion,
    LeaderboardEntry,
    Participation,
    ResultList,
    Submission,
    SubmissionLimits,
    Team,
    User,
)

__all__ = ["Client"]

Status = Literal["open", "closed", "all"]
Kind = Literal["competition", "hackathon"]
Reward = Literal["prize", "points", "knowledge"]

_CHUNK = 1024 * 256


class Client:
    """A connection to Zindi.

    Most methods take an optional ``competition`` id. When you leave it out, the client uses,
    in order: the competition set with :meth:`use`, the ``ZINDI_COMPETITION`` environment
    variable, then the ``zindi.toml`` file of your project (written by ``zindi use``).

    You do not need to log in to browse competitions and leaderboards. For everything else
    the client finds your credentials by itself (see :mod:`zindi.auth`) and, as a last
    resort, asks for them.

    Args:
        token: An auth token. Overrides every other source of credentials.
        username: Your Zindi username (used with ``password`` to sign in).
        password: Your Zindi password. Prefer environment variables or ``zindi login``.
        competition: Default competition id for this client.
        base_url: API root. Defaults to ``$ZINDI_API_URL`` or ``https://api.zindi.world/v1``.
        timeout: Network timeout in seconds.
        max_retries: Retries for transient failures (timeouts, HTTP 429/5xx on reads).
        progress: Show progress bars for downloads and uploads.
        use_saved_credentials: Read the credentials file written by ``zindi login``.
        http_client: A preconfigured :class:`httpx.Client` (proxies, custom certificates).

    Example:
        >>> import zindi
        >>> client = zindi.Client()
        >>> client.competitions(status="open")          # doctest: +SKIP
        >>> comp = client.competition("Crop-Yield-Forecast-Challenge")  # doctest: +SKIP
        >>> comp.join(); comp.download("data")          # doctest: +SKIP
    """

    def __init__(
        self,
        token: str | None = None,
        *,
        username: str | None = None,
        password: str | None = None,
        competition: str | None = None,
        base_url: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 3,
        progress: bool = True,
        use_saved_credentials: bool = True,
        http_client: httpx.Client | None = None,
        _sleep: Callable[[float], None] = time.sleep,
    ) -> None:
        self._http = Transport(
            base_url=base_url,
            token=token,
            timeout=timeout,
            max_retries=max_retries,
            http_client=http_client,
            sleep=_sleep,
        )
        self._sleep = _sleep
        self._username = username
        self._password = password
        self._use_saved = use_saved_credentials
        self._credentials_source = "argument" if token else None
        self._resolved = bool(token)
        self.competition_id = competition
        self.progress = progress
        self._console = Console(stderr=True)
        self._last_total: int | None = None

    # -- context manager ---------------------------------------------------------------------

    def __enter__(self) -> Client:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def close(self) -> None:
        """Close network connections."""
        self._http.close()

    def __repr__(self) -> str:
        who = self._username or "anonymous"
        return f"Client(user={who!r}, competition={self.competition_id!r})"

    # -- authentication ----------------------------------------------------------------------

    @property
    def token(self) -> str | None:
        """The current auth token, if any (does not trigger a login)."""
        return self._http.token

    @property
    def username(self) -> str | None:
        """The username of the logged-in user, if known."""
        return self._username

    @property
    def credentials_source(self) -> str | None:
        """Where the credentials came from, e.g. ``"environment variables"``."""
        return self._credentials_source

    @property
    def is_authenticated(self) -> bool:
        """``True`` if credentials are available, prompting nobody to find out."""
        self._resolve_credentials()
        return bool(self._http.token or (self._username and self._password))

    def login(
        self,
        username: str | None = None,
        password: str | None = None,
        *,
        save: bool = True,
    ) -> User:
        """Sign in with a username and password, and return your profile.

        Missing values are taken from the environment or asked for interactively.
        With ``save=True`` the auth token (never the password) is stored in the
        shared credentials file, so later sessions, the CLI and the other zindi
        clients are logged in too.
        """
        username = username or self._username
        password = password or self._password
        if not (username and password):
            found = auth.resolve(use_saved=False)
            if found and found.can_sign_in:
                username = username or found.username
                password = password or found.password
        if not username:
            if not auth.can_prompt():
                raise AuthenticationError("A username is required to log in.")
            username = auth.prompt_username()
        if not password:
            if not auth.can_prompt():
                raise AuthenticationError("A password is required to log in.")
            password = auth.prompt_password(username)

        data = self._sign_in(username, password)
        if save:
            config.save_credentials(self._username or username, self._http.token or "")
        user = data.get("user") if isinstance(data, dict) else None
        return User.from_api(user or {"username": self._username})

    def _sign_in(self, username: str, password: str) -> dict[str, Any]:
        try:
            data = self._http.request(
                "POST", "auth/signin", auth=False, data={"username": username, "password": password}
            )
        except APIError as exc:
            fields = exc.errors if isinstance(exc.errors, dict) else {}
            if "password" in fields or "username" in fields or "login" in str(exc).lower():
                raise AuthenticationError(
                    f"Login failed: {exc.message}. Check your username and password. "
                    f"Forgot your password? Reset it at {_RESET_URL}",
                    status_code=exc.status_code,
                    errors=exc.errors,
                ) from None
            raise
        token = data.get("auth_token") if isinstance(data, dict) else None
        if not token:
            raise AuthenticationError("Zindi did not return an auth token.")
        self._http.token = str(token)
        user = data.get("user") or {}
        self._username = str(user.get("username") or username)
        self._password = password
        self._resolved = True
        self._credentials_source = self._credentials_source or "password"
        return dict(data)

    def logout(self, *, forget: bool = True) -> None:
        """Forget the token in memory and, with ``forget=True``, the saved credentials file."""
        self._http.token = None
        self._password = None
        self._resolved = True
        if forget:
            config.delete_credentials()

    def _resolve_credentials(self) -> None:
        if self._resolved:
            return
        self._resolved = True
        if self._username and self._password:
            self._credentials_source = "argument"
            return
        found = auth.resolve(use_saved=self._use_saved)
        if found is None:
            return
        self._credentials_source = found.source
        self._username = self._username or found.username
        if found.token:
            self._http.token = found.token
        if found.can_sign_in:
            self._password = found.password

    def _ensure_token(self) -> None:
        self._resolve_credentials()
        if self._http.token:
            return
        if self._username and self._password:
            self._sign_in(self._username, self._password)
            return
        if auth.can_prompt():
            self._console.print("[bold]Log in to Zindi[/] (your token will be saved for next time)")
            self.login(save=True)
            self._console.print(f"Saved credentials to {config.credentials_path()}")
            return
        raise AuthenticationError(
            "You are not logged in. Run `zindi login`, or set ZINDI_TOKEN "
            "(or ZINDI_USERNAME and ZINDI_PASSWORD)."
        )

    def _call(self, method: str, path: str, *, auth_required: bool = True, **kwargs: Any) -> Any:
        """Make an API call, signing in first if needed and once more if the token expired."""
        if auth_required:
            self._ensure_token()
        else:
            self._resolve_credentials()
        try:
            return self._http.request(method, path, **kwargs)
        except AuthenticationError:
            if not (auth_required and self._username and self._password):
                raise
            self._sign_in(self._username, self._password)
            return self._http.request(method, path, **kwargs)

    # -- competition selection ---------------------------------------------------------------

    def use(self, competition: str, *, save: bool = False) -> str:
        """Make ``competition`` the default for this client.

        With ``save=True`` it is also written to ``zindi.toml`` in the current folder, which
        the CLI and future clients started from this project will pick up.
        """
        self.competition_id = competition
        if save:
            config.write_project_competition(competition)
        return competition

    def resolve_competition(self, competition: str | None = None) -> str:
        """Return the competition id that a method would use for ``competition``.

        Raises:
            CompetitionNotSetError: No competition was given and none is configured.
        """
        return self._cid(competition)

    def _cid(self, competition: str | None) -> str:
        cid = (
            competition
            or self.competition_id
            or os.environ.get("ZINDI_COMPETITION")
            or config.read_project_competition()
        )
        if not cid:
            raise CompetitionNotSetError()
        return cid

    # -- users -------------------------------------------------------------------------------

    def me(self) -> User:
        """Your own profile."""
        data = self._call("GET", "me")
        # ``/me`` answers ``{"auth_token": ..., "user": {...}}``; older versions sent the user.
        if isinstance(data, dict) and isinstance(data.get("user"), dict):
            data = data["user"]
        return User.from_api(data if isinstance(data, dict) else {})

    def user(self, username: str) -> User:
        """The public profile of any user."""
        return User.from_api(self._call("GET", f"users/{username}", auth_required=False))

    # -- competitions ------------------------------------------------------------------------

    def competitions(
        self,
        *,
        status: Status = "open",
        kind: Kind | None = None,
        reward: Reward | None = None,
        search: str | None = None,
        limit: int | None = 50,
    ) -> ResultList[Competition]:
        """List competitions.

        Args:
            status: ``"open"`` (default), ``"closed"`` or ``"all"``.
            kind: ``"competition"`` or ``"hackathon"``; both when ``None``.
            reward: ``"prize"``, ``"points"`` or ``"knowledge"``; any when ``None``.
            search: Free-text search on titles.
            limit: Maximum number of results; ``None`` for all of them.
        """
        params: dict[str, Any] = {"kind[]": kind, "prize": reward, "query": search}
        if status == "open":
            params["active"] = "true"
        items: list[Competition] = []
        for raw in self._paginate("competitions", params=params, per_page=50, auth_required=False):
            comp = Competition.from_api(raw, client=self)
            if status == "open" and not comp.is_open:
                continue
            if status == "closed" and comp.is_open:
                continue
            if reward and comp.reward_type and comp.reward_type != reward:
                continue
            if kind and comp.kind and comp.kind != kind:
                continue
            items.append(comp)
            if limit is not None and len(items) >= limit:
                break
        label = {"open": "Open competitions", "closed": "Closed competitions"}.get(
            status, "Competitions"
        )
        return ResultList(items, title=label)

    def competition(self, competition: str | None = None) -> Competition:
        """Full details of a competition: data files, rules, metric, team size, ..."""
        cid = self._cid(competition)
        return Competition.from_api(
            self._call("GET", f"competitions/{cid}", auth_required=False), self
        )

    def join(self, competition: str | None = None, *, secret_code: str | None = None) -> bool:
        """Join a competition (accepting its rules).

        Returns ``True`` if you just joined and ``False`` if you were already in.

        Raises:
            SecretCodeRequiredError: The competition is private and ``secret_code`` is missing.
        """
        cid = self._cid(competition)
        body = {"secret_code": secret_code} if secret_code else None
        try:
            self._call("POST", f"competitions/{cid}/participations", json=body)
        except AlreadyJoinedError:
            return False
        return True

    # -- data --------------------------------------------------------------------------------

    def download(
        self,
        competition: str | None = None,
        destination: str | os.PathLike[str] = ".",
        *,
        files: Iterable[str] | None = None,
        unzip: bool = False,
        overwrite: bool = False,
    ) -> list[Path]:
        """Download a competition's data files into ``destination``.

        Files that are already there with the expected size are skipped, so you can call this
        at the top of every notebook. Downloads go to a temporary ``.part`` file first, so an
        interrupted download never leaves a truncated file behind.

        Args:
            files: Only these file names. By default, every file except the "everything in
                one zip" archive, unless that archive is the only file.
            unzip: Extract ``.zip`` files after downloading them.
            overwrite: Download again even if the file is already there.

        Returns:
            The paths of the files on disk.
        """
        cid = self._cid(competition)
        comp = self.competition(cid)
        chosen = self._choose_files(comp.data_files, files)
        target = Path(destination)
        target.mkdir(parents=True, exist_ok=True)
        self._ensure_token()

        paths: list[Path] = []
        with self._progress() as progress:
            for data_file in chosen:
                path = target / data_file.filename
                if (
                    not overwrite
                    and path.is_file()
                    and (data_file.size is None or path.stat().st_size == data_file.size)
                ):
                    paths.append(path)
                    continue
                self._download_file(cid, data_file, path, progress)
                paths.append(path)
        if unzip:
            for path in list(paths):
                if path.suffix.lower() == ".zip":
                    _safe_extract(path, target)
        return paths

    @staticmethod
    def _choose_files(available: list[DataFile], wanted: Iterable[str] | None) -> list[DataFile]:
        if wanted is not None:
            names = list(wanted)
            by_name = {f.filename: f for f in available}
            missing = [n for n in names if n not in by_name]
            if missing:
                known = ", ".join(by_name) or "none"
                raise ZindiError(f"Unknown file(s): {', '.join(missing)}. Available: {known}")
            return [by_name[n] for n in names]
        individual = [f for f in available if not f.is_archive_of_everything]
        return individual or list(available)

    def _download_file(
        self, cid: str, data_file: DataFile, path: Path, progress: Progress | None
    ) -> None:
        part = path.with_name(path.name + ".part")
        task = progress.add_task(data_file.filename, total=data_file.size) if progress else None
        try:
            with self._http.download(f"competitions/{cid}/files/{data_file.filename}") as response:
                length = response.headers.get("Content-Length")
                if progress is not None and task is not None and length and length.isdigit():
                    progress.update(task, total=int(length))
                with part.open("wb") as fh:
                    for chunk in response.iter_bytes(_CHUNK):
                        fh.write(chunk)
                        if progress is not None and task is not None:
                            progress.advance(task, len(chunk))
            os.replace(part, path)
        except AuthenticationError as exc:
            part.unlink(missing_ok=True)
            raise AuthenticationError(
                f"Zindi refused the download of {data_file.filename}: {exc.message}. "
                f"Have you joined the competition? Try `zindi join {cid}`.",
                status_code=exc.status_code,
                errors=exc.errors,
            ) from None
        except BaseException:
            part.unlink(missing_ok=True)
            raise

    # -- submissions -------------------------------------------------------------------------

    def submit(
        self,
        path: str | os.PathLike[str],
        comment: str = "",
        *,
        competition: str | None = None,
        wait: bool = False,
        timeout: float = 600.0,
        poll_interval: float = 10.0,
    ) -> Submission:
        """Upload a submission file.

        Args:
            path: The file to submit (CSV, or whatever the competition expects).
            comment: A note to remember what this submission was.
            wait: Block until Zindi has scored the submission and return it with its score.
            timeout: With ``wait=True``, give up waiting after this many seconds.
            poll_interval: With ``wait=True``, seconds between two checks.
        """
        cid = self._cid(competition)
        file_path = Path(path)
        if not file_path.is_file():
            raise ZindiError(f"Submission file not found: {file_path}")
        self._ensure_token()
        size = file_path.stat().st_size
        mime = mimetypes.guess_type(file_path.name)[0] or "application/octet-stream"
        with self._progress(upload=True) as progress, file_path.open("rb") as raw_fh:
            task = (
                progress.add_task(f"Submitting {file_path.name}", total=size) if progress else None
            )
            on_read = (
                (lambda n: progress.advance(task, n)) if progress and task is not None else None
            )
            fh = _ReadCallback(raw_fh, on_read) if on_read else raw_fh
            data = self._call(
                "POST",
                f"competitions/{cid}/submissions",
                files={"file": (file_path.name, fh, mime)},
                data={"comment": comment},
            )
        submission = Submission.from_api(data if isinstance(data, dict) else {})
        if wait and submission.id:
            return self.wait_for_score(
                submission.id, competition=cid, timeout=timeout, poll_interval=poll_interval
            )
        return submission

    def submissions(
        self, competition: str | None = None, *, limit: int | None = None
    ) -> ResultList[Submission]:
        """Your submissions to a competition, newest first."""
        cid = self._cid(competition)
        data = self._call(
            "GET", f"competitions/{cid}/submissions", params={"per_page": limit or 1000}
        )
        items = [Submission.from_api(s) for s in data or [] if isinstance(s, dict)]
        if limit is not None:
            items = items[:limit]
        return ResultList(items, title=f"Your submissions to {cid}")

    def wait_for_score(
        self,
        submission_id: str,
        *,
        competition: str | None = None,
        timeout: float = 600.0,
        poll_interval: float = 10.0,
    ) -> Submission:
        """Poll until a submission is scored (or failed) and return it."""
        cid = self._cid(competition)
        deadline = time.monotonic() + timeout
        while True:
            for sub in self.submissions(cid):
                if sub.id == str(submission_id) and not sub.is_pending:
                    return sub
            if time.monotonic() >= deadline:
                raise ZindiError(
                    f"Submission {submission_id} was not scored within {timeout:.0f}s. "
                    f"Check later with `zindi submissions {cid}`."
                )
            self._sleep(poll_interval)

    def limits(self, competition: str | None = None) -> SubmissionLimits:
        """How many submissions you can still make today."""
        cid = self._cid(competition)
        data = self._call("GET", f"competitions/{cid}/submissions/limits")
        return SubmissionLimits.from_api(data if isinstance(data, dict) else {})

    # -- leaderboard -------------------------------------------------------------------------

    def leaderboard(
        self,
        competition: str | None = None,
        *,
        limit: int | None = None,
        include_unranked: bool = False,
    ) -> ResultList[LeaderboardEntry]:
        """The leaderboard of a competition (private ranks once they are revealed)."""
        cid = self._cid(competition)
        rows: list[LeaderboardEntry] = []
        for raw in self._paginate(
            f"competitions/{cid}/participations", per_page=100, auth_required=False
        ):
            entry = LeaderboardEntry.from_api(raw)
            if entry.rank is None and not include_unranked:
                break  # unranked rows come last: nobody after this has a valid score
            rows.append(entry)
            if limit is not None and len(rows) >= limit:
                break
        return ResultList(rows, total=self._last_total, title=f"Leaderboard · {cid}")

    def my_participation(self, competition: str | None = None) -> Participation:
        """Your rank, best score and submission count in a competition."""
        cid = self._cid(competition)
        data = self._call("GET", f"competitions/{cid}/participations/my_participation")
        return Participation.from_api(data if isinstance(data, dict) else {})

    def rank(self, competition: str | None = None) -> int | None:
        """Your rank on the leaderboard, or ``None`` if you have no valid score yet."""
        cid = self._cid(competition)
        try:
            rank = self.my_participation(cid).rank
        except APIError:
            rank = None
        if rank is not None:
            return rank
        username = self._username
        if not username:
            return None
        for entry in self.leaderboard(cid):
            if entry.includes(username):
                return entry.rank
        return None

    # -- teams -------------------------------------------------------------------------------

    def team(self, competition: str | None = None) -> Team | None:
        """Your team in a competition, or ``None`` if you compete alone."""
        cid = self._cid(competition)
        try:
            data = self._call("GET", f"competitions/{cid}/my_team")
        except APIError as exc:
            if exc.status_code == 404 or "not found" in exc.message.lower():
                return None
            raise
        return Team.from_api(data) if isinstance(data, dict) and data else None

    def create_team(
        self, name: str, *, competition: str | None = None, invite: Iterable[str] = ()
    ) -> Team:
        """Create a team (you become its leader) and optionally invite people."""
        cid = self._cid(competition)
        data = self._call("POST", f"competitions/{cid}/my_team", data={"title": name})
        team = Team.from_api(data if isinstance(data, dict) else {"title": name})
        invitees = list(invite)
        if invitees:
            self.invite(invitees, competition=cid)
        return team

    def invite(
        self, usernames: str | Iterable[str], *, competition: str | None = None
    ) -> list[str]:
        """Invite one or more users to your team. Returns who was invited.

        People who already have a pending invitation count as invited.
        """
        cid = self._cid(competition)
        names = [usernames] if isinstance(usernames, str) else list(usernames)
        invited: list[str] = []
        for name in names:
            try:
                self._call("POST", f"competitions/{cid}/my_team/invite", data={"username": name})
            except APIError as exc:
                if "already invited" not in exc.message.lower():
                    raise
            invited.append(name)
        return invited

    def disband_team(self, competition: str | None = None) -> None:
        """Disband your team (you must be its leader)."""
        cid = self._cid(competition)
        self._call("DELETE", f"competitions/{cid}/my_team")

    # -- discussions -------------------------------------------------------------------------

    def discussions(
        self, competition: str | None = None, *, limit: int | None = 20
    ) -> ResultList[Discussion]:
        """Discussion threads of a competition, newest first."""
        cid = self._cid(competition)
        items = [
            Discussion.from_api(raw, competition=cid)
            for raw in self._paginate(
                f"competitions/{cid}/discussions", per_page=20, limit=limit, auth_required=False
            )
        ]
        return ResultList(items, title=f"Discussions · {cid}")

    # -- internals ---------------------------------------------------------------------------

    def _paginate(
        self,
        path: str,
        *,
        params: dict[str, Any] | None = None,
        per_page: int = 100,
        limit: int | None = None,
        auth_required: bool = True,
    ) -> Iterator[dict[str, Any]]:
        if auth_required:
            self._ensure_token()
        else:
            self._resolve_credentials()
        page_number = 0
        seen = 0
        while True:
            page = self._http.page(
                path, params={**(params or {}), "page": page_number, "per_page": per_page}
            )
            self._last_total = page.total
            for item in page.items:
                yield item
                seen += 1
                if limit is not None and seen >= limit:
                    return
            if len(page.items) < per_page or (page.total is not None and seen >= page.total):
                return
            page_number += 1

    @contextmanager
    def _progress(self, *, upload: bool = False) -> Iterator[Progress | None]:
        if not self.progress:
            yield None
            return
        columns = [
            TextColumn("[bold]{task.description}"),
            BarColumn(),
            DownloadColumn(),
            TransferSpeedColumn(),
            TimeRemainingColumn(),
        ]
        with Progress(*columns, console=self._console, transient=upload) as progress:
            yield progress


class _ReadCallback(io.RawIOBase):
    """File wrapper that reports how many bytes were read (for upload progress bars)."""

    def __init__(self, fh: io.BufferedReader, callback: Callable[[int], None]) -> None:
        self._fh = fh
        self._callback = callback

    def readable(self) -> bool:
        return True

    def read(self, size: int = -1) -> bytes:
        chunk = self._fh.read(size)
        if chunk:
            self._callback(len(chunk))
        return chunk

    def seek(self, offset: int, whence: int = io.SEEK_SET) -> int:
        return self._fh.seek(offset, whence)

    def tell(self) -> int:
        return self._fh.tell()

    def fileno(self) -> int:
        return self._fh.fileno()


def _safe_extract(archive: Path, target: Path) -> None:
    """Extract a zip file, refusing entries that would land outside ``target``."""
    root = target.resolve()
    with zipfile.ZipFile(archive) as zf:
        for member in zf.infolist():
            destination = (root / member.filename).resolve()
            if root != destination and root not in destination.parents:
                raise ZindiError(f"Refusing to extract {member.filename!r}: unsafe path.")
        zf.extractall(root)
