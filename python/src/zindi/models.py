"""Typed views over Zindi API objects.

Every model keeps the untouched API payload in ``raw``, so nothing the platform
returns is ever out of reach, even fields this package does not know about yet.
"""

from __future__ import annotations

import html
import re
from collections.abc import Iterable, Mapping
from dataclasses import dataclass, field, fields
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING, Any, ClassVar, Generic, TypeVar

if TYPE_CHECKING:
    import pandas as pd
    from rich.console import RenderableType

    from .client import Client

__all__ = [
    "Competition",
    "DataFile",
    "Discussion",
    "LeaderboardEntry",
    "Participation",
    "ResultList",
    "Submission",
    "SubmissionLimits",
    "Team",
    "User",
]

WEB_URL = "https://zindi.world"
_DAILY_LIMIT = re.compile(r"maximum of\s+(\d+)\s+submissions?\s+per\s+day", re.IGNORECASE)
_TAGS = re.compile(r"<[^>]+>")


# -- parsing helpers -------------------------------------------------------------------------


def parse_time(value: Any) -> datetime | None:
    """Parse an ISO 8601 timestamp as returned by Zindi (``2026-11-18T10:02:11.000Z``)."""
    if not value or not isinstance(value, str):
        return None
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None


def _int(value: Any) -> int | None:
    try:
        return int(value) if value is not None else None
    except (TypeError, ValueError):
        return None


def _float(value: Any) -> float | None:
    try:
        return float(value) if value is not None else None
    except (TypeError, ValueError):
        return None


def _str(value: Any) -> str | None:
    return str(value) if value is not None else None


def _first(raw: Mapping[str, Any], *keys: str) -> Any:
    for key in keys:
        if raw.get(key) is not None:
            return raw[key]
    return None


def html_to_text(content: str | None) -> str:
    """Very small HTML-to-text conversion, good enough for rules and descriptions."""
    if not content:
        return ""
    text = re.sub(r"<\s*(br|/p|/li|/h\d|/div)\s*/?>", "\n", content, flags=re.IGNORECASE)
    text = re.sub(r"<\s*li[^>]*>", "• ", text, flags=re.IGNORECASE)
    text = html.unescape(_TAGS.sub("", text))
    return re.sub(r"\n{3,}", "\n\n", text).strip()


# -- result collections ----------------------------------------------------------------------

T = TypeVar("T", bound="_Model")


class _Model:
    """Shared behaviour: tabular columns and rich/HTML rendering."""

    #: ``(header, attribute)`` pairs shown in tables.
    columns: ClassVar[tuple[tuple[str, str], ...]] = ()

    def to_dict(self) -> dict[str, Any]:
        """Return the model's fields as a plain dict (without ``raw``)."""
        out: dict[str, Any] = {}
        for f in fields(self):  # type: ignore[arg-type]
            if f.name in ("raw", "client") or f.name.startswith("_"):
                continue
            value = getattr(self, f.name)
            if isinstance(value, list):
                value = [v.to_dict() if isinstance(v, _Model) else v for v in value]
            out[f.name] = value
        return out

    def __rich__(self) -> RenderableType:
        from ._display import rich_record

        return rich_record(self)

    def _repr_html_(self) -> str:
        from ._display import html_record

        return html_record(self)


class ResultList(list[T], Generic[T]):
    """A list of models that also renders as a table and converts to pandas.

    It behaves exactly like a ``list``; the extras are:

    * ``to_pandas()`` returns a :class:`pandas.DataFrame` (needs ``pip install zindi[pandas]``),
    * ``to_records()`` returns a list of plain dicts,
    * it displays as a table in a terminal (rich) and in Jupyter or Colab.
    """

    def __init__(self, items: Iterable[T] = (), *, total: int | None = None, title: str = ""):
        super().__init__(items)
        self.total = total
        self.title = title

    def to_records(self) -> list[dict[str, Any]]:
        return [item.to_dict() for item in self]

    def to_pandas(self) -> pd.DataFrame:
        try:
            import pandas as pd
        except ImportError as exc:  # pragma: no cover - depends on the environment
            raise ImportError("to_pandas() needs pandas: pip install 'zindi[pandas]'") from exc
        return pd.DataFrame(self.to_records())

    def __rich__(self) -> RenderableType:
        from ._display import rich_table

        return rich_table(self)

    def _repr_html_(self) -> str:
        from ._display import html_table

        return html_table(self)


# -- models ----------------------------------------------------------------------------------


