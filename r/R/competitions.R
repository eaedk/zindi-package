#' Choose the competition you work on
#'
#' Makes `id` the default competition for the rest of the session, so other
#' functions can be called without one. With `save = TRUE` it is also written
#' to `zindi.toml` in the working directory, which future sessions started
#' from this project (and the Python command line tool) pick up.
#'
#' `zindi_competition_id()` returns the competition a function would use:
#' `id` if given, else the session default, else the `ZINDI_COMPETITION`
#' environment variable, else the nearest `zindi.toml`.
#'
#' @param id A competition id, for example `"Crop-Yield-Forecast-Challenge"`.
#' @param save Also write it to `zindi.toml` in the working directory.
#' @return The competition id, invisibly for `zindi_use()`.
#' @export
#' @examples
#' \dontrun{
#' zindi_use("Crop-Yield-Forecast-Challenge", save = TRUE)
#' zindi_competition_id()
#' }
zindi_use <- function(id, save = FALSE) {
  check_string(id, "id")
  the$competition <- id
  if (save) {
    path <- write_project_competition(id)
    cli::cli_alert_success("Saved {.val {id}} to {.path {path}}.")
  }
  invisible(id)
}

#' @rdname zindi_use
#' @export
zindi_competition_id <- function(id = NULL) {
  id <- id %||% the$competition %||% env_value("ZINDI_COMPETITION") %||% read_project_competition()
  if (is.null(id)) {
    zindi_abort(
      paste(
        "No competition selected. Pass a competition id, call",
        "`zindi_use(\"<competition-id>\")`, or set ZINDI_COMPETITION."
      ),
      class = "zindi_competition_not_set"
    )
  }
  check_string(id, "id")
  id
}

#' List competitions
#'
#' Browse and search the competitions and hackathons on Zindi. No login is
#' needed.
#'
#' @param status `"open"` (default), `"closed"` or `"all"`.
#' @param kind `"competition"` or `"hackathon"`; both when `NULL`.
#' @param reward `"prize"`, `"points"` or `"knowledge"`; any when `NULL`.
#' @param search Free-text search on titles.
#' @param limit Maximum number of rows; `NULL` for all of them.
#' @return A data frame with one row per competition and the columns `id`,
#'   `title`, `kind`, `reward`, `reward_type`, `organization`, `is_open`,
#'   `start_time`, `end_time` (POSIXct, UTC), `participants`,
#'   `secret_code_required` and `is_participant`. Its `total` attribute holds
#'   the number of matches reported by the API.
#' @export
#' @examples
#' \dontrun{
#' zindi_competitions()
#' zindi_competitions(status = "all", kind = "hackathon", search = "speech")
#' }
zindi_competitions <- function(status = c("open", "closed", "all"), kind = NULL, reward = NULL,
                               search = NULL, limit = 50) {
  status <- rlang::arg_match(status)
  if (!is.null(kind)) kind <- rlang::arg_match(kind, c("competition", "hackathon"))
  if (!is.null(reward)) reward <- rlang::arg_match(reward, c("prize", "points", "knowledge"))
  query <- list(`kind[]` = kind, prize = reward, query = search,
    active = if (status == "open") "true")
  keep <- function(raw) {
    is_open <- as_lgl(raw[["open"]])
    item_kind <- as_chr(raw[["kind"]])
    item_reward <- as_chr(raw[["reward_type"]])
    !(status == "open" && !is_open) &&
      !(status == "closed" && is_open) &&
      (is.null(reward) || is.na(item_reward) || item_reward == reward) &&
      (is.null(kind) || is.na(item_kind) || item_kind == kind)
  }
  items <- zindi_paginate("competitions", query = query, per_page = 50L, limit = limit,
    auth = FALSE, keep = keep)
  df <- build_df(lapply(items, competition_summary), competition_columns)
  attr(df, "total") <- attr(items, "total")
  df
}

competition_columns <- list(
  id = character(), title = character(), kind = character(), reward = character(),
  reward_type = character(), organization = character(), is_open = logical(),
  start_time = .POSIXct(numeric(), tz = "UTC"), end_time = .POSIXct(numeric(), tz = "UTC"),
  participants = integer(), secret_code_required = logical(), is_participant = logical()
)

competition_summary <- function(raw) {
  list(
    id = as_chr(raw[["id"]] %||% ""),
    title = as_chr(first_of(raw, "title", "subtitle", "id") %||% ""),
    kind = as_chr(raw[["kind"]]),
    reward = as_chr(raw[["reward"]]),
    reward_type = as_chr(raw[["reward_type"]]),
    organization = as_chr(raw[["organization"]]),
    is_open = as_lgl(raw[["open"]]),
    start_time = parse_time(raw[["start_time"]]),
    end_time = parse_time(raw[["end_time"]]),
    participants = as_int(raw[["participations_count"]]),
    secret_code_required = as_lgl(raw[["secret_code_required"]]),
    is_participant = as_lgl(raw[["is_participant"]])
  )
}

