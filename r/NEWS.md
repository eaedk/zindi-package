# zindi 1.0.0

* First release of the R client, in step with the Python, JavaScript and Rust
  clients of the same repository.
* Browse competitions (`zindi_competitions()`, `zindi_competition()`), join
  them, download their data, submit and wait for scores, follow leaderboards,
  manage teams and read discussions.
* Credentials are shared with the other clients through
  `credentials.json` in `zindi_config_dir()`, and can also come from
  `ZINDI_TOKEN` or `ZINDI_USERNAME` / `ZINDI_PASSWORD`.
* The default competition can be set with `zindi_use()`, `ZINDI_COMPETITION`
  or a `zindi.toml` project file.
* Reads are retried on HTTP 429 and 5xx; uploads are never replayed.
