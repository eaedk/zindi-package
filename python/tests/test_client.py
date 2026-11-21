from __future__ import annotations

import io
import json
import zipfile
from pathlib import Path

import httpx
import pytest
import respx

import zindi
from zindi import (
    APIError,
    AuthenticationError,
    Client,
    CompetitionNotSetError,
    NotFoundError,
    SecretCodeRequiredError,
    ZindiError,
)

from .conftest import API

COMP = "Crop-Yield-Forecast-Challenge"


# -- competitions ----------------------------------------------------------------------------


def test_list_open_competitions_without_login(api, anon, fixture):
    route = api.get("/competitions").respond(json=fixture("competitions"))

    comps = anon.competitions()

    assert [c.id for c in comps] == [COMP, "Swahili-ASR-Hackathon"]
    params = route.calls.last.request.url.params
    assert params["active"] == "true"
    assert "auth-token" not in route.calls.last.request.headers
    assert comps[0].client is anon


def test_competition_filters_are_sent_and_enforced(api, anon, fixture):
    route = api.get("/competitions").respond(json=fixture("competitions"))

    comps = anon.competitions(status="all", kind="hackathon", reward="knowledge", search="asr")

    params = route.calls.last.request.url.params
    assert params["kind[]"] == "hackathon"
    assert params["prize"] == "knowledge"
    assert params["query"] == "asr"
    assert "active" not in params
    assert [c.id for c in comps] == ["Swahili-ASR-Hackathon"]


def test_closed_status_and_limit(api, anon, fixture):
    api.get("/competitions").respond(json=fixture("competitions"))
    assert [c.id for c in anon.competitions(status="closed")] == ["Air-Quality-Lagos"]
    assert len(anon.competitions(status="all", limit=1)) == 1


def test_competitions_paginate_until_total(api, anon, fixture):
    data = fixture("competitions")
    first = {"data": data["data"][:2] * 25, "meta": {"total_count": 51}}
    second = {"data": data["data"][2:], "meta": {"total_count": 51}}
    route = api.get("/competitions").mock(
        side_effect=[httpx.Response(200, json=first), httpx.Response(200, json=second)]
    )

    comps = anon.competitions(status="all", limit=None)

    assert len(comps) == 51
    assert [c.request.url.params["page"] for c in route.calls] == ["0", "1"]


def test_competition_details(api, anon, fixture):
    api.get(f"/competitions/{COMP}").respond(json=fixture("competition"))

    comp = anon.competition(COMP)

    assert comp.title == "Crop Yield Forecast Challenge"
    assert comp.metric == "RMSE"
    assert comp.max_team_size == 4
    assert comp.daily_submission_limit == 10
    assert [f.filename for f in comp.data_files][:2] == ["Train.csv", "Test.csv"]
    assert comp.data_files[0].size_human == "1.0 MB"
    assert "maize" in comp.description
    assert "10 submissions per day" in comp.rules
    assert comp.url == f"https://zindi.world/competitions/{COMP}"
    assert comp.end_time is not None and comp.end_time.year == 2026


def test_missing_competition_raises_not_found(api, anon):
    api.get("/competitions/nope").respond(404, json={"data": {"errors": {"message": "not found"}}})
    with pytest.raises(NotFoundError):
        anon.competition("nope")


def test_competition_id_resolution_order(api, anon, fixture, monkeypatch, tmp_path):
    with pytest.raises(CompetitionNotSetError):
        anon.resolve_competition()
    (Path.cwd() / "zindi.toml").write_text('competition = "From-File"\n')
    assert anon.resolve_competition() == "From-File"
    monkeypatch.setenv("ZINDI_COMPETITION", "From-Env")
    assert anon.resolve_competition() == "From-Env"
    anon.use("From-Use")
    assert anon.resolve_competition() == "From-Use"
    assert anon.resolve_competition("Explicit") == "Explicit"


def test_use_with_save_writes_project_file(anon):
    anon.use(COMP, save=True)
    assert (Path.cwd() / "zindi.toml").read_text() == f'competition = "{COMP}"\n'


# -- joining ---------------------------------------------------------------------------------


