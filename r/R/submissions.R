#' Submit a file
#'
#' Uploads a submission file (a CSV, or whatever the competition expects).
#' Uploads are never retried automatically, so a submission is never counted
#' twice.
#'
#' @param file Path of the file to submit.
#' @param comment A note to remember what this submission was.
#' @param competition A competition id. See [zindi_competition_id()] for the
#'   default.
#' @param wait Wait until Zindi has scored the submission, and return it with
#'   its score.
#' @param timeout,poll_interval With `wait = TRUE`, give up after `timeout`
#'   seconds, checking every `poll_interval` seconds.
#' @return The submission, as a one-row data frame with the columns described
#'   in [zindi_submissions()].
#' @export
#' @examples
#' \dontrun{
#' zindi_submit("submission.csv", comment = "lgbm v1", wait = TRUE)
#' }
zindi_submit <- function(file, comment = "", competition = NULL, wait = FALSE, timeout = 600,
                         poll_interval = 10) {
  id <- zindi_competition_id(competition)
  check_string(file, "file")
  if (!file.exists(file) || dir.exists(file)) {
    zindi_abort(sprintf("Submission file not found: %s", file))
  }
  data <- zindi_call(
    "POST",
    paste0("competitions/", url_segment(id), "/submissions"),
    body = function(req) {
      httr2::req_body_multipart(req, file = curl::form_file(file), comment = comment)
    }
  )
  submission <- submissions_df(list(as_record(data)))
  cli::cli_alert_success("Submitted {.file {basename(file)}} to {.val {id}}.")
  if (wait && !is.na(submission$id) && nzchar(submission$id)) {
    return(zindi_wait_for_score(submission$id, competition = id, timeout = timeout,
      poll_interval = poll_interval))
  }
  submission
}

#' Your submissions
#'
#' Lists your (or your team's) submissions to a competition, newest first.
#'
#' @inheritParams zindi_competition
#' @param limit Maximum number of rows; `NULL` for all of them.
#' @return A data frame with the columns `id`, `created_at` (POSIXct, UTC),
#'   `filename`, `status` (as returned by Zindi), `state` (`"scored"`,
#'   `"pending"` or `"failed"`), `public_score`, `private_score`, `score`
#'   (private if known, else public), `comment`, `status_description` (the
#'   error message of a failed submission) and `selected`.
#' @export
#' @examples
#' \dontrun{
#' zindi_submissions("Crop-Yield-Forecast-Challenge")
#' }
zindi_submissions <- function(id = NULL, limit = NULL) {
  id <- zindi_competition_id(id)
  data <- zindi_call("GET", paste0("competitions/", url_segment(id), "/submissions"),
    query = list(per_page = limit %||% 1000L))
  records <- Filter(is.list, if (is.list(data) && is.null(names(data))) data else list())
  if (!is.null(limit)) records <- utils::head(records, limit)
  submissions_df(records)
}

submissions_df <- function(records) {
  rows <- lapply(records, function(raw) {
    status <- as_chr(raw[["status"]] %||% "unknown")
    public <- as_num(raw[["public_score"]])
    private <- as_num(raw[["private_score"]])
    failed <- !status %in% c("successful", "initial")
    state <- if (failed) "failed" else if (!is.na(public) || !is.na(private)) "scored" else "pending"
    list(
      id = as_chr(raw[["id"]] %||% ""),
      created_at = parse_time(raw[["created_at"]]),
      filename = as_chr(raw[["filename"]]),
      status = status,
      state = state,
      public_score = public,
      private_score = private,
      score = if (is.na(private)) public else private,
      comment = as_chr(raw[["comment"]]),
      status_description = as_chr(raw[["status_description"]]),
      selected = as_lgl(raw[["chosen"]] %||% raw[["selected"]])
    )
  })
  build_df(rows, list(
    id = character(), created_at = .POSIXct(numeric(), tz = "UTC"), filename = character(),
    status = character(), state = character(), public_score = numeric(),
    private_score = numeric(), score = numeric(), comment = character(),
    status_description = character(), selected = logical()
  ))
}

#' Wait for a submission's score
#'
#' Checks your submissions every `poll_interval` seconds until the given one
#' is scored or has failed.
#'
#' @param submission_id The id of the submission, as returned by
#'   [zindi_submit()].
#' @inheritParams zindi_submit
#' @return The submission, as a one-row data frame (see
#'   [zindi_submissions()]). Raises an error if it is still pending after
#'   `timeout` seconds.
#' @export
#' @examples
#' \dontrun{
#' sub <- zindi_submit("submission.csv")
#' zindi_wait_for_score(sub$id)
#' }
zindi_wait_for_score <- function(submission_id, competition = NULL, timeout = 600,
                                 poll_interval = 10) {
  id <- zindi_competition_id(competition)
  deadline <- Sys.time() + timeout
  repeat {
    subs <- zindi_submissions(id)
    found <- subs[subs$id == as.character(submission_id) & subs$state != "pending", , drop = FALSE]
    if (nrow(found) > 0) {
      return(found[1, , drop = FALSE])
    }
    if (Sys.time() >= deadline) {
      zindi_abort(sprintf(
        "Submission %s was not scored within %.0fs. Check later with `zindi_submissions(\"%s\")`.",
        submission_id, timeout, id
      ), class = "zindi_timeout")
    }
    Sys.sleep(poll_interval)
  }
}

#' Remaining submissions
#'
#' @inheritParams zindi_competition
#' @return A list with `remaining_today`, the number of submissions you can
#'   still make today (`NA` if unknown), and the untouched API payload in
#'   `raw`.
#' @export
#' @examples
#' \dontrun{
#' zindi_limits("Crop-Yield-Forecast-Challenge")$remaining_today
#' }
zindi_limits <- function(id = NULL) {
  id <- zindi_competition_id(id)
  raw <- as_record(zindi_call("GET", paste0("competitions/", url_segment(id), "/submissions/limits")))
  list(remaining_today = as_int(first_of(raw, "today", "remaining_today")), raw = raw)
}