@dataclass(repr=False)
class User(_Model):
    """A Zindi user profile."""

    username: str
    first_name: str | None = None
    last_name: str | None = None
    title: str | None = None
    country: str | None = None
    countrycode: str | None = None
    points: int | None = None
    rank: int | None = None
    best_rank: int | None = None
    competitions_count: int | None = None
    submissions_count: int | None = None
    gold_medals: int = 0
    silver_medals: int = 0
    bronze_medals: int = 0
    joined_at: datetime | None = None
    raw: dict[str, Any] = field(default_factory=dict)

    columns = (
        ("Username", "username"),
        ("Name", "full_name"),
        ("Country", "country"),
        ("Rank", "rank"),
        ("Points", "points"),
        ("Medals", "medals"),
    )

    @classmethod
    def from_api(cls, raw: Mapping[str, Any]) -> User:
        return cls(
            username=str(raw.get("username", "")),
            first_name=_str(raw.get("first_name")),
            last_name=_str(raw.get("last_name")),
            title=_str(raw.get("title")),
            country=_str(raw.get("country")),
            countrycode=_str(raw.get("countrycode")),
            points=_int(raw.get("points")),
            rank=_int(raw.get("rank")),
            best_rank=_int(raw.get("best_rank")),
            competitions_count=_int(raw.get("count_of_competitions")),
            submissions_count=_int(raw.get("count_of_submissions")),
            gold_medals=_int(raw.get("user_medals_summary_gold_count")) or 0,
            silver_medals=_int(raw.get("user_medals_summary_silver_count")) or 0,
            bronze_medals=_int(raw.get("user_medals_summary_bronze_count")) or 0,
            joined_at=parse_time(raw.get("created_at")),
            raw=dict(raw),
        )

    @property
    def full_name(self) -> str:
        return " ".join(p for p in (self.first_name, self.last_name) if p)

    @property
    def medals(self) -> str:
        return f"🥇{self.gold_medals} 🥈{self.silver_medals} 🥉{self.bronze_medals}"

    @property
    def url(self) -> str:
        return f"{WEB_URL}/users/{self.username}"

    def __repr__(self) -> str:
        return f"User(username={self.username!r}, rank={self.rank}, points={self.points})"


@dataclass(repr=False)
class DataFile(_Model):
    """One file of a competition's dataset."""

    filename: str
    size: int | None = None
    description: str | None = None
    is_archive_of_everything: bool = False
    raw: dict[str, Any] = field(default_factory=dict)

    columns = (("File", "filename"), ("Size", "size_human"), ("Description", "description"))

    @classmethod
    def from_api(cls, raw: Mapping[str, Any]) -> DataFile:
        return cls(
            filename=str(raw.get("filename", "")),
            size=_int(raw.get("filesize")),
            description=_str(raw.get("description")),
            is_archive_of_everything=bool(raw.get("zip_of_everything", False)),
            raw=dict(raw),
        )

    @property
    def size_human(self) -> str:
        if self.size is None:
            return "?"
        size = float(self.size)
        for unit in ("B", "KB", "MB", "GB"):
            if size < 1024 or unit == "GB":
                return f"{size:.0f} {unit}" if unit == "B" else f"{size:.1f} {unit}"
            size /= 1024
        return f"{size:.1f} GB"  # pragma: no cover

    def __repr__(self) -> str:
        return f"DataFile({self.filename!r}, size={self.size_human!r})"


