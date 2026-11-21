"""Exceptions raised by the zindi package.

Every exception inherits from :class:`ZindiError`, so ``except zindi.ZindiError``
catches anything this package raises on purpose.
"""

from __future__ import annotations

from typing import Any

__all__ = [
    "APIError",
    "AlreadyJoinedError",
    "AuthenticationError",
    "CompetitionNotSetError",
    "NotFoundError",
    "RateLimitError",
    "SecretCodeRequiredError",
    "ZindiError",
]

PASSWORD_RESET_URL = "https://zindi.world/passwords/request"


class ZindiError(Exception):
    """Base class for every error raised by this package."""


class APIError(ZindiError):
    """The Zindi API answered with an error.

    Attributes:
        status_code: HTTP status of the response.
        errors: The ``errors`` object returned by Zindi, when there is one.
    """

    def __init__(
        self,
        message: str,
        *,
        status_code: int | None = None,
        errors: Any = None,
    ) -> None:
        super().__init__(message)
        self.message = message
        self.status_code = status_code
        self.errors = errors


class AuthenticationError(APIError):
    """Missing, wrong or expired credentials."""


class NotFoundError(APIError):
    """The competition, user or file does not exist (or is not visible to you)."""


class RateLimitError(APIError):
    """Zindi asked us to slow down (HTTP 429) and retries did not help."""


class SecretCodeRequiredError(APIError):
    """The competition is private: joining it needs a secret code."""


class AlreadyJoinedError(APIError):
    """You are already a participant. :meth:`zindi.Client.join` swallows this one."""


class CompetitionNotSetError(ZindiError):
    """An operation needs a competition, but none was given or selected."""

    def __init__(self) -> None:
        super().__init__(
            "No competition selected. Pass a competition id, call "
            "`client.use('<competition-id>')`, or run `zindi use <competition-id>`."
        )
