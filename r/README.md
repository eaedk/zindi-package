# zindi (R)

[![R](https://github.com/eaedk/zindi-package/actions/workflows/r.yml/badge.svg)](https://github.com/eaedk/zindi-package/actions/workflows/r.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](https://github.com/eaedk/zindi-package/blob/main/LICENSE)

Compete on [Zindi](https://zindi.world) from R: browse competitions, join,
download data, submit, wait for your score, follow the leaderboard and manage
your team.

**[Documentation](https://zindi.readthedocs.io/en/latest/)** ·
[Changelog](NEWS.md)

## Install

```r
# From GitHub
# install.packages("remotes")
remotes::install_github("eaedk/zindi-package", subdir = "r")

# From CRAN, once published
install.packages("zindi")
```

## Quick start

```r
library(zindi)

zindi_login("amina_dev")                       # once per computer; prompts for the password
zindi_competitions(search = "crop")            # open competitions, as a data frame

zindi_use("Crop-Yield-Forecast-Challenge", save = TRUE)   # remembered in zindi.toml
zindi_join()
zindi_download(destination = "data")

sub <- zindi_submit("submission.csv", comment = "lgbm v1", wait = TRUE)
sub$public_score
zindi_rank()
zindi_leaderboard(limit = 10)
```

## Authentication

Browsing competitions, leaderboards, profiles and discussions needs no account.
For everything else zindi finds your credentials by itself, in this order:

1. credentials set for the session with `zindi_auth(token = ...)` or
   `zindi_auth(username = ..., password = ...)`;
2. the `ZINDI_TOKEN` environment variable;
3. the `ZINDI_USERNAME` and `ZINDI_PASSWORD` environment variables (zindi signs
   in the first time a token is needed);
4. the credentials file written by `zindi_login()`;
5. in an interactive session, a login prompt.

`zindi_login()` stores your auth token, never your password, in
`credentials.json` in `zindi_config_dir()` (`~/.config/zindi` on macOS and Linux,
`%APPDATA%\zindi` on Windows, or `ZINDI_CONFIG_DIR`). The Python, JavaScript and
Rust clients use the same file, so logging in once is enough. `zindi_logout()`
deletes it. Forgot your password? Reset it at
<https://zindi.world/passwords/request>.

In CI or on a server, set `ZINDI_TOKEN` (or `ZINDI_USERNAME` and
`ZINDI_PASSWORD`) in `.Renviron` or as a secret.

## Choosing a competition

Functions that act on a competition take its id, the slug in its URL. When you
leave it out, zindi uses the competition chosen with `zindi_use()`, then the
`ZINDI_COMPETITION` environment variable, then the `competition` entry of the
nearest `zindi.toml` in the working directory or a parent folder.

## Functions

| Function | What it does |
|---|---|
| `zindi_login()`, `zindi_logout()` | Log in (and save the token) or out |
| `zindi_auth()` | Use a token or a username and password for this session only |
| `zindi_whoami()`, `zindi_user()` | Your profile, or anyone's |
| `zindi_competitions()` | List and search competitions (data frame) |
| `zindi_competition()` | Details, data files, description and rules of a competition |
| `zindi_use()`, `zindi_competition_id()` | Set or show the default competition |
| `zindi_join()` | Join a competition (`secret_code` for private ones) |
| `zindi_download()` | Download the data, skipping files already there |
| `zindi_submit()` | Upload a submission, optionally waiting for its score |
| `zindi_submissions()`, `zindi_wait_for_score()` | Your submissions; wait for a score |
| `zindi_limits()` | Submissions left today |
| `zindi_leaderboard()` | The leaderboard (data frame) |
| `zindi_my_participation()`, `zindi_rank()` | Your rank and best score |
| `zindi_team()`, `zindi_team_create()`, `zindi_team_invite()`, `zindi_team_disband()` | Manage your team |
| `zindi_discussions()` | Discussion threads (data frame) |
| `zindi_config_dir()` | Where the credentials file lives |

Errors inherit from the condition class `zindi_error`, with a more specific
class such as `zindi_auth_error`, `zindi_not_found` or `zindi_rate_limit`:

```r
tryCatch(zindi_join("Private-Challenge"), zindi_secret_code_required = function(e) {
  zindi_join("Private-Challenge", secret_code = readline("Secret code: "))
})
```

## Running the tests

The tests run offline against the example responses shared by every client in
[`spec/fixtures`](https://github.com/eaedk/zindi-package/tree/main/spec/fixtures):

```r
testthat::test_local()     # from the r/ folder
```

Outside the repository (for example when checking a built tarball), point
`ZINDI_FIXTURES` at that folder, otherwise the tests that need it are skipped.

## License

MIT. Not affiliated with Zindi.
