from __future__ import annotations

import json
import os
from collections.abc import Callable, Iterator
from pathlib import Path
from typing import Any

import pytest
import respx

from zindi import Client

API = "https://api.test/v1"
FIXTURES = Path(os.environ.get("ZINDI_FIXTURES", Path(__file__).parents[2] / "spec" / "fixtures"))


@pytest.fixture(autouse=True)
def isolated(
    request: pytest.FixtureRequest, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> Path:
    """Every test gets its own config dir and working dir, and no ambient credentials."""
    if request.node.get_closest_marker("live"):
        return tmp_path  # live tests use the real environment on purpose
    for key in list(os.environ):
        if key.startswith("ZINDI_") and key != "ZINDI_FIXTURES":
            monkeypatch.delenv(key)
    monkeypatch.delenv("KAGGLE_KERNEL_RUN_TYPE", raising=False)
    monkeypatch.setenv("ZINDI_CONFIG_DIR", str(tmp_path / "config"))
    monkeypatch.setenv("ZINDI_NO_PROMPT", "1")
    monkeypatch.setenv("ZINDI_API_URL", API)
    work = tmp_path / "work"
    work.mkdir()
    monkeypatch.chdir(work)
    return tmp_path


@pytest.fixture
def fixture() -> Callable[[str], Any]:
    """Load a JSON fixture shared by every client of this repository."""
    if not FIXTURES.is_dir():
        pytest.skip(f"shared fixtures not found at {FIXTURES}")

    def load(name: str) -> Any:
        return json.loads((FIXTURES / f"{name}.json").read_text(encoding="utf-8"))

    return load


@pytest.fixture
def api() -> Iterator[respx.MockRouter]:
    with respx.mock(base_url=API, assert_all_called=False) as router:
        yield router


@pytest.fixture
def client() -> Iterator[Client]:
    c = Client(token="tok_test", progress=False, _sleep=lambda _s: None)
    yield c
    c.close()


@pytest.fixture
def anon() -> Iterator[Client]:
    c = Client(progress=False, use_saved_credentials=False, _sleep=lambda _s: None)
    yield c
    c.close()