@dataclass(repr=False)
class Competition(_Model):
    """A Zindi competition or hackathon.

    Objects returned by :meth:`zindi.Client.competition` carry the full details (data files,
    rules, metric). Objects in a :meth:`zindi.Client.competitions` listing carry a summary.

    A competition is bound to the client that fetched it, so you can act on it directly::

        comp = client.competition("Crop-Yield-Forecast-Challenge")
        comp.join()
        comp.download("data/")
        comp.submit("submission.csv", comment="baseline")
    """

    id: str
    title: str
    subtitle: str | None = None
    kind: str | None = None
    reward: str | None = None
    reward_type: str | None = None
    organization: str | None = None
    is_open: bool = False
    start_time: datetime | None = None
    end_time: datetime | None = None
    participants: int | None = None
    secret_code_required: bool = False
    is_participant: bool = False
    metric: str | None = None
    max_team_size: int | None = None
    data_files: list[DataFile] = field(default_factory=list)
    pages: dict[str, str] = field(default_factory=dict)
    raw: dict[str, Any] = field(default_factory=dict)
    client: Client | None = field(default=None, compare=False)

    columns = (
        ("Id", "id"),
        ("Reward", "reward"),
        ("Ends", "end_date"),
        ("Participants", "participants"),
        ("Title", "title"),
    )

    @classmethod
    def from_api(cls, raw: Mapping[str, Any], client: Client | None = None) -> Competition:
        pages: dict[str, str] = {}
        for page in raw.get("pages") or []:
            if isinstance(page, Mapping):
                key = str(page.get("url_title") or page.get("title") or "").lower()
                if key:
                    pages[key] = str(page.get("content_html") or "")
        rules = raw.get("rules")
        if isinstance(rules, Mapping) and rules.get("content_html") and "rules" not in pages:
            pages["rules"] = str(rules["content_html"])
        return cls(
            id=str(raw.get("id", "")),
            title=str(_first(raw, "title", "subtitle", "id") or ""),
            subtitle=_str(raw.get("subtitle")),
            kind=_str(raw.get("kind")),
            reward=_str(raw.get("reward")),
            reward_type=_str(raw.get("reward_type")),
            organization=_str(raw.get("organization")),
            is_open=bool(raw.get("open", False)),
            start_time=parse_time(raw.get("start_time")),
            end_time=parse_time(raw.get("end_time")),
            participants=_int(raw.get("participations_count")),
            secret_code_required=bool(raw.get("secret_code_required", False)),
            is_participant=bool(raw.get("is_participant", False)),
            metric=_str(_first(raw, "error_metric")),
            max_team_size=_int(raw.get("max_number_of_participants_per_team")),
            data_files=[
                DataFile.from_api(f) for f in raw.get("datafiles") or [] if isinstance(f, Mapping)
            ],
            pages=pages,
            raw=dict(raw),
            client=client,
        )

    # -- convenience properties --------------------------------------------------------------

    @property
    def url(self) -> str:
        return f"{WEB_URL}/competitions/{self.id}"

    @property
    def status(self) -> str:
        return "open" if self.is_open else "closed"

    @property
    def end_date(self) -> str:
        return self.end_time.strftime("%Y-%m-%d") if self.end_time else ""

    @property
    def description(self) -> str:
        """The competition description, as plain text."""
        return html_to_text(self.pages.get("description") or next(iter(self.pages.values()), ""))

    @property
    def rules(self) -> str:
        """The competition rules, as plain text."""
        return html_to_text(self.pages.get("rules"))

    @property
    def daily_submission_limit(self) -> int | None:
        """Maximum submissions per day, read from the rules (``None`` if not stated)."""
        match = _DAILY_LIMIT.search(html_to_text(self.pages.get("rules")))
        return int(match.group(1)) if match else None

    # -- actions (delegated to the client) ---------------------------------------------------

    def _require_client(self) -> Client:
        if self.client is None:
            raise RuntimeError("This competition is not bound to a client.")
        return self.client

    def join(self, secret_code: str | None = None) -> bool:
        """Join this competition. See :meth:`zindi.Client.join`."""
        return self._require_client().join(self.id, secret_code=secret_code)

    def download(self, destination: str | Path = ".", **kwargs: Any) -> list[Path]:
        """Download the dataset. See :meth:`zindi.Client.download`."""
        return self._require_client().download(self.id, destination, **kwargs)

    def submit(self, path: str | Path, comment: str = "", **kwargs: Any) -> Submission:
        """Submit a file. See :meth:`zindi.Client.submit`."""
        return self._require_client().submit(path, comment=comment, competition=self.id, **kwargs)

    def leaderboard(self, limit: int | None = None) -> ResultList[LeaderboardEntry]:
        """The leaderboard. See :meth:`zindi.Client.leaderboard`."""
        return self._require_client().leaderboard(self.id, limit=limit)

    def submissions(self) -> ResultList[Submission]:
        """Your submissions. See :meth:`zindi.Client.submissions`."""
        return self._require_client().submissions(self.id)

    def limits(self) -> SubmissionLimits:
        """Remaining submissions. See :meth:`zindi.Client.limits`."""
        return self._require_client().limits(self.id)

    def my_participation(self) -> Participation:
        """Your rank and best score. See :meth:`zindi.Client.my_participation`."""
        return self._require_client().my_participation(self.id)

    def discussions(self, limit: int | None = 20) -> ResultList[Discussion]:
        """Discussion threads. See :meth:`zindi.Client.discussions`."""
        return self._require_client().discussions(self.id, limit=limit)

    def __repr__(self) -> str:
        return f"Competition(id={self.id!r}, title={self.title!r}, status={self.status!r})"


