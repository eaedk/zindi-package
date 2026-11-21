# zindi

**Compete on [Zindi](https://zindi.world) without leaving your terminal, notebook or code.**

`zindi` lets you do everything a competitor does on the website (browse competitions,
join, download data, submit, wait for the score, follow the leaderboard, manage your team),
from the place where you already work.

<div class="grid cards" markdown>

- **Command line**: `zindi submit submission.csv --wait` → [CLI guide](cli.md)
- **Python**: `client.competition("…").download("data")` → [Python guide](python.md)
- **Notebooks**: tables render nicely, and `zindi.explore()` gives you buttons → [Notebook guide](notebooks.md)
- **R, JavaScript, Rust**: the same features, the same login → [Other languages](languages/javascript.md)

</div>

## In thirty seconds

```bash
pip install zindi
zindi login
zindi competitions
zindi use <competition-id>
zindi join && zindi download
zindi submit submission.csv -m "first try" --wait
```

## Works everywhere

| | Windows | macOS | Linux | Colab | Kaggle | Jupyter / VS Code |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Python + CLI | ✔ | ✔ | ✔ | ✔ | ✔ | ✔ |
| JavaScript (Node 18+) | ✔ | ✔ | ✔ | | | |
| R | ✔ | ✔ | ✔ | ✔ | ✔ | ✔ |
| Rust | ✔ | ✔ | ✔ | | | |

Python 3.10 or newer. Install with `pip`, `uv`, `pipx`, or the pip of your conda environment.

!!! note "Community project"
    This project is not affiliated with Zindi. Use it within the Zindi terms of use and each
    competition's rules.
