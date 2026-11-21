# Contributing to zindi

Thank you for taking the time to contribute! This project exists to make life easier for
Zindi competitors, and it only stays useful if people like you report problems and send
improvements. Every contribution counts: a typo fix, a bug report, a new feature, a review.

## Ways to help

- **Report a bug** or **suggest a feature** with an
  [issue](https://github.com/eaedk/zindi-package/issues/new/choose). For bugs, paste the
  output of `zindi doctor`; it tells us your OS, Python version and environment.
- **Fix the API description** when Zindi changes something: update
  [`spec/openapi.yaml`](https://github.com/eaedk/zindi-package/blob/main/spec/openapi.yaml) and the [fixtures](https://github.com/eaedk/zindi-package/tree/main/spec/fixtures), then the
  clients. This is the single most valuable kind of contribution.
- **Improve the docs**: they live in [`docs/`](https://github.com/eaedk/zindi-package/tree/main/docs) and are plain Markdown.
- **Pick an issue** labelled `good first issue` or `help wanted`, and say you are on it.

## Repository layout

| Folder | What | Tooling |
|---|---|---|
| `spec/` | API description and shared JSON fixtures | YAML/JSON |
| `python/` | Python library, CLI, notebook widgets | [uv](https://docs.astral.sh/uv/), pytest, ruff, mypy |
| `js/` | TypeScript client | npm, `node --test` |
| `r/` | R package | devtools/testthat |
| `rust/` | Rust crate | cargo |
| `docs/` | Documentation website | MkDocs Material |

Each client is independent: you only need the toolchain of the part you change.

## Development setup

### Python

```bash
cd python
uv sync                    # creates .venv with the package and dev tools
uv run pytest              # tests (no network needed)
uv run ruff check .        # lint
uv run ruff format .       # format
uv run mypy                # type check (strict)
uv run zindi --help        # try the CLI from your checkout
```

No uv? `python -m venv .venv && . .venv/bin/activate && pip install -e . pytest respx ruff mypy pandas ipywidgets`
works too.

### JavaScript

```bash
cd js
npm install
npm test
```

### R

```r
setwd("r")
devtools::install_deps(dependencies = TRUE)
devtools::test()
devtools::check()
```

### Rust

```bash
cd rust
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
```

### Docs

```bash
pip install mkdocs-material
mkdocs serve               # http://127.0.0.1:8000
```

## Tests and the real API

Unit tests never touch the network: they replay the fixtures in `spec/fixtures`. To check
that the fixtures still match reality, the Python client has opt-in live tests:

```bash
cd python
ZINDI_LIVE=1 ZINDI_TOKEN=... uv run pytest -m live
```

They only read data (no join, no submission). Never commit tokens or passwords.

## Pull requests

1. Fork, then create a branch: `git switch -c fix/leaderboard-ties`.
2. Keep the change focused; add or update tests; update the docs and the client's
   `CHANGELOG.md` under "Unreleased".
3. Make sure the checks of the folder you touched pass locally.
4. Open the PR and describe the problem it solves. Screenshots of CLI output are welcome.

We use [Conventional Commits](https://www.conventionalcommits.org/) loosely
(`fix(python): ...`, `feat(js): ...`, `docs: ...`); it helps writing changelogs.

## Design principles

- **Library functions return data; only the CLI prints.**
- **Same behaviour in every language.** If you change how something works in one client,
  open an issue for the others (or change them too).
- **Be gentle with Zindi's servers**: paginate, retry with backoff, never hammer.
- **Never store a password.** Tokens only, in the shared credentials file.
- **Friendly errors**: say what went wrong and what to do next.

## Releasing (maintainers)

Each package is released from a tag; CI builds and publishes it.

| Package | Bump version in | Tag |
|---|---|---|
| Python | `python/src/zindi/_version.py` | `python-v1.2.3` |
| JavaScript | `js/package.json` | `js-v1.2.3` |
| Rust | `rust/Cargo.toml` | `rust-v1.2.3` |
| R | `r/DESCRIPTION` | `r-v1.2.3` (CRAN submission is manual) |

Update the matching `CHANGELOG.md`/`NEWS.md`, merge, then push the tag.

## Code of conduct

By participating you agree to follow our [code of conduct](https://github.com/eaedk/zindi-package/blob/main/CODE_OF_CONDUCT.md).
