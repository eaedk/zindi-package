from __future__ import annotations

import json
import os
import stat
import sys

import httpx
import pytest

from zindi import APIError, AuthenticationError, Client, RateLimitError, config
from zindi._http import error_message


def test_login_saves_token_but_never_password(api, anon, fixture):
    route = api.post("/auth/signin").respond(json=fixture("signin_ok"))

    user = anon.login("amina_dev", "hunter2")

    assert user.username == "amina_dev"
    assert anon.token == "tok_3f9a1c2e7b"
    assert route.calls.last.request.content == b"username=amina_dev&password=hunter2"
    saved = json.loads(config.credentials_path().read_text())
    assert saved["auth_token"] == "tok_3f9a1c2e7b"
    assert "hunter2" not in config.credentials_path().read_text()
    if os.name != "nt":
        assert stat.S_IMODE(config.credentials_path().stat().st_mode) == 0o600


def test_wrong_password_points_to_reset_page(api, anon, fixture):
    api.post("/auth/signin").respond(json=fixture("signin_wrong_password"))
    with pytest.raises(AuthenticationError, match="passwords/request"):
        anon.login("amina_dev", "nope")
    assert not config.credentials_path().exists()


def test_saved_credentials_are_used(api, fixture):
    config.save_credentials("amina_dev", "tok_saved")
    route = api.get("/me").respond(json=fixture("me"))
    with Client(progress=False) as c:
        assert c.me().username == "amina_dev"
        assert c.credentials_source == str(config.credentials_path())
    assert route.calls.last.request.headers["auth-token"] == "tok_saved"


def test_env_token_beats_saved_file(api, fixture, monkeypatch):
    config.save_credentials("someone", "tok_saved")
    monkeypatch.setenv("ZINDI_TOKEN", "tok_env")
    route = api.get("/me").respond(json=fixture("me"))
    with Client(progress=False) as c:
        c.me()
        assert c.credentials_source == "environment variables"
    assert route.calls.last.request.headers["auth-token"] == "tok_env"


def test_env_username_password_signs_in_lazily(api, fixture, monkeypatch):
    monkeypatch.setenv("ZINDI_USERNAME", "amina_dev")
    monkeypatch.setenv("ZINDI_PASSWORD", "hunter2")
    signin = api.post("/auth/signin").respond(json=fixture("signin_ok"))
    api.get("/me").respond(json=fixture("me"))
    with Client(progress=False) as c:
        assert not signin.called
        c.me()
    assert signin.call_count == 1


def test_expired_token_is_refreshed_once(api, fixture):
    signin = api.post("/auth/signin").respond(json=fixture("signin_ok"))
    me = api.get("/me").mock(
        side_effect=[
            httpx.Response(401, json=fixture("error_not_authorized")),
            httpx.Response(200, json=fixture("me")),
        ]
    )
    with Client(token="old", username="amina_dev", password="hunter2", progress=False) as c:
        assert c.me().username == "amina_dev"
        assert c.token == "tok_3f9a1c2e7b"
    assert signin.call_count == 1 and me.call_count == 2


def test_not_authorized_in_200_envelope(api, client, fixture):
    api.get("/me").respond(200, json=fixture("error_not_authorized"))
    with pytest.raises(AuthenticationError):
        client.me()


def test_logout_forgets_everything(api, anon, fixture):
    api.post("/auth/signin").respond(json=fixture("signin_ok"))
    anon.login("amina_dev", "hunter2")
    anon.logout()
    assert anon.token is None
    assert not config.credentials_path().exists()


def test_login_without_terminal_needs_values(anon):
    with pytest.raises(AuthenticationError, match="username"):
        anon.login()


def test_colab_secrets(monkeypatch, api, fixture):
    import types

    userdata = types.SimpleNamespace(
        get=lambda key: {"ZINDI_TOKEN": "tok_colab"}.get(key) or _raise()
    )
    colab = types.ModuleType("google.colab")
    colab.userdata = userdata  # type: ignore[attr-defined]
    google = types.ModuleType("google")
    google.colab = colab  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "google", google)
    monkeypatch.setitem(sys.modules, "google.colab", colab)
    route = api.get("/me").respond(json=fixture("me"))
    with Client(progress=False) as c:
        c.me()
        assert c.credentials_source == "Colab secrets"
    assert route.calls.last.request.headers["auth-token"] == "tok_colab"


def _raise() -> None:
    raise KeyError("secret not found")


# -- transport -------------------------------------------------------------------------------


def test_retries_transient_errors_on_reads(api, anon, fixture):
    route = api.get("/users/amina_dev").mock(
        side_effect=[
            httpx.Response(503),
            httpx.ConnectError("boom"),
            httpx.Response(200, json=fixture("user_profile")),
        ]
    )
    assert anon.user("amina_dev").username == "amina_dev"
    assert route.call_count == 3


def test_gives_up_after_max_retries(api, anon):
    api.get("/users/x").respond(429, json={"data": {"errors": {"message": "slow down"}}})
    with pytest.raises(RateLimitError):
        anon.user("x")


def test_posts_are_not_replayed_after_a_timeout(api, client):
    route = api.post("/competitions/C/participations").mock(side_effect=httpx.ReadTimeout("slow"))
    with pytest.raises(APIError, match="Could not reach Zindi"):
        client.join("C")
    assert route.call_count == 1


def test_non_json_success_is_reported(api, anon):
    api.get("/users/x").respond(200, text="<html>maintenance</html>")
    with pytest.raises(APIError, match="non-JSON"):
        anon.user("x")


def test_user_agent_identifies_the_client(api, anon, fixture):
    route = api.get("/users/amina_dev").respond(json=fixture("user_profile"))
    anon.user("amina_dev")
    assert route.calls.last.request.headers["user-agent"].startswith("zindi-python/")


@pytest.mark.parametrize(
    ("errors", "expected"),
    [
        ({"message": "already in"}, "already in"),
        ({"base": ["a", "b"]}, "a; b"),
        ({"password": "wrong password"}, "password: wrong password"),
        ("plain", "plain"),
        (None, "Unknown error"),
    ],
)
def test_error_message_shapes(errors, expected):
    assert error_message(errors) == expected


def test_project_file_round_trip(tmp_path):
    (tmp_path / "zindi.toml").write_text('# my project\ncompetition = "Old"\nother = 1\n')
    config.write_project_competition("New-One", tmp_path)
    nested = tmp_path / "a" / "b"
    nested.mkdir(parents=True)
    assert config.read_project_competition(nested) == "New-One"
    assert "other = 1" in (tmp_path / "zindi.toml").read_text()
