# zindi (Python)

[![PyPI](https://img.shields.io/pypi/v/zindi?color=f5a623)](https://pypi.org/project/zindi/)
[![Python versions](https://img.shields.io/pypi/pyversions/zindi)](https://pypi.org/project/zindi/)
[![CI](https://github.com/eaedk/zindi-package/actions/workflows/python.yml/badge.svg)](https://github.com/eaedk/zindi-package/actions/workflows/python.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](https://github.com/eaedk/zindi-package/blob/main/LICENSE)

Compete on [Zindi](https://zindi.world) from Python, the command line, or a notebook:
browse competitions, join, download data, submit, wait for your score, follow the
leaderboard and manage your team.

**[Documentation](https://eaedk.github.io/zindi-package/)** ·
[Open in Colab](https://colab.research.google.com/github/eaedk/zindi-package/blob/main/examples/quickstart.ipynb) ·
[Changelog](https://github.com/eaedk/zindi-package/blob/main/python/CHANGELOG.md)

## Install

```bash
pip install zindi                # or: uv add zindi · uv tool install zindi · pipx install zindi
pip install "zindi[all]"         # + pandas and the notebook explorer
```

Python 3.10+ on Windows, macOS, Linux, Colab, Kaggle and Jupyter.

## Command line

```bash
zindi login
zindi competitions --search soil
zindi use Crop-Yield-Forecast-Challenge
zindi join
zindi download
zindi submit submission.csv -m "lgbm v1" --wait
zindi leaderboard
```

Run `zindi --help` or see the [CLI guide](https://eaedk.github.io/zindi-package/cli/).

## Library

```python
import zindi

client = zindi.Client()
comp = client.competition("Crop-Yield-Forecast-Challenge")
comp.join()
comp.download("data")
sub = comp.submit("submission.csv", comment="lgbm v1", wait=True)
print(sub.public_score, client.rank(comp.id))
client.leaderboard(comp.id, limit=10).to_pandas()
```

## Notebooks

Results display as tables, and `zindi.explore()` opens a point-and-click explorer.
On Colab or Kaggle, store `ZINDI_USERNAME`/`ZINDI_PASSWORD` (or `ZINDI_TOKEN`) as secrets
and `zindi.Client()` picks them up.

## Running the tests

```bash
uv sync
uv run pytest                                  # offline, uses ../spec/fixtures
ZINDI_LIVE=1 uv run pytest -m live             # read-only checks against the real API
```

## License

MIT. Not affiliated with Zindi.