def test_join_sends_token_and_reports_new_participation(api, client, fixture):
    route = api.post(f"/competitions/{COMP}/participations").respond(json=fixture("join_ok"))
    assert client.join(COMP) is True
    request = route.calls.last.request
    assert request.headers["auth-token"] == "tok_test"
    assert request.content == b""


def test_join_when_already_in_returns_false(api, client, fixture):
    api.post(f"/competitions/{COMP}/participations").respond(json=fixture("join_already"))
    assert client.join(COMP) is False


def test_join_private_competition(api, client, fixture):
    route = api.post(f"/competitions/{COMP}/participations")
    route.respond(json=fixture("join_secret_required"))
    with pytest.raises(SecretCodeRequiredError):
        client.join(COMP)
    route.respond(json=fixture("join_ok"))
    assert client.join(COMP, secret_code="s3cret") is True
    assert json.loads(route.calls.last.request.content) == {"secret_code": "s3cret"}


# -- downloads -------------------------------------------------------------------------------


def _detail_with(fixture, files):
    detail = fixture("competition")
    detail["data"]["datafiles"] = files
    return detail


def test_download_skips_everything_archive_and_existing_files(api, client, fixture, tmp_path):
    api.get(f"/competitions/{COMP}").respond(json=fixture("competition"))
    train = api.get(f"/competitions/{COMP}/files/Train.csv").respond(content=b"a" * 1048576)
    api.get(f"/competitions/{COMP}/files/Test.csv").respond(content=b"b" * 262144)
    api.get(f"/competitions/{COMP}/files/SampleSubmission.csv").respond(content=b"c" * 2048)
    archive = api.get(f"/competitions/{COMP}/files/all_data.zip").respond(content=b"zip")

    paths = client.download(COMP, tmp_path / "data")

    assert [p.name for p in paths] == ["Train.csv", "Test.csv", "SampleSubmission.csv"]
    assert (tmp_path / "data" / "Train.csv").stat().st_size == 1048576
    assert not archive.called
    assert not list((tmp_path / "data").glob("*.part"))

    client.download(COMP, tmp_path / "data")
    assert train.call_count == 1  # second run: already there, same size


def test_download_selected_files_and_overwrite(api, client, fixture, tmp_path):
    api.get(f"/competitions/{COMP}").respond(json=fixture("competition"))
    route = api.get(f"/competitions/{COMP}/files/SampleSubmission.csv").respond(content=b"c" * 2048)

    client.download(COMP, tmp_path, files=["SampleSubmission.csv"])
    client.download(COMP, tmp_path, files=["SampleSubmission.csv"], overwrite=True)

    assert route.call_count == 2
    with pytest.raises(ZindiError, match="Unknown file"):
        client.download(COMP, tmp_path, files=["nope.csv"])


def test_download_follows_redirect_without_leaking_token(client, fixture, tmp_path):
    detail = _detail_with(fixture, [{"filename": "Train.csv", "filesize": 4}])
    signed_url = "https://storage.example.com/signed/Train.csv"
    with respx.mock(assert_all_called=True) as router:
        router.get(f"{API}/competitions/{COMP}").respond(json=detail)
        router.get(f"{API}/competitions/{COMP}/files/Train.csv").respond(
            302, headers={"Location": signed_url}
        )
        signed = router.get(signed_url).respond(content=b"data")
        client.download(COMP, tmp_path)
    assert (tmp_path / "Train.csv").read_bytes() == b"data"
    assert "auth-token" not in signed.calls.last.request.headers


def test_download_refused_suggests_joining(api, client, fixture, tmp_path):
    api.get(f"/competitions/{COMP}").respond(
        json=_detail_with(fixture, [{"filename": "Train.csv", "filesize": 4}])
    )
    api.get(f"/competitions/{COMP}/files/Train.csv").respond(
        401, json={"data": {"errors": {"message": "not authorized"}}}
    )
    with pytest.raises(AuthenticationError, match="zindi join"):
        client.download(COMP, tmp_path)
    assert not (tmp_path / "Train.csv").exists()
    assert not (tmp_path / "Train.csv.part").exists()


