"""Backward compatibility with zindi 0.x (``from zindi.user import Zindian``).

Notebooks written for the first version of this package keep working::

    from zindi.user import Zindian
    user = Zindian(username="amina_dev")
    user.select_a_challenge(fixed_index=0)
    user.download_dataset(destination="dataset")
    user.submit(filepaths=["dataset/SampleSubmission.csv"], comments=["first try"])

New code should use :class:`zindi.Client`, which returns data instead of printing it.
"""

from __future__ import annotations

import warnings
from collections.abc import Sequence
from typing import Any

from rich.console import Console

from .client import Client
from .errors import CompetitionNotSetError
from .models import Competition, ResultList

__all__ = ["Zindian"]

_console = Console()


def _say(ok: bool, message: str) -> None:
    _console.print(f"[{'green' if ok else 'red'}]●[/] {message}")


class Zindian:
    """Legacy account manager. Prefer :class:`zindi.Client` in new code."""

    def __init__(self, username: str, fixed_password: str | None = None) -> None:
        warnings.warn(
            "zindi.user.Zindian is the 0.x interface; use zindi.Client instead.",
            FutureWarning,
            stacklevel=2,
        )
        self._client = Client(
            username=username, password=fixed_password, use_saved_credentials=False
        )
        self._client.login(username, fixed_password, save=False)
        self._challenge: Competition | None = None
        _say(True, f"Welcome {self._client.username} 👋🏾")

    @property
    def client(self) -> Client:
        """The modern client behind this object."""
        return self._client

    def _cid(self) -> str:
        if self._challenge is None:
            raise CompetitionNotSetError()
        return self._challenge.id

    # -- 0.x API ---------------------------------------------------------------------------

    def select_a_challenge(
        self,
        reward: str = "all",
        kind: str = "competition",
        active: Any = "all",
        fixed_index: int | None = None,
    ) -> None:
        """Pick a competition from a list (interactively, or with ``fixed_index``), then join it."""
        status = "open" if active is True or str(active).lower() in ("true", "1") else "all"
        comps: ResultList[Competition] = self._client.competitions(
            status=status,  # type: ignore[arg-type]
            kind=kind if kind in ("competition", "hackathon") else None,  # type: ignore[arg-type]
            reward=reward if reward in ("prize", "points", "knowledge") else None,  # type: ignore[arg-type]
            limit=None,
        )
        if not comps:
            _say(False, "No competition matches these filters.")
            return
        if fixed_index is None:
            for i, comp in enumerate(comps):
                _console.print(f"[bold]{i:>3}[/]  {comp.id}  [dim]{comp.reward or ''}[/]")
            while True:
                answer = input("Type the index of the challenge, or 'q' to quit.\n>> ").strip()
                if answer.lower() == "q":
                    return
                if answer.isdigit() and int(answer) < len(comps):
                    fixed_index = int(answer)
                    break
                _say(False, "Please enter a valid index.")
        if not 0 <= fixed_index < len(comps):
            raise ValueError(f"fixed_index must be in range(0, {len(comps)})")
        self._challenge = comps[fixed_index]
        self._client.use(self._challenge.id)
        _say(True, f"You chose {self._challenge.id}")
        self._client.join(self._challenge.id)

    @property
    def which_challenge(self) -> str | None:
        if self._challenge is None:
            _say(False, "You have not selected any challenge yet.")
            return None
        _say(True, f"Current challenge: {self._challenge.id}")
        return self._challenge.id

    def leaderboard(self, to_print: bool = True) -> None:
        board = self._client.leaderboard(self._cid())
        if to_print:
            from ._display import rich_table

            _console.print(rich_table(board, highlight=self._client.username))

    @property
    def my_rank(self) -> int:
        rank = self._client.rank(self._cid()) or 0
        _say(True, f"Your rank: {rank or 'not ranked yet'}")
        return rank

    @property
    def remaining_submissions(self) -> int | None:
        remaining = self._client.limits(self._cid()).remaining_today
        _say(True, f"Remaining submissions today: {remaining}")
        return remaining

    remaining_subimissions = remaining_submissions  # historic spelling

    def submission_board(self, to_print: bool = True) -> None:
        subs = self._client.submissions(self._cid())
        if to_print:
            _console.print(subs)

    def download_dataset(self, destination: str = ".", make_destination: bool = True) -> None:
        self._client.download(self._cid(), destination)

    def submit(self, filepaths: Sequence[str] = (), comments: Sequence[str] = ()) -> None:
        notes = list(comments) + [""] * (len(filepaths) - len(comments))
        for path, comment in zip(filepaths, notes, strict=False):
            sub = self._client.submit(path, comment=comment, competition=self._cid())
            _say(True, f"Submission ID: {sub.id} - file submitted: {path}")

    def create_team(self, team_name: str, teammates: Sequence[str] = ()) -> None:
        team = self._client.create_team(team_name, competition=self._cid(), invite=teammates)
        _say(True, f"Team created: {team.title}")

    def team_up(self, zindians: Sequence[str] = ()) -> None:
        for name in self._client.invite(list(zindians), competition=self._cid()):
            _say(True, f"Invitation sent to {name}")

    def disband_team(self) -> None:
        self._client.disband_team(self._cid())
        _say(True, "Team disbanded")
