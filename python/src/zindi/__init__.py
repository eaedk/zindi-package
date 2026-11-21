"""zindi: take part in Zindi competitions from Python, the terminal or a notebook.

Quick start::

    import zindi

    client = zindi.Client()                       # finds your credentials by itself
    client.competitions(status="open")            # browse
    comp = client.competition("Crop-Yield-Forecast-Challenge")
    comp.join()
    comp.download("data")
    comp.submit("submission.csv", comment="baseline", wait=True)
    comp.leaderboard(limit=10)

The same features are available from the command line: run ``zindi --help``.
"""

from __future__ import annotations

from ._version import __version__
from .client import Client
from .errors import (
    AlreadyJoinedError,
    APIError,
    AuthenticationError,
    CompetitionNotSetError,
    NotFoundError,
    RateLimitError,
    SecretCodeRequiredError,
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

__all__ = [
    "APIError",
    "AlreadyJoinedError",
    "AuthenticationError",
    "Client",
    "Competition",
    "CompetitionNotSetError",
    "DataFile",
    "Discussion",
    "LeaderboardEntry",
    "NotFoundError",
    "Participation",
    "RateLimitError",
    "ResultList",
    "SecretCodeRequiredError",
    "Submission",
    "SubmissionLimits",
    "Team",
    "User",
    "ZindiError",
    "__version__",
    "explore",
    "login",
]


def login(username: str | None = None, password: str | None = None, *, save: bool = True) -> Client:
    """Log in and return a ready-to-use :class:`Client`.

    Prompts for whatever is missing. In a notebook this is the one-liner to start with::

        client = zindi.login()
    """
    client = Client(use_saved_credentials=False)
    client.login(username, password, save=save)
    return client


def explore(client: Client | None = None) -> object:
    """Open the interactive competition explorer in a Jupyter or Colab notebook.

    Needs ``ipywidgets`` (``pip install 'zindi[notebook]'``; already present on Colab).
    """
    from .notebook import Explorer

    return Explorer(client or Client()).show()
