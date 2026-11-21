#' zindi: compete on Zindi from R
#'
#' The zindi package wraps the API behind <https://zindi.world>: browse
#' competitions, join them, download their data, submit, wait for your score,
#' follow the leaderboard, manage your team and read discussions.
#'
#' @section Credentials:
#' Browsing competitions, leaderboards, profiles and discussions needs no
#' account. Everything else needs an auth token, which zindi looks for in this
#' order:
#'
#' 1. a token or username and password given to [zindi_auth()] (or obtained by
#'    [zindi_login()]) in the current session;
#' 2. the `ZINDI_TOKEN` environment variable;
#' 3. the `ZINDI_USERNAME` and `ZINDI_PASSWORD` environment variables (zindi
#'    signs in the first time a token is needed);
#' 4. the credentials file saved by [zindi_login()], shared with the Python,
#'    JavaScript and Rust clients (see [zindi_config_dir()]);
#' 5. in an interactive session, a login prompt.
#'
#' @section Choosing a competition:
#' Functions that act on a competition take its id (the slug in its URL, for
#' example `"Crop-Yield-Forecast-Challenge"`). When you leave it out, zindi
#' uses, in order: the competition chosen with [zindi_use()], the
#' `ZINDI_COMPETITION` environment variable, then the `competition` entry of
#' the nearest `zindi.toml` file in the working directory or one of its
#' parents.
#'
#' @section Environment variables and options:
#' * `ZINDI_API_URL`: API root, `https://api.zindi.world/v1` by default.
#' * `ZINDI_CONFIG_DIR`: where the credentials file lives.
#' * `ZINDI_NO_PROMPT`: set to any value to never prompt for credentials.
#' * `options(zindi.max_tries = 4)`: attempts for reads that fail with HTTP 429
#'   or 5xx, or because the network is down. Uploads and other writes are
#'   never retried.
#'
#' @section Errors:
#' Errors raised by zindi inherit from the class `zindi_error`, plus one of
#' `zindi_auth_error`, `zindi_not_found`, `zindi_rate_limit`,
#' `zindi_already_joined`, `zindi_secret_code_required` or `zindi_api_error`
#' for errors returned by the API. Their `status_code` and `errors` fields hold
#' the HTTP status and the raw error object.
#'
#' @keywords internal
"_PACKAGE"
