# Migrating from 0.x

Version 1.0 is a rewrite. Your old notebooks keep working: `from zindi.user import Zindian`
is still available and prints a `FutureWarning`. New code should use `zindi.Client`, which
returns data instead of printing it.

| 0.x | 1.0 |
|---|---|
| `Zindian(username=...)` | `zindi.Client()` or `zindi.login()` |
| `user.select_a_challenge()` | `client.competitions()` then `client.use(id)` / `client.join(id)`, or `zindi.explore()` |
| `user.which_challenge` | `client.competition_id` |
| `user.leaderboard()` | `client.leaderboard()` |
| `user.my_rank` | `client.rank()` |
| `user.remaining_subimissions` | `client.limits().remaining_today` |
| `user.submission_board()` | `client.submissions()` |
| `user.download_dataset(destination)` | `client.download(destination=...)` |
| `user.submit(filepaths, comments)` | `client.submit(path, comment=...)` |
| `user.create_team(name, teammates)` | `client.create_team(name, invite=teammates)` |
| `user.team_up(zindians)` | `client.invite(usernames)` |
| `user.disband_team()` | `client.disband_team()` |

What else changed:

- Zindi moved to `zindi.world`; the client talks to `https://api.zindi.world/v1`.
- Submissions are no longer limited to CSV files.
- The package installs with `pip install zindi` (no more `git+https://...`), and its
  dependencies are declared properly (the old `tdqm` typo is gone).
- There is a command line, an explorer widget, and clients for R, JavaScript and Rust.
