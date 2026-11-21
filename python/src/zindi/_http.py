"""Low-level HTTP layer: envelopes, errors, retries, auth headers."""

from __future__ import annotations

import os
import time
from collections.abc import Callable, Iterator, Mapping
from contextlib import contextmanager
from dataclasses import dataclass
from typing import Any

import httpx

from ._version import __version__
from .errors import (
    AlreadyJoinedError,
    APIError,
    AuthenticationError,
    NotFoundError,
    RateLimitError,
    SecretCodeRequiredError,
)

DEFAULT_BASE_URL = "https://api.zindi.world/v1"
WEB_URL = "https://zindi.world"
USER_AGENT = f"zindi-python/{__version__} (+https://github.com/eaedk/zindi-package)"

_RETRY_STATUSES = frozenset({429, 500, 502, 503, 504})
_IDEMPOTENT = frozenset({"GET", "HEAD", "OPTIONS", "DELETE"})
_MAX_BACKOFF = 30.0


@dataclass
class Page:
    """One page of a list endpoint."""

    items: list[dict[str, Any]]
    total: int | None


def base_url_from_env() -> str:
    return os.environ.get("ZINDI_API_URL", DEFAULT_BASE_URL).rstrip("/")


def error_message(errors: Any) -> str:
    """Turn Zindi's ``errors`` object (whatever its shape) into one readable sentence."""
    if errors is None:
        return "Unknown error"
    if isinstance(errors, str):
        return errors
    if isinstance(errors, list):
        return "; ".join(error_message(e) for e in errors)
    if isinstance(errors, Mapping):
        for key in ("message", "base", "error"):
            if key in errors:
                return error_message(errors[key])
        return "; ".join(f"{key}: {error_message(value)}" for key, value in errors.items())
    return str(errors)


def raise_for_errors(status_code: int, errors: Any) -> None:
    """Raise the most specific exception for an error response."""
    message = error_message(errors)
    lowered = message.lower()
    kwargs: dict[str, Any] = {"status_code": status_code, "errors": errors}
    if "already in" in lowered or "already joined" in lowered:
        raise AlreadyJoinedError(message, **kwargs)
    if "secret code" in lowered:
        raise SecretCodeRequiredError(message, **kwargs)
    if status_code in (401, 403) or "not authorized" in lowered or "not authenticated" in lowered:
        raise AuthenticationError(message, **kwargs)
    if status_code == 404 or lowered == "not found":
        raise NotFoundError(message, **kwargs)
    if status_code == 429:
        raise RateLimitError(message, **kwargs)
    raise APIError(message, **kwargs)