@dataclass(repr=False)
class LeaderboardEntry(_Model):
    """One row of a leaderboard (a user or a team)."""

    rank: int | None
    name: str
    score: float | None = None
    is_team: bool = False
    username: str | None = None
    team_id: str | None = None
    members: list[str] = field(default_factory=list)
    submission_count: int = 0
    last_submission_at: datetime | None = None
    countrycode: str | None = None
    raw: dict[str, Any] = field(default_factory=dict)

    columns = (
        ("Rank", "rank"),
        ("Name", "display_name"),
        ("Score", "score"),
        ("Subs", "submission_count"),
        ("Last submission", "last_submission"),
    )

    @classmethod
    def from_api(cls, raw: Mapping[str, Any]) -> LeaderboardEntry:
        user = raw.get("user") if isinstance(raw.get("user"), Mapping) else None
        team = raw.get("team") if isinstance(raw.get("team"), Mapping) else None
        members = [
            str(m.get("username"))
            for m in (team or {}).get("members") or []
            if isinstance(m, Mapping) and m.get("username")
        ]
        country = (user or {}).get("countrycode")
        if not country and team and team.get("countrycodes"):
            country = team["countrycodes"][0]
        return cls(
            rank=_int(_first(raw, "private_rank", "public_rank")),
            name=str((team or {}).get("title") or (user or {}).get("username") or "?"),
            score=_float(_first(raw, "best_private_score", "best_public_score")),
            is_team=team is not None,
            username=_str((user or {}).get("username")),
            team_id=_str((team or {}).get("id")),
            members=members,
            submission_count=_int(raw.get("submission_count")) or 0,
            last_submission_at=parse_time(
                _first(raw, "best_private_submitted_at", "best_public_submitted_at")
            ),
            countrycode=_str(country),
            raw=dict(raw),
        )

    @property
    def display_name(self) -> str:
        return f"{self.name} (team)" if self.is_team else self.name

    @property
    def last_submission(self) -> str:
        return self.last_submission_at.strftime("%Y-%m-%d %H:%M") if self.last_submission_at else ""

    def includes(self, username: str) -> bool:
        """``True`` if ``username`` is this row's user or a member of this row's team."""
        wanted = username.casefold()
        return (self.username or "").casefold() == wanted or any(
            m.casefold() == wanted for m in self.members
        )

    def __repr__(self) -> str:
        return f"LeaderboardEntry(rank={self.rank}, name={self.name!r}, score={self.score})"


@dataclass(repr=False)
class Participation(_Model):
    """Your own standing in a competition."""

    rank: int | None
    score: float | None = None
    submission_count: int = 0
    team_title: str | None = None
    raw: dict[str, Any] = field(default_factory=dict)

    columns = (
        ("Rank", "rank"),
        ("Score", "score"),
        ("Subs", "submission_count"),
        ("Team", "team_title"),
    )

    @classmethod
    def from_api(cls, raw: Mapping[str, Any]) -> Participation:
        team = raw.get("team") if isinstance(raw.get("team"), Mapping) else {}
        return cls(
            rank=_int(_first(raw, "private_rank", "public_rank")),
            score=_float(_first(raw, "best_private_score", "best_public_score")),
            submission_count=_int(raw.get("submission_count")) or 0,
            team_title=_str((team or {}).get("title")),
            raw=dict(raw),
        )

    def __repr__(self) -> str:
        return f"Participation(rank={self.rank}, score={self.score})"


@dataclass(repr=False)
class Submission(_Model):
    """One of your submissions."""

    id: str
    status: str
    filename: str | None = None
    comment: str | None = None
    created_at: datetime | None = None
    public_score: float | None = None
    private_score: float | None = None
    status_description: str | None = None
    selected: bool = False
    raw: dict[str, Any] = field(default_factory=dict)

    columns = (
        ("Id", "id"),
        ("Date", "date"),
        ("File", "filename"),
        ("Status", "status_label"),
        ("Public", "public_score"),
        ("Private", "private_score"),
        ("Comment", "note"),
    )

    @classmethod
    def from_api(cls, raw: Mapping[str, Any]) -> Submission:
        return cls(
            id=str(raw.get("id", "")),
            status=str(raw.get("status") or "unknown"),
            filename=_str(raw.get("filename")),
            comment=_str(raw.get("comment")),
            created_at=parse_time(raw.get("created_at")),
            public_score=_float(raw.get("public_score")),
            private_score=_float(raw.get("private_score")),
            status_description=_str(raw.get("status_description")),
            selected=bool(_first(raw, "chosen", "selected")),
            raw=dict(raw),
        )

    @property
    def is_failed(self) -> bool:
        return self.status not in ("successful", "initial")

    @property
    def is_scored(self) -> bool:
        return not self.is_failed and (
            self.public_score is not None or self.private_score is not None
        )

    @property
    def is_pending(self) -> bool:
        return not self.is_failed and not self.is_scored

    @property
    def score(self) -> float | None:
        return self.private_score if self.private_score is not None else self.public_score

    @property
    def status_label(self) -> str:
        if self.is_failed:
            return "failed"
        return "scored" if self.is_scored else "scoring…"

    @property
    def date(self) -> str:
        return self.created_at.strftime("%Y-%m-%d %H:%M") if self.created_at else ""

    @property
    def note(self) -> str:
        """The comment, or the error message for a failed submission."""
        return (self.status_description if self.is_failed else self.comment) or ""

    def __repr__(self) -> str:
        return f"Submission(id={self.id!r}, status={self.status_label!r}, score={self.score})"


