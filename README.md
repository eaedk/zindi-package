<div align="center">

# zindi

**Compete on [Zindi](https://zindi.world) without leaving your terminal, notebook or code.**

Browse competitions, join them, download the data, submit, wait for your score and follow
the leaderboard from Python, the command line, Jupyter/Colab, R, JavaScript or Rust.

[![PyPI](https://img.shields.io/pypi/v/zindi?label=PyPI&color=f5a623)](https://pypi.org/project/zindi/)
[![npm](https://img.shields.io/npm/v/zindi?label=npm&color=f5a623)](https://www.npmjs.com/package/zindi)
[![crates.io](https://img.shields.io/crates/v/zindi?label=crates.io&color=f5a623)](https://crates.io/crates/zindi)
[![Python CI](https://github.com/eaedk/zindi-package/actions/workflows/python.yml/badge.svg)](https://github.com/eaedk/zindi-package/actions/workflows/python.yml)
[![Docs](https://readthedocs.org/projects/zindi/badge/?version=latest)](https://zindi.readthedocs.io/en/latest/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

[Documentation](https://zindi.readthedocs.io/en/latest/) ·
[Quick start](#quick-start) ·
[Open in Colab](https://colab.research.google.com/github/eaedk/zindi-package/blob/main/examples/quickstart.ipynb) ·
[Contributing](CONTRIBUTING.md)

</div>

---

## Quick start

```bash
pip install zindi          # or: uv tool install zindi · pipx install zindi · uvx zindi
```

```bash
zindi login                                  # once; your token is saved, never your password
zindi competitions                           # what is open right now?
zindi use Crop-Yield-Forecast-Challenge      # remember it for this project folder
zindi join                                   # accept the rules
zindi download                               # data lands in ./data
zindi submit submission.csv -m "lgbm v1" --wait   # upload and print the score
zindi leaderboard                            # your row is highlighted
```

The same in Python:

```python
import zindi

client = zindi.Client()                      # finds your credentials by itself
comp = client.competition("Crop-Yield-Forecast-Challenge")
comp.join()
comp.download("data")
sub = comp.submit("submission.csv", comment="lgbm v1", wait=True)
print(sub.public_score, client.rank(comp.id))
comp.leaderboard(limit=10).to_pandas()
```

In a notebook (Jupyter, Colab, Kaggle, VS Code), results render as tables, and
`zindi.explore()` opens a point-and-click explorer:

```python
!pip install -q zindi
import zindi
zindi.explore()
```

## Why this package

- **Everything a competitor does**: browse and search competitions, read the rules, join
  (including private competitions with a secret code), download data, submit, wait for the
  score, check remaining submissions, see your rank, manage your team, read discussions.
- **Works where you work**: Windows, macOS, Linux, Google Colab, Kaggle, Jupyter, VS Code,
  plain terminals and CI. Installs with `pip`, `uv`, `pipx` or `conda`'s pip.
- **Log in once, everywhere**: the token is stored in one shared file read by the CLI and by
  every client below. Environment variables and Colab/Kaggle secrets are supported too.
- **Safe and polite with the platform**: resumable, size-checked downloads, retries with
  backoff, no password stored, no token sent to third-party storage.
- **Data you can use**: every result is a typed object, converts to pandas, and keeps the
  raw API payload so nothing is hidden.

## One API, several languages

| Language | Package | Install | Folder |
|---|---|---|---|
| Python + CLI | [`zindi`](https://pypi.org/project/zindi/) | `pip install zindi` | [`python/`](python) |
| JavaScript / TypeScript | [`zindi`](https://www.npmjs.com/package/zindi) | `npm install zindi` | [`js/`](js) |
| R | `zindi` | `remotes::install_github("eaedk/zindi-package", subdir = "r")` | [`r/`](r) |
| Rust | [`zindi`](https://crates.io/crates/zindi) | `cargo add zindi` | [`rust/`](rust) |

All clients follow the same [API description](spec/openapi.yaml) and are tested against the
same [fixtures](spec/fixtures), so a change on the Zindi side is fixed in one place first.
Any other language can drive the CLI, which speaks JSON: `zindi leaderboard --json`.

## Authentication

Credentials are looked up in this order, so you rarely have to think about them:

1. a token or username/password passed in code,
2. `ZINDI_TOKEN`, or `ZINDI_USERNAME` and `ZINDI_PASSWORD` environment variables,
3. Colab or Kaggle secrets with the same names,
4. the file saved by `zindi login`,
5. an interactive prompt.

Details, including where the file lives on each OS, are in the
[authentication guide](https://zindi.readthedocs.io/en/latest/authentication/).

## Project layout

```text
spec/      API description + shared test fixtures (the contract every client follows)
python/    Python library, CLI and notebook widgets  (PyPI: zindi)
js/        TypeScript client for Node.js             (npm: zindi)
r/         R package                                 (zindi)
rust/      Rust crate                                (crates.io: zindi)
docs/      Documentation website (MkDocs)
examples/  Notebooks and scripts
```

## Contributing

Bug reports, ideas and pull requests are very welcome, whatever your level. Start with
[CONTRIBUTING.md](CONTRIBUTING.md); issues labelled
[`good first issue`](https://github.com/eaedk/zindi-package/labels/good%20first%20issue)
are a gentle way in. Please follow our [code of conduct](CODE_OF_CONDUCT.md).

## Team

<table>
  <tr>
    <td align="center"><a href="https://github.com/eaedk"><img src="https://github.com/eaedk.png?size=100" width="90" alt=""><br><b>Emmanuel Koupoh</b></a><br><sub>@eaedk · creator, maintainer</sub></td>
    <td align="center"><a href="https://github.com/NazarioR9"><img src="https://github.com/NazarioR9.png?size=100" width="90" alt=""><br><b>Muhamed Tuo</b></a><br><sub>@NazarioR9 · core contributor</sub></td>
    <td align="center"><a href="https://github.com/dric2018"><img src="https://github.com/dric2018.png?size=100" width="90" alt=""><br><b>Cédric Manouan</b></a><br><sub>@dric2018 · core contributor</sub></td>
  </tr>
</table>

### Thank you

This project lived through years when the maintainers had little time. These people opened
issues, sent pull requests or kept forks alive, and many of their ideas are in this release:

| Who | Contribution |
|---|---|
| [@KameniAlexNea](https://github.com/KameniAlexNea) | Packaging for PyPI, the submission-limits endpoint and the 2026 API fixes (PRs #11, #12, #13) |
| [@Tim-Abwao](https://github.com/Tim-Abwao) | Packaging and project-structure clean-up (PRs #3, #4) |
| [@LeMeteore](https://github.com/LeMeteore) | Asked for real tests, tox-style automation and proper docs (issues #5, #6, #7) |
| [@TshepoMK](https://github.com/TshepoMK) | Improved usage examples for Colab (PR #2) |
| [@RamvigneshPasupathy](https://github.com/RamvigneshPasupathy) | Suggested a password-reset hint on login failure (issue #1) |
| [@MuhammadQasimShabeer](https://github.com/MuhammadQasimShabeer) | Asked for an "open competitions only" filter (issue #10) |
| [@Stephen-Kamau](https://github.com/Stephen-Kamau), [@dote7ant](https://github.com/dote7ant), [@ThapeloMarakalla](https://github.com/ThapeloMarakalla) | Forked and experimented with the package |

Issue and PR numbers refer to the former
[testing-zindi-package](https://github.com/eaedk/testing-zindi-package) repository, whose
history is now part of this one. The full list is in [ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md).

## Disclaimer

This is a community project. It is not affiliated with or endorsed by Zindi. Use it within
the [Zindi terms](https://zindi.world/terms) and each competition's rules.

## License

[MIT](LICENSE) © 2020-2026 Emmanuel Koupoh and the zindi contributors.
