# Session state: credentials given in this R session and the default competition.
the <- new.env(parent = emptyenv())

env_value <- function(name) {
  value <- Sys.getenv(name)
  if (nzchar(value)) value else NULL
}

#' Find credentials, without prompting or signing in.
#' @return A list with `token`, `username`, `password` and `source` (any may be NULL).
#' @noRd
resolve_credentials <- function() {
  if (!is.null(the$token) || (!is.null(the$username) && !is.null(the$password))) {
    return(list(token = the$token, username = the$username, password = the$password,
      source = the$source %||% "session"))
  }
  env_token <- env_value("ZINDI_TOKEN")
  env_user <- env_value("ZINDI_USERNAME")
  env_password <- env_value("ZINDI_PASSWORD")
  if (!is.null(env_token) || (!is.null(env_user) && !is.null(env_password))) {
    return(list(token = env_token, username = env_user, password = env_password,
      source = "environment variables"))
  }
  saved <- load_credentials()
  if (!is.null(saved)) {
    username <- if (is.na(saved$username)) NULL else saved$username
    return(list(token = saved$auth_token, username = username, source = credentials_path()))
  }
  list()
}

#' Return the token to send, signing in or prompting when one is required.
#' @noRd
current_token <- function(required = TRUE) {
  creds <- resolve_credentials()
  if (!is.null(creds$token) || !required) {
    return(creds$token)
  }
  if (!is.null(creds$username) && !is.null(creds$password)) {
    sign_in(creds$username, creds$password, source = creds$source)
    return(the$token)
  }
  if (can_prompt()) {
    cli::cli_inform("Log in to Zindi (your token will be saved for next time).")
    zindi_login()
    return(the$token)
  }
  zindi_abort(
    paste(
      "You are not logged in. Run `zindi_login()`, or set ZINDI_TOKEN",
      "(or ZINDI_USERNAME and ZINDI_PASSWORD)."
    ),
    class = "zindi_auth_error"
  )
}

sign_in <- function(username, password, source = NULL) {
  data <- tryCatch(
    zindi_call("POST", "auth/signin", auth = FALSE, send_token = FALSE, body = function(req) {
      httr2::req_body_form(req, username = username, password = password)
    }),
    zindi_error = function(cnd) {
      fields <- if (is.list(cnd$errors)) names(cnd$errors) else character()
      wrong <- any(c("password", "username") %in% fields) ||
        grepl("login", conditionMessage(cnd), ignore.case = TRUE)
      if (!wrong) stop(cnd)
      zindi_abort(
        paste0(
          "Login failed: ", conditionMessage(cnd), ". Check your username and password. ",
          "Forgot your password? Reset it at ", password_reset_url
        ),
        class = "zindi_auth_error",
        status_code = cnd$status_code,
        errors = cnd$errors
      )
    }
  )
  token <- if (is.list(data)) data[["auth_token"]]
  if (is.null(token) || !nzchar(as_chr(token))) {
    zindi_abort("Zindi did not return an auth token.", class = "zindi_auth_error")
  }
  user <- as_record(data[["user"]])
  the$token <- as_chr(token)
  the$username <- as_chr(user[["username"]] %||% username)
  the$password <- password
  the$source <- source %||% "password"
  invisible(data)
}

#' Log in to Zindi
#'
#' Signs in with your username and password and, with `save = TRUE`, stores
#' the auth token (never the password) in the credentials file shared by every
#' zindi client, so later R sessions, the Python command line tool and the
#' other clients are logged in too.
#'
#' Missing values are taken from the `ZINDI_USERNAME` and `ZINDI_PASSWORD`
#' environment variables, or asked for in an interactive session.
#'
#' @param username Your Zindi username.
#' @param password Your Zindi password. Leave it out to be prompted for it
#'   without echo, rather than typing it in a script.
#' @param save Save the auth token to the credentials file.
#' @return Your profile (a `zindi_user` object), invisibly.
#' @seealso [zindi_logout()], [zindi_whoami()], [zindi_auth()].
#' @export
#' @examplesIf interactive()
#' zindi_login("amina_dev")
zindi_login <- function(username = NULL, password = NULL, save = TRUE) {
  username <- username %||% env_value("ZINDI_USERNAME")
  if (is.null(password) && identical(username, env_value("ZINDI_USERNAME"))) {
    password <- env_value("ZINDI_PASSWORD")
  }
  if (is.null(username) || !nzchar(username)) {
    if (!can_prompt()) {
      zindi_abort("A username is required to log in.", class = "zindi_auth_error")
    }
    username <- trimws(readline("Zindi username: "))
  }
  if (is.null(password) || !nzchar(password)) {
    if (!can_prompt()) {
      zindi_abort("A password is required to log in.", class = "zindi_auth_error")
    }
    password <- ask_password(sprintf("Zindi password for %s: ", username))
  }
  data <- sign_in(username, password, source = "password")
  if (save) {
    path <- save_credentials(the$username, the$token)
    cli::cli_alert_success("Logged in as {.strong {the$username}}. Token saved to {.path {path}}.")
  } else {
    cli::cli_alert_success("Logged in as {.strong {the$username}}.")
  }
  user <- as_record(data[["user"]])
  if (length(user) == 0) user <- list(username = the$username)
  invisible(new_user(user))
}