@dataclass(repr=False)
class SubmissionLimits(_Model):
    """How many submissions you can still make."""

    remaining_today: int | None
    raw: dict[str, Any] = field(default_factory=dict)

    columns = (("Remaining today", "remaining_today"),)

    @classmethod
    def from_api(cls, raw: Mapping[str, Any]) -> SubmissionLimits:
        return cls(remaining_today=_int(_first(raw, "today", "remaining_today")), raw=dict(raw))

    def __repr__(self) -> str:
        return f"SubmissionLimits(remaining_today={self.remaining_today})"


@dataclass(repr=False)
class Team(_Model):
    """Your team in a competition."""

    id: str | None
    title: str
    leader: str | None = None
    members: list[str] = field(default_factory=list)
    pending_invitations: list[str] = field(default_factory=list)
    raw: dict[str, Any] = field(default_factory=dict)

    columns = (("Team", "title"), ("Leader", "leader"), ("Members", "members_text"))

    @classmethod
    def from_api(cls, raw: Mapping[str, Any]) -> Team:
        def names(items: Any) -> list[str]:
            out = []
            for item in items or []:
                if isinstance(item, Mapping) and item.get("username"):
                    out.append(str(item["username"]))
                elif isinstance(item, str):
                    out.append(item)
            return out

        leader = raw.get("leader")
        return cls(
            id=_str(raw.get("id")),
            title=str(raw.get("title") or ""),
            leader=_str(
                raw.get("leader_username")
                or (leader.get("username") if isinstance(leader, Mapping) else leader)
            ),
            members=names(raw.get("users") or raw.get("members")),
            pending_invitations=names(raw.get("invited_users") or raw.get("invitations")),
            raw=dict(raw),
        )

    @property
    def members_text(self) -> str:
        return ", ".join(self.members)

    def __repr__(self) -> str:
        return f"Team(title={self.title!r}, members={self.members!r})"


@dataclass(repr=False)
class Discussion(_Model):
    """A discussion thread."""

    id: str
    title: str
    author: str | None = None
    created_at: datetime | None = None
    comments_count: int = 0
    pinned: bool = False
    competition: str | None = None
    raw: dict[str, Any] = field(default_factory=dict)

    columns = (
        ("Title", "title"),
        ("Author", "author"),
        ("Date", "date"),
        ("Replies", "comments_count"),
    )

    @classmethod
    def from_api(cls, raw: Mapping[str, Any], competition: str | None = None) -> Discussion:
        user = raw.get("user") if isinstance(raw.get("user"), Mapping) else {}
        return cls(
            id=str(raw.get("id", "")),
            title=str(raw.get("title") or ""),
            author=_str((user or {}).get("username")),
            created_at=parse_time(_first(raw, "published_at", "created_at")),
            comments_count=_int(raw.get("comments_count")) or 0,
            pinned=bool(raw.get("pinned", False)),
            competition=competition,
            raw=dict(raw),
        )

    @property
    def date(self) -> str:
        return self.created_at.strftime("%Y-%m-%d") if self.created_at else ""

    @property
    def url(self) -> str:
        slug = self.raw.get("url_title")
        if self.competition and slug:
            return f"{WEB_URL}/competitions/{self.competition}/discussions/{self.id}/{slug}"
        if self.competition:
            return f"{WEB_URL}/competitions/{self.competition}/discussions/{self.id}"
        return f"{WEB_URL}/discussions/{self.id}"

    def __repr__(self) -> str:
        return f"Discussion(id={self.id!r}, title={self.title!r})"
