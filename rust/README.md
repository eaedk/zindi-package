# zindi (Rust)

[![crates.io](https://img.shields.io/crates/v/zindi.svg)](https://crates.io/crates/zindi)
[![docs.rs](https://docs.rs/zindi/badge.svg)](https://docs.rs/zindi)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

An async Rust client for [Zindi](https://zindi.world), the data science competition
platform. Browse competitions, download data, submit predictions, follow leaderboards and
manage teams.

It talks to the same API and reads the same credentials file as the Python, JavaScript
and R clients of the [zindi-package](https://github.com/eaedk/zindi-package) project: log
in once with any of them and every client is logged in.

Full documentation: <https://zindi.readthedocs.io/en/latest/> and
[docs.rs/zindi](https://docs.rs/zindi).

## Install

```sh
cargo add zindi
cargo add tokio --features macros,rt-multi-thread
```

The minimum supported Rust version is **1.83**. TLS uses rustls; no OpenSSL is needed.

## Quick start

```rust,no_run
use zindi::{Client, CompetitionQuery, DownloadOptions};

#[tokio::main]
async fn main() -> zindi::Result<()> {
    let client = Client::new()?;

    // Browsing needs no account.
    for comp in client.competitions(CompetitionQuery::new()).await? {
        println!("{:<45} {}", comp.id, comp.title);
    }

    let id = "Crop-Yield-Forecast-Challenge";
    client.join(Some(id), None).await?;
    client.download(Some(id), "data", DownloadOptions::new()).await?;

    let submission = client
        .submit("submission.csv", "lightgbm baseline", Some(id), true)
        .await?;
    println!("score: {:?}", submission.score());
    println!("rank: {:?}", client.rank(Some(id)).await?);
    Ok(())
}
```

Run the bundled example with `cargo run --example quickstart`.

## Authentication

Browsing competitions, leaderboards, profiles and discussions needs no credentials. For
everything else the client looks for credentials lazily, in this order:

1. a token passed to the builder: `Client::builder().token("...").build()?`;
2. the `ZINDI_TOKEN` environment variable;
3. `ZINDI_USERNAME` and `ZINDI_PASSWORD` (or `.username(..)` and `.password(..)` on the
   builder): the client signs in on first use;
4. the shared credentials file written by `zindi login` (any client):
   `$ZINDI_CONFIG_DIR/credentials.json`, else `%APPDATA%\zindi\credentials.json` on
   Windows, else `$XDG_CONFIG_HOME/zindi/credentials.json`, else
   `~/.config/zindi/credentials.json`.

```rust,no_run
# async fn demo() -> zindi::Result<()> {
let client = zindi::Client::new()?;
// Sign in and save the token (never the password) for every zindi client.
let me = client.login("amina_dev", "my password", true).await?;
println!("hello {}", me.username);
client.logout(true)?; // forget the token and delete the credentials file
# Ok(()) }
```

When a token has expired and a username and password are known, the client signs in
again and retries the call once. A wrong password gives an `Error::Authentication` whose
message links to <https://zindi.world/passwords/request>.

## Choosing a competition

Every method that works on a competition takes an `Option<&str>`. With `None`, the client
uses, in order: `client.use_competition(..)` (or `.competition(..)` on the builder), the
`ZINDI_COMPETITION` environment variable, then the nearest `zindi.toml` in the current
folder or its parents:

```toml
competition = "Crop-Yield-Forecast-Challenge"
```

`zindi::config::write_project_competition` writes that file.

## Methods

| Method | What it does | Login |
|---|---|---|
| `competitions(query)` | List competitions (`Status::Open` by default, `kind`, `reward`, `search`, `limit`) | no |
| `competition(id)` | Details: data files, rules, metric, team size | no |
| `join(id, secret_code)` | Join; `Ok(false)` if you were already in | yes |
| `download(id, dest, options)` | Download data files (skips the all-in-one zip and files already there) | yes |
| `submit(path, comment, id, wait)` | Upload a submission, optionally wait for its score | yes |
| `submissions(id, limit)` | Your submissions, newest first | yes |
| `wait_for_score(sub_id, id, timeout, poll)` | Poll until a submission is scored | yes |
| `limits(id)` | Submissions left today | yes |
| `leaderboard(id, limit, include_unranked)` | Leaderboard rows | no |
| `my_participation(id)` | Your rank, best score and submission count | yes |
| `rank(id)` | Your rank, or `None` | yes |
| `team(id)` | Your team, or `None` if you compete alone | yes |
| `create_team(name, id, invite)` | Create a team and invite people | yes |
| `invite(usernames, id)` | Invite users to your team | yes |
| `disband_team(id)` | Disband your team | yes |
| `discussions(id, limit)` | Discussion threads | no |
| `me()` / `user(username)` | Your profile / anyone's profile | yes / no |
| `login(username, password, save)` / `logout(forget)` | Sign in / out | |

Models (`Competition`, `DataFile`, `LeaderboardEntry`, `Participation`, `Submission`,
`SubmissionLimits`, `Team`, `User`, `Discussion`) implement `serde::Serialize` and keep
the untouched API payload in their `raw` field.

## Errors

Every method returns `zindi::Result<T>`. `zindi::Error` has a variant per situation:
`AlreadyJoined`, `SecretCodeRequired`, `Authentication`, `NotFound`, `RateLimited`,
`Api { status, message, errors }`, `CompetitionNotSet`, `Http`, `Io` and `Other`.

```rust,no_run
# async fn demo(client: zindi::Client) -> zindi::Result<()> {
match client.join(Some("Private-Challenge"), None).await {
    Ok(_) => println!("joined"),
    Err(zindi::Error::SecretCodeRequired { .. }) => println!("ask the hosts for the code"),
    Err(err) => return Err(err),
}
# Ok(()) }
```

## Configuration

| Builder option | Environment variable | Default |
|---|---|---|
| `base_url` | `ZINDI_API_URL` | `https://api.zindi.world/v1` |
| `token` | `ZINDI_TOKEN` | |
| `username`, `password` | `ZINDI_USERNAME`, `ZINDI_PASSWORD` | |
| `competition` | `ZINDI_COMPETITION` | |
| | `ZINDI_CONFIG_DIR` | see above |
| `timeout` | | 30 s per network operation |
| `max_retries`, `retry_backoff` | | 3 retries, 0.5 s doubling (max 30 s) |
| `poll_interval`, `wait_timeout` | | 10 s, 10 min |
| `use_saved_credentials` | | `true` |

Reads (GET, DELETE) are retried on connection errors, timeouts and HTTP 429/5xx,
honouring `Retry-After`. Writes are only retried when the connection could not be
opened, so a submission is never sent twice.

## Development

The tests serve the JSON fixtures shared by every client from `../spec/fixtures` with a
local mock server:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

## License

MIT, see [LICENSE](LICENSE).
