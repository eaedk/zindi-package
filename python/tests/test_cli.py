from __future__ import annotations

import json
from pathlib import Path

from typer.testing import CliRunner

from zindi import config
from zindi.cli import app

runner = CliRunner()
COMP = "Crop-Yield-Forecast-Challenge"


def run(*args: str, input: str | None = None):
    return runner.invoke(app, list(args), input=input, catch_exceptions=False)


def test_version():
    result = run("--version")
    assert result.exit_code == 0 and result.output.startswith("zindi ")


def test_no_args_shows_help():
    result = run()
    assert "competitions" in result.output and "submit" in result.output


def test_competitions_table_and_json(api, fixture):
    api.get("/competitions").respond(json=fixture("competitions"))
    table = run("competitions")
    assert table.exit_code == 0 and COMP in table.output
    as_json = run("competitions", "--all", "--json")
    data = json.loads(as_json.stdout)
    assert [c["id"] for c in data][-1] == "Air-Quality-Lagos"


def test_bad_option_value_is_rejected():
    result = runner.invoke(app, ["competitions", "--status", "soon"])
    assert result.exit_code != 0
    assert "must be one of" in result.output


def test_show_json(api, fixture):
    api.get(f"/competitions/{COMP}").respond(json=fixture("competition"))
    data = json.loads(run("show", COMP, "--json").stdout)
    assert data["metric"] == "RMSE"
    assert data["data_files"][0]["filename"] == "Train.csv"


def test_use_then_commands_need_no_id(api, fixture):
    api.get(f"/competitions/{COMP}").respond(json=fixture("competition"))
    api.get(f"/competitions/{COMP}/participations").respond(json=fixture("leaderboard"))
    assert run("use", COMP).exit_code == 0
    assert config.read_project_competition() == COMP
    result = run("leaderboard")
    assert result.exit_code == 0 and "kofi_ml" in result.output


def test_missing_competition_is_a_clear_error():
    result = runner.invoke(app, ["leaderboard"])
    assert result.exit_code == 1
    assert "zindi use" in result.output


def test_login_with_password_stdin(api, fixture):
    api.post("/auth/signin").respond(json=fixture("signin_ok"))
    result = run("login", "-u", "amina_dev", "--password-stdin", input="hunter2\n")
    assert result.exit_code == 0, result.output
    assert config.load_credentials().auth_token == "tok_3f9a1c2e7b"  # type: ignore[union-attr]


def test_login_with_token_and_whoami(api, fixture):
    api.get("/me").respond(json=fixture("me"))
    assert run("login", "--token", "tok_x").exit_code == 0
    result = run("whoami", "--json")
    data = json.loads(result.stdout)
    assert data["username"] == "amina_dev"
    assert data["credentials_source"].endswith("credentials.json")


def test_whoami_when_logged_out():
    result = runner.invoke(app, ["whoami"])
    assert result.exit_code == 2 and "zindi login" in result.output


def test_logout():
    config.save_credentials("a", "b")
    assert "Logged out" in run("logout").output
    assert "not logged in" in run("logout").output


def test_join_and_submit_flow(api, fixture, tmp_path):
    config.save_credentials("amina_dev", "tok")
    api.post(f"/competitions/{COMP}/participations").respond(json=fixture("join_ok"))
    api.post(f"/competitions/{COMP}/submissions").respond(json=fixture("submission_created"))
    api.get(f"/competitions/{COMP}/submissions").respond(json=fixture("submission_scored"))
    api.get(f"/competitions/{COMP}/submissions/limits").respond(json=fixture("submission_limits"))
    assert "Joined" in run("join", COMP).output
    sub = tmp_path / "sub.csv"
    sub.write_text("ID,Yield\n")
    result = run("submit", str(sub), "-c", COMP, "-m", "first try", "--wait")
    assert result.exit_code == 0, result.output
    assert "sub_d4" in result.output and "0.4211" in result.output
    assert "7 submission(s) left today" in result.output


def test_download(api, fixture, tmp_path):
    config.save_credentials("amina_dev", "tok")
    detail = fixture("competition")
    detail["data"]["datafiles"] = [{"filename": "Train.csv", "filesize": 3}]
    api.get(f"/competitions/{COMP}").respond(json=detail)
    api.get(f"/competitions/{COMP}/files/Train.csv").respond(content=b"abc")
    result = run("download", COMP, "-o", str(tmp_path / "d"))
    assert result.exit_code == 0, result.output
    assert (tmp_path / "d" / "Train.csv").read_bytes() == b"abc"


def test_team_commands(api, fixture):
    config.save_credentials("amina_dev", "tok")
    api.post(f"/competitions/{COMP}/my_team").respond(json=fixture("team_created"))
    api.post(f"/competitions/{COMP}/my_team/invite").respond(json=fixture("team_invite_ok"))
    api.delete(f"/competitions/{COMP}/my_team").respond(json=fixture("team_disbanded"))
    created = run("team", "create", "Lagos Learners", "-i", "zawadi_k", "-c", COMP)
    assert "created" in created.output and "invited zawadi_k" in created.output
    assert "disbanded" in run("team", "disband", COMP, "--yes").output


def test_rank_and_limits(api, fixture):
    config.save_credentials("amina_dev", "tok")
    api.get(f"/competitions/{COMP}/participations/my_participation").respond(
        json=fixture("my_participation")
    )
    api.get(f"/competitions/{COMP}/submissions/limits").respond(json=fixture("submission_limits"))
    assert "#2" in run("rank", COMP).output
    assert "7" in run("limits", COMP).output


def test_doctor_runs_offline(api):
    api.get("/competitions").respond(json={"data": [], "meta": {"total_count": 0}})
    result = run("doctor")
    assert result.exit_code == 0
    assert "Config folder" in result.output and "Credentials" in result.output


def test_python_dash_m_entry_point():
    import runpy

    assert Path(runpy.__file__).exists()
    import zindi.__main__  # noqa: F401  (importing must not run the CLI)