#' Details of a competition
#'
#' Returns everything about a competition: dates, reward, metric, team size,
#' data files, description and rules. No login is needed.
#'
#' @param id A competition id. See [zindi_competition_id()] for the default.
#' @return A `zindi_competition` object: a list with the fields of
#'   [zindi_competitions()] plus `subtitle`, `metric`, `max_team_size`,
#'   `daily_submission_limit` (read from the rules), `description` and
#'   `rules` (as plain text), `url`, `data_files` (a data frame with
#'   `filename`, `size` in bytes, `description` and `zip_of_everything`) and
#'   the untouched API payload in `raw`.
#' @export
#' @examples
#' \dontrun{
#' comp <- zindi_competition("Crop-Yield-Forecast-Challenge")
#' comp
#' comp$data_files
#' }
zindi_competition <- function(id = NULL) {
  id <- zindi_competition_id(id)
  raw <- as_record(zindi_call("GET", paste0("competitions/", url_segment(id)), auth = FALSE))
  new_competition(raw)
}

new_competition <- function(raw) {
  pages <- list()
  for (page in raw[["pages"]] %||% list()) {
    if (!is.list(page)) next
    key <- tolower(as_chr(first_of(page, "url_title", "title") %||% ""))
    if (nzchar(key)) pages[[key]] <- as_chr(page[["content_html"]] %||% "")
  }
  rules <- raw[["rules"]]
  if (is.list(rules) && !is.null(rules[["content_html"]]) && is.null(pages[["rules"]])) {
    pages[["rules"]] <- as_chr(rules[["content_html"]])
  }
  description <- html_to_text(pages[["description"]] %||% (if (length(pages)) pages[[1]]))
  rules_text <- html_to_text(pages[["rules"]])
  limit <- regmatches(rules_text, regexec("maximum of\\s+(\\d+)\\s+submissions?\\s+per\\s+day",
    rules_text, ignore.case = TRUE))[[1]]
  files <- Filter(is.list, raw[["datafiles"]] %||% list())
  data_files <- build_df(
    lapply(files, function(f) {
      list(
        filename = as_chr(f[["filename"]] %||% ""),
        size = as_num(f[["filesize"]]),
        description = as_chr(f[["description"]]),
        zip_of_everything = as_lgl(f[["zip_of_everything"]])
      )
    }),
    list(filename = character(), size = numeric(), description = character(),
      zip_of_everything = logical())
  )
  summary <- competition_summary(raw)
  structure(
    c(
      summary,
      list(
        subtitle = as_chr(raw[["subtitle"]]),
        metric = as_chr(raw[["error_metric"]]),
        max_team_size = as_int(raw[["max_number_of_participants_per_team"]]),
        daily_submission_limit = if (length(limit) == 2) as.integer(limit[[2]]) else NA_integer_,
        url = paste0(web_url, "/competitions/", summary$id),
        description = description,
        rules = rules_text,
        data_files = data_files,
        raw = raw
      )
    ),
    class = "zindi_competition"
  )
}

#' @export
format.zindi_competition <- function(x, ...) {
  dates <- paste(format_time(x$start_time, "%Y-%m-%d"), "to", format_time(x$end_time, "%Y-%m-%d"))
  fields <- c(
    Id = x$id,
    Status = if (x$is_open) "open" else "closed",
    Kind = x$kind,
    Reward = x$reward,
    Host = x$organization,
    Dates = dates,
    Participants = x$participants,
    Metric = x$metric,
    `Team size` = x$max_team_size,
    `Daily limit` = x$daily_submission_limit,
    `Joined` = if (x$is_participant) "yes" else "no",
    Url = x$url
  )
  fields <- fields[!is.na(fields) & nzchar(fields)]
  out <- c(paste0("<zindi_competition> ", x$title), paste0("  ", format(names(fields)), "  ", fields))
  if (nrow(x$data_files) > 0) {
    files <- paste0("    ", format(x$data_files$filename), "  ", format(format_size(x$data_files$size),
      justify = "right"))
    out <- c(out, "  Data files:", files)
  }
  out
}

#' @export
print.zindi_competition <- function(x, ...) {
  cat(format(x, ...), sep = "\n")
  invisible(x)
}

