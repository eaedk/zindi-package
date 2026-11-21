# Changelog

All notable changes to the `zindi` crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the crate follows
[Semantic Versioning](https://semver.org/).

## [1.0.0] - 2026-11-21

First release of the Rust client.

### Added

- Async `Client` (tokio + reqwest with rustls) built with `Client::builder()` or
  `Client::new()`.
- Competitions: `competitions` (open, closed or all; kind, reward and text filters;
  pagination), `competition`, `join` (with secret codes), `download` (skips the
  all-in-one archive and files already downloaded, writes through `.part` files, follows
  storage redirects without sending the auth token).
- Submissions: `submit` (multipart upload, optional wait for the score), `submissions`,
  `wait_for_score`, `limits`.
- Leaderboards: `leaderboard`, `my_participation`, `rank` (with a leaderboard fallback).
- Teams: `team`, `create_team`, `invite`, `disband_team`.
- Users and discussions: `me`, `user`, `discussions`.
- Credentials from the builder, `ZINDI_TOKEN`, `ZINDI_USERNAME` + `ZINDI_PASSWORD` or the
  credentials file shared with the Python, JavaScript and R clients; `login` and `logout`;
  automatic sign in again when a token expires.
- Competition selection from the argument, `use_competition`, `ZINDI_COMPETITION` or the
  nearest `zindi.toml`.
- Retries with exponential backoff for transient failures, honouring `Retry-After`.
- Typed `Error` enum and tolerant models that keep the raw API payload.

[1.0.0]: https://github.com/eaedk/zindi-package/releases/tag/rust-v1.0.0
