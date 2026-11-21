#' Leaderboard of a competition
#'
#' Returns the leaderboard (private ranks once they are revealed, public ranks
#' before). No login is needed.
#'
#' @inheritParams zindi_competition
#' @param limit Maximum number of rows; `NULL` for all of them.
#' @param include_unranked Also return participants without a valid score.
#' @return A data frame with the columns `rank`, `name` (user or team),
#'   `score`, `is_team`, `username` (`NA` for teams), `members` (team
#'   members, comma-separated), `submission_count` and `last_submission_at`
#'   (POSIXct, UTC). Its `total` attribute holds the number of participants
#'   reported by the API.
#' @export
#' @examples
#' \dontrun{
#' zindi_leaderboard("Crop-Yield-Forecast-Challenge", limit = 10)
#' }
zindi_leaderboard <- function(id = NULL, limit = NULL, include_unranked = FALSE) {
  id <- zindi_competition_id(id)
  stop_at <- if (!include_unranked) {
    # Unranked rows come last: nobody after the first one has a valid score.
    function(raw) is.na(as_int(first_of(raw, "private_rank", "public_rank")))
  }
  items <- zindi_paginate(paste0("competitions/", url_segment(id), "/participations"),
    per_page = 100L, limit = limit, auth = FALSE, stop_at = stop_at)
  df <- build_df(lapply(items, leaderboard_row), list(
    rank = integer(), name = character(), score = numeric(), is_team = logical(),
    username = character(), members = character(), submission_count = integer(),
    last_submission_at = .POSIXct(numeric(), tz = "UTC")
  ))
  attr(df, "total") <- attr(items, "total")
  df
}

leaderboard_row <- function(raw) {
  user <- if (is.list(raw[["user"]])) raw[["user"]]
  team <- if (is.list(raw[["team"]])) raw[["team"]]
  members <- vapply(Filter(function(m) is.list(m) && !is.null(m[["username"]]), team[["members"]]),
    function(m) as_chr(m[["username"]]), character(1))
  list(
    rank = as_int(first_of(raw, "private_rank", "public_rank")),
    name = as_chr(team[["title"]] %||% user[["username"]] %||% "?"),
    score = as_num(first_of(raw, "best_private_score", "best_public_score")),
    is_team = !is.null(team),
    username = as_chr(user[["username"]]),
    members = paste(members, collapse = ", "),
    submission_count = as_int(raw[["submission_count"]] %||% 0L),
    last_submission_at = parse_time(first_of(raw, "best_private_submitted_at", "best_public_submitted_at"))
  )
}

#' Your standing in a competition
#'
#' `zindi_my_participation()` returns your rank, best score and number of
#' submissions. `zindi_rank()` returns just your rank, falling back to a
#' search of the leaderboard by username.
#'
#' @inheritParams zindi_competition
#' @return `zindi_my_participation()`: a list with `rank`, `score`,
#'   `submission_count`, `team_title` and the untouched API payload in `raw`.
#'   `zindi_rank()`: an integer, `NA` if you have no valid score yet.
#' @export
#' @examples
#' \dontrun{
#' zindi_my_participation("Crop-Yield-Forecast-Challenge")
#' zindi_rank("Crop-Yield-Forecast-Challenge")
#' }
zindi_my_participation <- function(id = NULL) {
  id <- zindi_competition_id(id)
  raw <- as_record(zindi_call("GET",
    paste0("competitions/", url_segment(id), "/participations/my_participation")))
  team <- as_record(raw[["team"]])
  list(
    rank = as_int(first_of(raw, "private_rank", "public_rank")),
    score = as_num(first_of(raw, "best_private_score", "best_public_score")),
    submission_count = as_int(raw[["submission_count"]] %||% 0L),
    team_title = as_chr(team[["title"]]),
    raw = raw
  )
}

#' @rdname zindi_my_participation
#' @export
zindi_rank <- function(id = NULL) {
  id <- zindi_competition_id(id)
  rank <- tryCatch(zindi_my_participation(id)$rank, zindi_error = function(cnd) NA_integer_)
  if (!is.na(rank)) {
    return(rank)
  }
  username <- resolve_credentials()$username
  if (is.null(username) || is.na(username)) {
    return(NA_integer_)
  }
  board <- zindi_leaderboard(id)
  wanted <- tolower(username)
  hit <- vapply(seq_len(nrow(board)), function(i) {
    members <- strsplit(board$members[[i]], ", ", fixed = TRUE)[[1]]
    identical(tolower(board$username[[i]]), wanted) || wanted %in% tolower(members)
  }, logical(1))
  if (any(hit)) board$rank[which(hit)[[1]]] else NA_integer_
}