ask_password <- function(prompt) {
  if (requireNamespace("askpass", quietly = TRUE)) {
    return(askpass::askpass(prompt) %||% "")
  }
  readline(prompt)
}

#' Log out of Zindi
#'
#' Forgets the credentials held by the current session and, with
#' `forget = TRUE`, deletes the shared credentials file (which logs out the
#' other zindi clients on this computer too).
#'
#' @param forget Also delete the saved credentials file.
#' @return `TRUE` if a credentials file was deleted, invisibly.
#' @export
#' @examplesIf interactive()
#' zindi_logout()
zindi_logout <- function(forget = TRUE) {
  rm(list = intersect(c("token", "username", "password", "source"), ls(the)), envir = the)
  deleted <- if (forget) delete_credentials() else FALSE
  if (deleted) cli::cli_alert_success("Deleted {.path {credentials_path()}}.")
  invisible(deleted)
}

#' Set credentials for the current session
#'
#' Use a token or a username and password for the rest of the R session,
#' without saving anything to disk. Session credentials take precedence over
#' environment variables and the saved credentials file. With a username and
#' password, zindi signs in the first time a token is needed.
#'
#' @param token An auth token.
#' @param username,password Your Zindi username and password.
#' @return The previous session credentials, invisibly.
#' @export
#' @examples
#' \dontrun{
#' zindi_auth(token = Sys.getenv("MY_ZINDI_TOKEN"))
#' }
zindi_auth <- function(token = NULL, username = NULL, password = NULL) {
  if (is.null(token) && (is.null(username) || is.null(password))) {
    zindi_abort("Give a `token`, or both `username` and `password`.")
  }
  previous <- mget(c("token", "username", "password", "source"), envir = the,
    ifnotfound = list(NULL))
  zindi_logout(forget = FALSE)
  the$token <- token
  the$username <- username
  the$password <- password
  the$source <- "session"
  invisible(previous)
}

#' Your own Zindi profile
#'
#' `zindi_whoami()` returns the profile of the logged-in user, and
#' `zindi_user()` the public profile of any user.
#'
#' @param username A Zindi username.
#' @return A `zindi_user` object: a list with `username`, `full_name`,
#'   `country`, `points`, `rank`, `best_rank`, `competitions_count`,
#'   `submissions_count`, `medals` (gold, silver, bronze), `joined_at`, `url`
#'   and the untouched API payload in `raw`.
#' @export
#' @examples
#' \dontrun{
#' zindi_whoami()
#' zindi_user("amina_dev")
#' }
zindi_whoami <- function() {
  data <- as_record(zindi_call("GET", "me"))
  # `/me` answers list(auth_token = ..., user = list(...)); older versions sent the user itself.
  if (is.list(data[["user"]])) data <- data[["user"]]
  new_user(data)
}

#' @rdname zindi_whoami
#' @export
zindi_user <- function(username) {
  check_string(username, "username")
  new_user(as_record(zindi_call("GET", paste0("users/", url_segment(username)), auth = FALSE)))
}

new_user <- function(raw) {
  username <- as_chr(raw[["username"]] %||% "")
  name_parts <- c(as_chr(raw[["first_name"]]), as_chr(raw[["last_name"]]))
  structure(
    list(
      username = username,
      full_name = paste(name_parts[!is.na(name_parts)], collapse = " "),
      title = as_chr(raw[["title"]]),
      country = as_chr(raw[["country"]]),
      countrycode = as_chr(raw[["countrycode"]]),
      points = as_int(raw[["points"]]),
      rank = as_int(raw[["rank"]]),
      best_rank = as_int(raw[["best_rank"]]),
      competitions_count = as_int(raw[["count_of_competitions"]]),
      submissions_count = as_int(raw[["count_of_submissions"]]),
      medals = c(
        gold = as_int(raw[["user_medals_summary_gold_count"]] %||% 0L),
        silver = as_int(raw[["user_medals_summary_silver_count"]] %||% 0L),
        bronze = as_int(raw[["user_medals_summary_bronze_count"]] %||% 0L)
      ),
      joined_at = parse_time(raw[["created_at"]]),
      url = paste0(web_url, "/users/", username),
      raw = raw
    ),
    class = "zindi_user"
  )
}

#' @export
format.zindi_user <- function(x, ...) {
  fields <- c(
    Name = x$full_name,
    Country = x$country,
    Rank = x$rank,
    Points = x$points,
    Medals = sprintf("%d gold, %d silver, %d bronze", x$medals[["gold"]], x$medals[["silver"]],
      x$medals[["bronze"]]),
    Joined = format_time(x$joined_at, "%Y-%m-%d"),
    Profile = x$url
  )
  fields <- fields[!is.na(fields) & nzchar(fields)]
  c(paste0("<zindi_user> ", x$username), paste0("  ", format(names(fields)), "  ", fields))
}

#' @export
print.zindi_user <- function(x, ...) {
  cat(format(x, ...), sep = "\n")
  invisible(x)
}

check_string <- function(x, arg) {
  if (!is.character(x) || length(x) != 1 || is.na(x) || !nzchar(x)) {
    zindi_abort(sprintf("`%s` must be a single non-empty string.", arg))
  }
}