#' Join a competition
#'
#' Joins a competition, which accepts its rules and gives access to its data.
#' Joining a competition you are already in is not an error.
#'
#' @inheritParams zindi_competition
#' @param secret_code The secret code of a private competition.
#' @return `TRUE` if you just joined, `FALSE` if you were already in,
#'   invisibly. A private competition without `secret_code` raises an error of
#'   class `zindi_secret_code_required`.
#' @export
#' @examples
#' \dontrun{
#' zindi_join("Crop-Yield-Forecast-Challenge")
#' }
zindi_join <- function(id = NULL, secret_code = NULL) {
  id <- zindi_competition_id(id)
  body <- if (!is.null(secret_code)) {
    function(req) httr2::req_body_json(req, list(secret_code = secret_code))
  }
  joined <- tryCatch(
    {
      zindi_call("POST", paste0("competitions/", url_segment(id), "/participations"), body = body)
      TRUE
    },
    zindi_already_joined = function(cnd) FALSE
  )
  if (joined) {
    cli::cli_alert_success("Joined {.val {id}}.")
  } else {
    cli::cli_alert_info("You are already a participant of {.val {id}}.")
  }
  invisible(joined)
}

#' Download a competition's data
#'
#' Downloads the data files of a competition you have joined. Files that are
#' already in `destination` with the expected size are skipped, so this is
#' safe to call at the top of every script. Each file is written to a `.part`
#' file first and renamed when complete, so an interrupted download never
#' leaves a truncated file behind.
#'
#' @inheritParams zindi_competition
#' @param destination Folder to download into (created if needed).
#' @param files File names to download. By default, every file except the
#'   archive that bundles all of them, unless that archive is the only file.
#' @param overwrite Download files again even if they are already there.
#' @param unzip Extract `.zip` files into `destination` after downloading.
#' @return The paths of the files on disk, invisibly.
#' @export
#' @examples
#' \dontrun{
#' zindi_download("Crop-Yield-Forecast-Challenge", "data")
#' zindi_download(files = "SampleSubmission.csv")
#' }
zindi_download <- function(id = NULL, destination = "data", files = NULL, overwrite = FALSE,
                           unzip = FALSE) {
  id <- zindi_competition_id(id)
  comp <- zindi_competition(id)
  chosen <- choose_files(comp$data_files, files)
  dir.create(destination, recursive = TRUE, showWarnings = FALSE)
  token <- current_token(required = TRUE)

  paths <- file.path(destination, chosen$filename)
  for (i in seq_len(nrow(chosen))) {
    path <- paths[[i]]
    size <- chosen$size[[i]]
    if (!overwrite && file.exists(path) && (is.na(size) || file.size(path) == size)) {
      cli::cli_alert_info("{.file {path}} is already there.")
      next
    }
    download_data_file(id, chosen$filename[[i]], path, token)
    cli::cli_alert_success("Downloaded {.file {path}}.")
  }
  if (unzip) {
    for (path in paths[grepl("\\.zip$", paths, ignore.case = TRUE)]) {
      safe_unzip(path, destination)
    }
  }
  invisible(paths)
}

choose_files <- function(available, wanted) {
  if (!is.null(wanted)) {
    missing <- setdiff(wanted, available$filename)
    if (length(missing) > 0) {
      known <- if (nrow(available)) paste(available$filename, collapse = ", ") else "none"
      zindi_abort(sprintf("Unknown file(s): %s. Available: %s", paste(missing, collapse = ", "), known))
    }
    return(available[match(wanted, available$filename), , drop = FALSE])
  }
  individual <- available[!available$zip_of_everything, , drop = FALSE]
  if (nrow(individual) > 0) individual else available
}

download_data_file <- function(id, filename, path, token) {
  part <- paste0(path, ".part")
  on.exit(unlink(part), add = TRUE)
  tryCatch(
    download_to(paste0("competitions/", url_segment(id), "/files/", url_segment(filename)), part, token),
    zindi_auth_error = function(cnd) {
      zindi_abort(
        sprintf(
          "Zindi refused the download of %s: %s. Have you joined the competition? Try `zindi_join(\"%s\")`.",
          filename, conditionMessage(cnd), id
        ),
        class = "zindi_auth_error",
        status_code = cnd$status_code,
        errors = cnd$errors
      )
    }
  )
  if (file.exists(path)) unlink(path)
  if (!file.rename(part, path)) {
    zindi_abort(sprintf("Could not move the download to %s.", path))
  }
  invisible(path)
}

#' Extract a zip file, refusing entries that would land outside `target`.
#' @noRd
safe_unzip <- function(archive, target) {
  entries <- utils::unzip(archive, list = TRUE)$Name
  unsafe <- unsafe_entries(entries)
  if (length(unsafe) > 0) {
    zindi_abort(sprintf("Refusing to extract '%s' from %s: unsafe path.", unsafe[[1]], basename(archive)))
  }
  utils::unzip(archive, exdir = target)
  invisible(file.path(target, entries))
}

unsafe_entries <- function(entries) {
  absolute <- grepl("^([/\\\\]|[A-Za-z]:)", entries)
  climbs <- vapply(strsplit(entries, "[/\\\\]"), function(parts) any(parts == ".."), logical(1))
  entries[absolute | climbs]
}
