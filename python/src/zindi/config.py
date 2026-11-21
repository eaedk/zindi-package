"""Where zindi keeps its files, and how it reads them.

Two files matter:

* **Credentials**: ``<config dir>/credentials.json`` holds your username and an auth
  token (never your password). It is shared by every zindi client (Python, R,
  JavaScript, Rust), so logging in once is enough.
* **Project file**: ``zindi.toml`` in your project folder (or any parent folder)
  remembers the competition you work on, so commands need no competition id.

The config dir is, in order of priority: ``$ZINDI_CONFIG_DIR``; ``%APPDATA%\\zindi`` on
Windows; ``$XDG_CONFIG_HOME/zindi``; ``~/.config/zindi``.
"""

from __future__ import annotations

import json
import os
import re
import sys
import tempfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

if sys.version_info >= (3, 11):
    import tomllib
else:  # pragma: no cover - exercised on 3.10 only
    import tomli as tomllib

__all__ = [
    "PROJECT_FILE",
    "SavedCredentials",
    "config_dir",
    "credentials_path",
    "delete_credentials",
    "find_project_file",
    "load_credentials",
    "read_project_competition",
    "save_credentials",
    "write_project_competition",
]

PROJECT_FILE = "zindi.toml"
_COMPETITION_LINE = re.compile(r"^\s*competition\s*=")


def config_dir() -> Path:
    """Return the folder that holds zindi's user-level files."""
    if override := os.environ.get("ZINDI_CONFIG_DIR"):
        return Path(override).expanduser()
    if os.name == "nt" and (appdata := os.environ.get("APPDATA")):
        return Path(appdata) / "zindi"
    if xdg := os.environ.get("XDG_CONFIG_HOME"):
        return Path(xdg).expanduser() / "zindi"
    return Path.home() / ".config" / "zindi"


def credentials_path() -> Path:
    """Return the path of the shared credentials file."""
    return config_dir() / "credentials.json"


@dataclass(frozen=True)
class SavedCredentials:
    """Credentials stored on disk by ``zindi login``."""

    username: str
    auth_token: str


def load_credentials() -> SavedCredentials | None:
    """Read saved credentials, or return ``None`` if there are none (or they are unreadable)."""
    path = credentials_path()
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        return SavedCredentials(username=str(data["username"]), auth_token=str(data["auth_token"]))
    except (OSError, ValueError, KeyError, TypeError):
        return None


def save_credentials(username: str, auth_token: str) -> Path:
    """Save credentials atomically, readable by the current user only. Returns the path."""
    path = credentials_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "username": username,
        "auth_token": auth_token,
        "saved_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
    }
    fd, tmp = tempfile.mkstemp(dir=path.parent, prefix=".credentials-", suffix=".json")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            json.dump(payload, fh, indent=2)
            fh.write("\n")
        if os.name != "nt":
            os.chmod(tmp, 0o600)
        os.replace(tmp, path)
    except BaseException:
        Path(tmp).unlink(missing_ok=True)
        raise
    return path


def delete_credentials() -> bool:
    """Delete saved credentials. Returns ``True`` if a file was removed."""
    try:
        credentials_path().unlink()
    except FileNotFoundError:
        return False
    return True


def find_project_file(start: Path | None = None) -> Path | None:
    """Find ``zindi.toml`` in ``start`` (default: the current folder) or one of its parents."""
    here = (start or Path.cwd()).resolve()
    for folder in (here, *here.parents):
        candidate = folder / PROJECT_FILE
        if candidate.is_file():
            return candidate
    return None


def read_project_competition(start: Path | None = None) -> str | None:
    """Return the competition id stored in the nearest ``zindi.toml``, if any."""
    path = find_project_file(start)
    if path is None:
        return None
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError):
        return None
    value = data.get("competition")
    return str(value) if value else None


def write_project_competition(competition: str, folder: Path | None = None) -> Path:
    """Write (or update) ``zindi.toml`` in ``folder`` with the given competition id."""
    path = (folder or Path.cwd()) / PROJECT_FILE
    lines: list[str] = []
    if path.is_file():
        lines = [
            line
            for line in path.read_text(encoding="utf-8").splitlines()
            if not _COMPETITION_LINE.match(line)
        ]
    # A JSON string literal is also a valid TOML basic string.
    lines.insert(0, f"competition = {json.dumps(competition)}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path
