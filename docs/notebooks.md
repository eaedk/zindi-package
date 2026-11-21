# Notebooks: Jupyter, Colab, Kaggle, VS Code

[Open the quick-start notebook in Colab](https://colab.research.google.com/github/eaedk/zindi-package/blob/main/examples/quickstart.ipynb){ .md-button .md-button--primary }

## Install and log in

```python
%pip install -q zindi
import zindi
client = zindi.Client()
```

- On **Colab** or **Kaggle**, store `ZINDI_USERNAME` and `ZINDI_PASSWORD` (or `ZINDI_TOKEN`)
  as secrets; the client reads them, and your shared notebook contains no password.
- Elsewhere, the first call that needs an account shows a login prompt, or call
  `client = zindi.login()` yourself.

## Results render as tables

The last expression of a cell is displayed as an HTML table: competition lists,
leaderboards (your row highlighted), submissions. Competition details render as a card
with the data files.

```python
client.leaderboard("Crop-Yield-Forecast-Challenge", limit=15)
```

Need a DataFrame? `.to_pandas()`.

## The explorer

```python
zindi.explore()
```

A small app inside your notebook: search and filter competitions, then click to see
details, join (with a secret code if needed), download the data, show the leaderboard or
your submissions, and set the competition as the notebook's default.

It uses `ipywidgets`, preinstalled on Colab. Elsewhere: `pip install "zindi[notebook]"`.

## A typical competition notebook

```python
import zindi, pandas as pd

client = zindi.Client(competition="Crop-Yield-Forecast-Challenge")
client.join()
client.download("data")                     # instant on re-runs: files are skipped

train = pd.read_csv("data/Train.csv")
# ... your model ...
submission.to_csv("submission.csv", index=False)

sub = client.submit("submission.csv", comment="lgbm v3", wait=True)
print(f"public score: {sub.public_score}, rank: {client.rank()}")
```