class Transport:
    """Thin wrapper around :class:`httpx.Client` that speaks Zindi's conventions."""

    def __init__(
        self,
        *,
        base_url: str | None = None,
        token: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 3,
        http_client: httpx.Client | None = None,
        sleep: Callable[[float], None] = time.sleep,
    ) -> None:
        self.base_url = (base_url or base_url_from_env()).rstrip("/")
        self.token = token
        self.max_retries = max_retries
        self._sleep = sleep
        self._owns_client = http_client is None
        self._client = http_client or httpx.Client(
            timeout=httpx.Timeout(timeout, connect=15.0),
            follow_redirects=True,
            headers={"User-Agent": USER_AGENT, "Accept": "application/json"},
        )

    # -- lifecycle ---------------------------------------------------------------------------

    def close(self) -> None:
        if self._owns_client:
            self._client.close()

    # -- helpers -----------------------------------------------------------------------------

    def url(self, path: str) -> str:
        if path.startswith(("http://", "https://")):
            return path
        return f"{self.base_url}/{path.lstrip('/')}"

    def headers(self, *, auth: bool = True) -> dict[str, str]:
        headers = {"Current-Url": f"{WEB_URL}/", "User-Agent": USER_AGENT}
        if auth and self.token:
            # The web app sends ``auth-token``; older endpoints read ``auth_token``.
            headers["auth-token"] = self.token
            headers["auth_token"] = self.token
        return headers

    def _backoff(self, attempt: int, response: httpx.Response | None) -> float:
        if response is not None and (retry_after := response.headers.get("Retry-After")):
            try:
                return min(float(retry_after), _MAX_BACKOFF)
            except ValueError:
                pass
        return float(min(0.5 * (2**attempt), _MAX_BACKOFF))

    def _send(self, method: str, path: str, **kwargs: Any) -> httpx.Response:
        method = method.upper()
        attempt = 0
        while True:
            try:
                response = self._client.request(method, self.url(path), **kwargs)
            except httpx.TransportError as exc:
                # A POST that may have reached the server is never replayed.
                retryable = method in _IDEMPOTENT or isinstance(exc, httpx.ConnectError)
                if not retryable or attempt >= self.max_retries:
                    raise APIError(
                        f"Could not reach Zindi ({exc.__class__.__name__}): {exc}"
                    ) from exc
                self._sleep(self._backoff(attempt, None))
                attempt += 1
                continue
            if (
                response.status_code in _RETRY_STATUSES
                and method in _IDEMPOTENT
                and attempt < self.max_retries
            ):
                self._sleep(self._backoff(attempt, response))
                attempt += 1
                continue
            return response

    @staticmethod
    def _unwrap(response: httpx.Response) -> tuple[Any, Any]:
        try:
            payload = response.json()
        except ValueError:
            if response.is_success:
                preview = response.text.strip().replace("\n", " ")[:200]
                raise APIError(
                    f"Zindi returned a non-JSON response (HTTP {response.status_code}): {preview}",
                    status_code=response.status_code,
                ) from None
            raise_for_errors(response.status_code, response.reason_phrase or "HTTP error")
        data, meta = payload, None
        if isinstance(payload, dict) and "data" in payload:
            data, meta = payload["data"], payload.get("meta")
        errors = data.get("errors") if isinstance(data, dict) else None
        if errors is None and isinstance(payload, dict) and "errors" in payload:
            errors = payload["errors"]
        if errors or response.is_error:
            raise_for_errors(response.status_code, errors or response.reason_phrase)
        return data, meta

    # -- public API --------------------------------------------------------------------------

    def request(
        self,
        method: str,
        path: str,
        *,
        auth: bool = True,
        params: Mapping[str, Any] | None = None,
        **kwargs: Any,
    ) -> Any:
        """Send a request and return the ``data`` member of the response envelope."""
        clean = {k: v for k, v in (params or {}).items() if v is not None}
        response = self._send(method, path, params=clean, headers=self.headers(auth=auth), **kwargs)
        data, _ = self._unwrap(response)
        return data

    def page(self, path: str, *, params: Mapping[str, Any], auth: bool = True) -> Page:
        """Fetch one page of a list endpoint."""
        clean = {k: v for k, v in params.items() if v is not None}
        response = self._send("GET", path, params=clean, headers=self.headers(auth=auth))
        data, meta = self._unwrap(response)
        items = data if isinstance(data, list) else []
        total = meta.get("total_count") if isinstance(meta, dict) else None
        return Page(items=items, total=int(total) if total is not None else None)

    @contextmanager
    def download(self, path: str) -> Iterator[httpx.Response]:
        """Stream a file. Redirects to object storage are followed without the auth token."""
        url = self.url(path)
        headers = self.headers()
        for _ in range(5):
            request = self._client.build_request("GET", url, headers=headers)
            response = self._client.send(request, stream=True, follow_redirects=False)
            if response.is_redirect and "location" in response.headers:
                next_url = str(response.url.join(response.headers["location"]))
                response.close()
                if httpx.URL(next_url).host != httpx.URL(url).host:
                    headers = {"User-Agent": USER_AGENT}
                url = next_url
                continue
            try:
                if response.is_error:
                    response.read()
                    self._unwrap(response)
                yield response
            finally:
                response.close()
            return
        raise APIError("Too many redirects while downloading the file.")
