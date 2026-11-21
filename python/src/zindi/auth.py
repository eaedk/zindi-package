"""Find credentials without making the user think about it.

Sources are tried in this order, and the first one that yields something wins:

1. arguments passed to :class:`zindi.Client`,
2. environment variables ``ZINDI_TOKEN``, or ``ZINDI_USERNAME`` and ``ZINDI_PASSWORD``,
3. notebook secrets with the same names (Google Colab "Secrets", Kaggle "Add-ons > Secrets"),
4. the credentials file written by ``zindi login``,
5. an interactive prompt, when a human is there to answer it.
"""

from __future__ import annotations

import getpass
import os
import sys
from collections.abc import Callable
from dataclasses import dataclass

from . import config

__all__ = [
    "Credentials",
    "can_prompt",
    "in_notebook",
    "prompt_password",
    "prompt_username",
    "resolve",
]


@dataclass(frozen=True)
class Credentials:
    """What we found, and where we found it (for messages like ``zindi whoami``)."""

    source: str
    token: str | None = None
    username: str | None = None
    password: str | None = None

    @property
    def can_sign_in(self) -> bool:
        return bool(self.username and self.password)


def _from_mapping(get: Callable[[str], str | None], source: str) -> Credentials | None:
    token = get("ZINDI_TOKEN")
    username = get("ZINDI_USERNAME")
    password = get("ZINDI_PASSWORD")
    if token or (username and password):
        return Credentials(source=source, token=token or None, username=username, password=password)
    return None


def from_environment() -> Credentials | None:
    return _from_mapping(lambda key: os.environ.get(key) or None, "environment variables")


def from_colab() -> Credentials | None:
    if "google.colab" not in sys.modules:
        return None
    try:
        from google.colab import userdata
    except ImportError:  # pragma: no cover
        return None

    def get(key: str) -> str | None:
        try:
            value = userdata.get(key)
        except Exception:  # secret missing, or notebook access not granted
            return None
        return str(value) if value else None

    return _from_mapping(get, "Colab secrets")


def from_kaggle() -> Credentials | None:
    if not os.environ.get("KAGGLE_KERNEL_RUN_TYPE"):
        return None
    try:
        from kaggle_secrets import UserSecretsClient
    except ImportError:  # pragma: no cover
        return None
    client = UserSecretsClient()

    def get(key: str) -> str | None:
        try:
            value = client.get_secret(key)
        except Exception:
            return None
        return str(value) if value else None

    return _from_mapping(get, "Kaggle secrets")


def from_saved_file() -> Credentials | None:
    saved = config.load_credentials()
    if saved is None:
        return None
    return Credentials(
        source=str(config.credentials_path()), token=saved.auth_token, username=saved.username
    )


def resolve(*, use_saved: bool = True) -> Credentials | None:
    """Return the first credentials found, without prompting."""
    sources: list[Callable[[], Credentials | None]] = [from_environment, from_colab, from_kaggle]
    if use_saved:
        sources.append(from_saved_file)
    for source in sources:
        found = source()
        if found is not None:
            return found
    return None


def in_notebook() -> bool:
    """``True`` inside Jupyter, JupyterLab, Colab, Kaggle or VS Code notebooks."""
    try:
        from IPython import get_ipython
    except ImportError:
        return False
    shell = get_ipython()
    return shell is not None and shell.__class__.__name__ != "TerminalInteractiveShell"


def can_prompt() -> bool:
    """``True`` when someone can type an answer (a terminal or a notebook)."""
    if os.environ.get("ZINDI_NO_PROMPT"):
        return False
    try:
        interactive = sys.stdin is not None and sys.stdin.isatty()
    except (AttributeError, ValueError):
        interactive = False
    return interactive or in_notebook()


def prompt_username() -> str:
    return input("Zindi username: ").strip()


def prompt_password(username: str) -> str:
    return getpass.getpass(f"Zindi password for {username}: ")
