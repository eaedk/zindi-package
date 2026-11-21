# Changelog

All notable changes to the JavaScript client are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/).

## [1.0.0] - 2026-11-21

First release of the JavaScript and TypeScript client, on par with the Python client.

### Added

- `Client` with async methods for every part of the platform: `login`, `logout`, `me`,
  `user`, `competitions`, `competition`, `join`, `download`, `submit`, `submissions`,
  `waitForScore`, `limits`, `leaderboard`, `myParticipation`, `rank`, `team`, `createTeam`,
  `invite`, `disbandTeam` and `discussions`.
- Credentials found automatically: `token` option, `ZINDI_TOKEN`, `ZINDI_USERNAME` and
  `ZINDI_PASSWORD` (lazy sign-in), then the credentials file shared with the other zindi
  clients. Expired tokens are renewed once when a password is known.
- Default competition from `client.use()`, `ZINDI_COMPETITION` or the nearest `zindi.toml`.
- Resumable-friendly downloads: files already there are skipped, data is written to a
  `.part` file first, and the auth token is never sent to object storage after a redirect.
- Retries with exponential backoff for transient failures, honouring `Retry-After`.
  POST requests are only replayed when the connection could not be established.
- Typed errors (`ZindiError`, `APIError`, `AuthenticationError`, `NotFoundError`,
  `RateLimitError`, `SecretCodeRequiredError`, `AlreadyJoinedError`,
  `CompetitionNotSetError`) and typed models that keep the raw payload in `raw`.
- Zero runtime dependencies, ESM, Node.js 18 or newer, bundled TypeScript declarations.

[1.0.0]: https://github.com/eaedk/zindi-package/releases/tag/js-v1.0.0