def test_download_unzip(api, client, fixture, tmp_path):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as zf:
        zf.writestr("images/a.txt", "hello")
    payload = buffer.getvalue()
    api.get(f"/competitions/{COMP}").respond(
        json=_detail_with(fixture, [{"filename": "images.zip", "filesize": len(payload)}])
    )
    api.get(f"/competitions/{COMP}/files/images.zip").respond(content=payload)

    client.download(COMP, tmp_path, unzip=True)

    assert (tmp_path / "images" / "a.txt").read_text() == "hello"


def test_unzip_refuses_path_traversal(api, client, fixture, tmp_path):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as zf:
        zf.writestr("../evil.txt", "boom")
    payload = buffer.getvalue()
    api.get(f"/competitions/{COMP}").respond(
        json=_detail_with(fixture, [{"filename": "evil.zip", "filesize": len(payload)}])
    )
    api.get(f"/competitions/{COMP}/files/evil.zip").respond(content=payload)
    with pytest.raises(ZindiError, match="unsafe"):
        client.download(COMP, tmp_path / "data", unzip=True)
    assert not (tmp_path / "evil.txt").exists()


# -- submissions -----------------------------------------------------------------------------


def test_submit_uploads_file_and_comment(api, client, fixture, tmp_path):
    route = api.post(f"/competitions/{COMP}/submissions").respond(
        json=fixture("submission_created")
    )
    path = tmp_path / "submission.csv"
    path.write_text("ID,Yield\n1,2.5\n")

    sub = client.submit(path, comment="first try", competition=COMP)

    assert sub.id == "sub_d4"
    assert sub.is_pending
    body = route.calls.last.request.content
    assert b'name="comment"' in body and b"first try" in body
    assert b'filename="submission.csv"' in body and b"ID,Yield" in body


def test_submit_wait_polls_until_scored(api, client, fixture, tmp_path):
    api.post(f"/competitions/{COMP}/submissions").respond(json=fixture("submission_created"))
    pending = {"data": [fixture("submission_created")["data"]]}
    history = api.get(f"/competitions/{COMP}/submissions").mock(
        side_effect=[
            httpx.Response(200, json=pending),
            httpx.Response(200, json=fixture("submission_scored")),
        ]
    )
    path = tmp_path / "submission.csv"
    path.write_text("x")

    sub = client.submit(path, competition=COMP, wait=True, poll_interval=0)

    assert sub.is_scored and sub.public_score == pytest.approx(0.4211)
    assert history.call_count == 2


def test_wait_for_score_times_out(api, client, fixture):
    api.get(f"/competitions/{COMP}/submissions").respond(
        json={"data": [fixture("submission_created")["data"]]}
    )
    with pytest.raises(ZindiError, match="not scored"):
        client.wait_for_score("sub_d4", competition=COMP, timeout=0)


def test_submit_missing_file(client, tmp_path):
    with pytest.raises(ZindiError, match="not found"):
        client.submit(tmp_path / "nope.csv", competition=COMP)


def test_submissions_and_statuses(api, client, fixture):
    api.get(f"/competitions/{COMP}/submissions").respond(json=fixture("submissions"))

    subs = client.submissions(COMP)

    assert [s.status_label for s in subs] == ["scoring…", "scored", "failed"]
    assert subs[2].note == "Missing column: Yield"
    assert subs[1].score == pytest.approx(0.4377)
    assert len(client.submissions(COMP, limit=1)) == 1


def test_limits(api, client, fixture):
    api.get(f"/competitions/{COMP}/submissions/limits").respond(json=fixture("submission_limits"))
    assert client.limits(COMP).remaining_today == 7


# -- leaderboard -----------------------------------------------------------------------------


def test_leaderboard_stops_at_unranked_rows(api, anon, fixture):
    api.get(f"/competitions/{COMP}/participations").respond(json=fixture("leaderboard"))

    board = anon.leaderboard(COMP)

    assert [e.rank for e in board] == [1, 2, 3]
    team = board[1]
    assert team.is_team and team.name == "Lagos Learners" and team.includes("AMINA_DEV")
    assert board.total == 4
    assert len(anon.leaderboard(COMP, include_unranked=True)) == 4
    assert len(anon.leaderboard(COMP, limit=2)) == 2


