# Changelog

All notable changes to the Python package are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.0.0] - 2026-11-21

A complete rewrite.

### Added

- `zindi.Client`, a typed client that returns data instead of printing it.
- The `zindi` command line: `login`, `logout`, `whoami`, `user`, `competitions`, `show`,
  `use`, `join`, `download`, `submit --wait`, `submissions`, `limits`, `leaderboard`,
  `rank`, `discussions`, `team show|create|invite|disband` and `doctor`. `--json` output for
  scripting, and `python -m zindi`.
- Per-project default competition in `zindi.toml` (`zindi use`).
- Credentials from arguments, environment variables, Colab and Kaggle secrets, or a token
  file shared with the R, JavaScript and Rust clients. Passwords are never stored.
- Notebook support: HTML tables, competition cards and the `zindi.explore()` widget.
- `submit(..., wait=True)` waits for the score; `limits()` shows submissions left today.
- Resumable, size-checked downloads with optional unzip; private competitions with secret codes.
- Discussions and public user profiles.
- `.to_pandas()` on every result list.
- Retries with backoff on transient errors; friendly, typed exceptions.
- Login errors link to the password-reset page (thanks @RamvigneshPasupathy, issue #1).
- Competitions are filtered to open ones by default (thanks @MuhammadQasimShabeer, issue #10).

### Changed

- Talks to `https://api.zindi.world/v1` (Zindi's new domain) with the `auth-token` header.
- Packaged with `pyproject.toml` and published on PyPI (thanks @KameniAlexNea and @Tim-Abwao).
- Submissions are no longer restricted to CSV files.
- Requires Python 3.10+.

### Deprecated

- `zindi.user.Zindian`, kept for old notebooks. Use `zindi.Client`.

## [0.0.1] - 2020-08-31

First version, by @eaedk, @dric2018 and @NazarioR9.
