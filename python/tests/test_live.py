"""Read-only checks against the real Zindi API.

They are skipped unless ``ZINDI_LIVE=1``. Endpoints that need an account also need
``ZINDI_TOKEN`` (or ``ZINDI_USERNAME``/``ZINDI_PASSWORD``, or a saved ``zindi login``).
Nothing here joins a competition, submits or changes a team.

    ZINDI_LIVE=1 uv run pytest -m live -v
"""

from __future__ import annotations

import os

import pytest

from zindi import Client

pytestmark = [
    pytest.mark.live,
    pytest.mark.skipif(not os.environ.get("ZINDI_LIVE"), reason="set ZINDI_LIVE=1 to run"),
]


@pytest.fixture(scope="module")
def live() -> Client:
    return Client(progress=False)


@pytest.fixture(scope="module")
def some_competition(live: Client) -> str:
    comps = live.competitions(status="all", limit=5)
    assert comps, "the API returned no competition"
    return os.environ.get("ZINDI_LIVE_COMPETITION") or comps[0].id


def test_competitions_have_the_expected_shape(live: Client) -> None:
    for comp in live.competitions(status="all", limit=10):
        assert comp.id
        assert comp.title
        assert comp.kind in ("competition", "hackathon", None)


def test_competition_details(live: Client, some_competition: str) -> None:
    comp = live.competition(some_competition)
    assert comp.id == some_competition
    assert isinstance(comp.data_files, list)


def test_leaderboard(live: Client, some_competition: str) -> None:
    board = live.leaderboard(some_competition, limit=5)
    ranks = [row.rank for row in board]
    assert ranks == sorted(ranks)


def test_discussions(live: Client, some_competition: str) -> None:
    live.discussions(some_competition, limit=3)


def test_user_profile(live: Client) -> None:
    assert live.user("eaedk").username.lower() == "eaedk"


@pytest.fixture(scope="module")
def authed(live: Client) -> Client:
    if not live.is_authenticated:
        pytest.skip("no credentials available")
    return live


def test_me(authed: Client) -> None:
    assert authed.me().username


def test_participant_endpoints(authed: Client) -> None:
    joined = os.environ.get("ZINDI_LIVE_JOINED_COMPETITION")
    if not joined:
        pytest.skip("set ZINDI_LIVE_JOINED_COMPETITION to a competition you joined")
    assert authed.limits(joined).raw is not None
    authed.submissions(joined)
    authed.my_participation(joined)