def test_rank_from_my_participation(api, client, fixture):
    api.get(f"/competitions/{COMP}/participations/my_participation").respond(
        json=fixture("my_participation")
    )
    assert client.rank(COMP) == 2
    assert client.my_participation(COMP).team_title == "Lagos Learners"


def test_rank_falls_back_to_leaderboard_search(api, fixture):
    api.get(f"/competitions/{COMP}/participations/my_participation").respond(
        json={"data": {"public_rank": None}}
    )
    api.get(f"/competitions/{COMP}/participations").respond(json=fixture("leaderboard"))
    with Client(token="t", username="zawadi_k", progress=False) as c:
        assert c.rank(COMP) == 3


# -- teams -----------------------------------------------------------------------------------


def test_team_lifecycle(api, client, fixture):
    create = api.post(f"/competitions/{COMP}/my_team").respond(json=fixture("team_created"))
    invite = api.post(f"/competitions/{COMP}/my_team/invite")
    invite.mock(
        side_effect=[
            httpx.Response(200, json=fixture("team_invite_ok")),
            httpx.Response(200, json=fixture("team_invite_already")),
        ]
    )
    api.get(f"/competitions/{COMP}/my_team").respond(json=fixture("my_team"))
    disband = api.delete(f"/competitions/{COMP}/my_team").respond(json=fixture("team_disbanded"))

    team = client.create_team("Lagos Learners", competition=COMP, invite=["zawadi_k", "tolu_a"])

    assert team.title == "Lagos Learners" and team.leader == "amina_dev"
    assert create.calls.last.request.content == b"title=Lagos+Learners"
    assert invite.call_count == 2
    shown = client.team(COMP)
    assert shown is not None and shown.members == ["amina_dev", "tolu_a"]
    assert shown.pending_invitations == ["zawadi_k"]
    client.disband_team(COMP)
    assert disband.called


def test_team_none_when_solo(api, client):
    api.get(f"/competitions/{COMP}/my_team").respond(
        404, json={"data": {"errors": {"message": "not found"}}}
    )
    assert client.team(COMP) is None


def test_invite_other_error_is_raised(api, client):
    api.post(f"/competitions/{COMP}/my_team/invite").respond(
        json={"data": {"errors": {"base": "User does not exist"}}}
    )
    with pytest.raises(APIError, match="does not exist"):
        client.invite("ghost", competition=COMP)


# -- users and discussions -------------------------------------------------------------------


def test_user_profile(api, anon, fixture):
    api.get("/users/amina_dev").respond(json=fixture("user_profile"))
    user = anon.user("amina_dev")
    assert user.full_name == "Amina Diallo"
    assert (user.gold_medals, user.silver_medals, user.bronze_medals) == (1, 2, 4)
    assert user.url == "https://zindi.world/users/amina_dev"


def test_me_requires_login(anon):
    with pytest.raises(AuthenticationError, match="zindi login"):
        anon.me()


def test_discussions(api, anon, fixture):
    api.get(f"/competitions/{COMP}/discussions").respond(json=fixture("discussions"))
    threads = anon.discussions(COMP)
    assert threads[0].author == "zawadi_k"
    assert threads[0].date == "2026-11-15"
    assert threads[0].url.startswith(f"https://zindi.world/competitions/{COMP}/discussions/7001")


# -- results ---------------------------------------------------------------------------------


def test_result_list_renders_and_converts(api, anon, fixture):
    api.get(f"/competitions/{COMP}/participations").respond(json=fixture("leaderboard"))
    board = anon.leaderboard(COMP)

    html = board._repr_html_()
    assert "<table" in html and "Lagos Learners" in html
    records = board.to_records()
    assert records[0]["name"] == "kofi_ml" and "raw" not in records[0]
    frame = board.to_pandas()
    assert list(frame["rank"]) == [1, 2, 3]


def test_public_api_surface():
    assert zindi.__version__
    for name in zindi.__all__:
        assert hasattr(zindi, name), name
