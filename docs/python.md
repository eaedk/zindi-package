# Python library

```python
import zindi

client = zindi.Client()
```

`Client` finds your credentials by itself ([how](authentication.md)). It only logs in when
an operation needs it, so browsing works without an account.

## Choosing a competition

Methods that act on a competition accept its id. To avoid repeating it:

```python
client.use("Crop-Yield-Forecast-Challenge")          # for this client
client.use("Crop-Yield-Forecast-Challenge", save=True)  # also writes ./zindi.toml
```

Or work with a competition object, which is bound to its client:

```python
comp = client.competition("Crop-Yield-Forecast-Challenge")
comp.join()
comp.download("data")
comp.submit("submission.csv", comment="baseline", wait=True)
comp.leaderboard(limit=10)
```

## Reference

| Method | Returns |
|---|---|
| `login(username=None, password=None, save=True)` | `User` |
| `logout(forget=True)` | |
| `me()` / `user(username)` | `User` |
| `competitions(status="open", kind=None, reward=None, search=None, limit=50)` | `ResultList[Competition]` |
| `competition(id)` | `Competition` (with `data_files`, `rules`, `metric`, `daily_submission_limit`) |
| `join(id, secret_code=None)` | `True` if newly joined, `False` if already in |
| `download(id, destination=".", files=None, unzip=False, overwrite=False)` | `list[Path]` |
| `submit(path, comment="", competition=None, wait=False, timeout=600)` | `Submission` |
| `submissions(id, limit=None)` | `ResultList[Submission]` |
| `wait_for_score(submission_id, competition=None)` | `Submission` |
| `limits(id)` | `SubmissionLimits` (`remaining_today`) |
| `leaderboard(id, limit=None, include_unranked=False)` | `ResultList[LeaderboardEntry]` |
| `my_participation(id)` | `Participation` (rank, score, team) |
| `rank(id)` | `int` or `None` |
| `team(id)` | `Team` or `None` |
| `create_team(name, competition=None, invite=())` | `Team` |
| `invite(usernames, competition=None)` | `list[str]` |
| `disband_team(id)` | |
| `discussions(id, limit=20)` | `ResultList[Discussion]` |

`zindi.login()` is a shortcut that prompts for what is missing and returns a logged-in client.

## Results

Every model is a dataclass with typed fields and a `raw` attribute holding the untouched API
payload. Lists are `ResultList`s: normal lists that also

- print as tables in a terminal (`rich.print(result)`) and in notebooks,
- convert with `.to_pandas()` (needs `pandas`) or `.to_records()`.

```python
board = client.leaderboard("Crop-Yield-Forecast-Challenge")
df = board.to_pandas()
df[df.is_team]
```

## Errors

All exceptions derive from `zindi.ZindiError`:

| Exception | When |
|---|---|
| `AuthenticationError` | Not logged in, wrong password, expired token, or not allowed (e.g. download before joining) |
| `NotFoundError` | Unknown competition, user or file |
| `SecretCodeRequiredError` | Joining a private competition without its code |
| `RateLimitError` | Zindi kept answering HTTP 429 after retries |
| `CompetitionNotSetError` | No competition given and none configured |
| `APIError` | Any other error reported by Zindi (`.status_code`, `.errors`) |

```python
try:
    client.join("Private-Hackathon")
except zindi.SecretCodeRequiredError:
    client.join("Private-Hackathon", secret_code=input("Code: "))
```

## Configuration

| Option / variable | Default | Meaning |
|---|---|---|
| `base_url` / `ZINDI_API_URL` | `https://api.zindi.world/v1` | API root |
| `timeout` | 30 s | Network timeout |
| `max_retries` | 3 | Retries on timeouts and HTTP 429/5xx (reads only) |
| `progress` | `True` | Progress bars for downloads and uploads |
| `http_client` | | Your own `httpx.Client` (proxies, certificates) |
| `ZINDI_CONFIG_DIR` | OS-specific | Where `credentials.json` lives |
| `ZINDI_COMPETITION` | | Default competition |
| `ZINDI_NO_PROMPT` | | Never prompt (useful in CI) |
