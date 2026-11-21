from __future__ import annotations

import pytest

from zindi import Client

COMP = "Crop-Yield-Forecast-Challenge"


def test_legacy_zindian_still_works(api, fixture, tmp_path):
    api.post("/auth/signin").respond(json=fixture("signin_ok"))
    api.get("/competitions").respond(json=fixture("competitions"))
    api.post(f"/competitions/{COMP}/participations").respond(json=fixture("join_already"))
    api.get(f"/competitions/{COMP}/submissions/limits").respond(json=fixture("submission_limits"))
    upload = api.post(f"/competitions/{COMP}/submissions").respond(
        json=fixture("submission_created")
    )

    from zindi.user import Zindian

    with pytest.warns(FutureWarning):
        user = Zindian(username="amina_dev", fixed_password="hunter2")
    assert user.which_challenge is None
    user.select_a_challenge(fixed_index=0)
    assert user.which_challenge == COMP
    assert user.remaining_subimissions == 7  # the historic misspelling keeps working
    sub = tmp_path / "s.csv"
    sub.write_text("x")
    user.submit(filepaths=[str(sub)], comments=["legacy"])
    assert upload.called


def test_explorer_builds_and_acts(api, fixture):
    pytest.importorskip("ipywidgets")
    from zindi.notebook import Explorer

    api.get("/competitions").respond(json=fixture("competitions"))
    api.post(f"/competitions/{COMP}/participations").respond(json=fixture("join_ok"))
    client = Client(token="t", progress=False)
    explorer = Explorer(client)
    explorer.refresh()
    assert len(explorer.picker.options) == 2
    explorer.picker.index = 0
    assert explorer.selected is not None and explorer.selected.id == COMP
    explorer._act("join", explorer.selected)
    explorer._act("use", explorer.selected)
    assert client.competition_id == COMP
