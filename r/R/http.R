default_base_url <- "https://api.zindi.world/v1"
retry_statuses <- c(429L, 500L, 502L, 503L, 504L)

zindi_base_url <- function() {
  url <- Sys.getenv("ZINDI_API_URL")
  if (!nzchar(url)) url <- default_base_url
  sub("/+$", "", url)
}

zindi_user_agent <- function() {
  paste0("zindi-r/", utils::packageVersion("zindi"), " (+https://github.com/eaedk/zindi-package)")
}

api_url <- function(path) {
  if (grepl("^https?://", path)) path else paste0(zindi_base_url(), "/", sub("^/+", "", path))
}

retry_backoff <- function(attempt) min(0.5 * 2^(attempt - 1), 30)

#' Build a request that follows Zindi's conventions.
#'
#' Reads (GET, DELETE) are retried on HTTP 429/5xx and network failures; other
#' methods are sent once, so an upload that may have reached the server is
#' never replayed. HTTP errors are handled by `zindi_unwrap()`, not by httr2.
#' @noRd
zindi_request <- function(method, path, token = NULL, query = list()) {
  req <- httr2::request(api_url(path))
  req <- httr2::req_method(req, method)
  req <- httr2::req_user_agent(req, zindi_user_agent())
  req <- httr2::req_headers(req, Accept = "application/json", `Current-Url` = paste0(web_url, "/"))
  if (!is.null(token)) {
    # The web app sends `auth-token`; older endpoints read `auth_token`.
    req <- httr2::req_headers(req, `auth-token` = token, auth_token = token,
      .redact = c("auth-token", "auth_token"))
  }
  query <- compact(query)
  if (length(query) > 0) {
    req <- httr2::req_url_query(req, !!!query)
  }
  req <- httr2::req_options(req, connecttimeout = 15)
  req <- httr2::req_error(req, is_error = function(resp) FALSE)
  if (method %in% c("GET", "DELETE")) {
    req <- httr2::req_retry(
      req,
      max_tries = getOption("zindi.max_tries", 4L),
      retry_on_failure = TRUE,
      is_transient = function(resp) httr2::resp_status(resp) %in% retry_statuses,
      backoff = retry_backoff
    )
  }
  req
}

perform <- function(req, path = NULL) {
  tryCatch(
    httr2::req_perform(req, path = path),
    httr2_failure = function(cnd) {
      zindi_abort(
        paste0("Could not reach Zindi: ", conditionMessage(cnd)),
        class = "zindi_api_error",
        parent = cnd
      )
    }
  )
}

#' Parse a response body: return `list(data, meta)` or raise a classed error.
#' @noRd
parse_envelope <- function(status, body, reason = NULL) {
  reason <- reason %||% "HTTP error"
  if (!nzchar(trimws(body))) {
    if (status >= 400) abort_for_errors(status, reason)
    return(list(data = NULL, meta = NULL))
  }
  payload <- tryCatch(jsonlite::fromJSON(body, simplifyVector = FALSE), error = function(e) e)
  if (inherits(payload, "error")) {
    if (status < 400) {
      preview <- substr(gsub("\n", " ", trimws(body), fixed = TRUE), 1, 200)
      zindi_abort(
        sprintf("Zindi returned a non-JSON response (HTTP %d): %s", status, preview),
        class = "zindi_api_error",
        status_code = status
      )
    }
    abort_for_errors(status, reason)
  }
  data <- payload
  meta <- NULL
  if (is.list(payload) && "data" %in% names(payload)) {
    data <- payload[["data"]]
    meta <- payload[["meta"]]
  }
  errors <- if (is.list(data) && !is.null(names(data))) data[["errors"]]
  if (is.null(errors) && is.list(payload) && "errors" %in% names(payload)) {
    errors <- payload[["errors"]]
  }
  has_errors <- length(errors) > 0 && !isFALSE(errors) && !identical(errors, "")
  if (has_errors || status >= 400) {
    abort_for_errors(status, if (has_errors) errors else reason)
  }
  list(data = data, meta = meta)
}

zindi_unwrap <- function(resp) {
  body <- if (httr2::resp_has_body(resp)) httr2::resp_body_string(resp) else ""
  parse_envelope(httr2::resp_status(resp), body, httr2::resp_status_desc(resp))
}

#' Fetch one page of a list endpoint, or follow pages until a stop condition.
#'
#' @param keep Function of one item: `TRUE` to keep it in the result.
#' @param stop_at Function of one item: `TRUE` to stop before it.
#' @return A list of items with a `total` attribute (`meta.total_count`).
#' @noRd
zindi_paginate <- function(path, query = list(), per_page = 100L, limit = NULL,
                           auth = FALSE, keep = NULL, stop_at = NULL) {
  items <- list()
  total <- NULL
  page <- 0L
  seen <- 0L
  repeat {
    result <- zindi_call("GET", path, query = c(query, list(page = page, per_page = per_page)),
      auth = auth, envelope = TRUE)
    batch <- if (is.list(result$data) && is.null(names(result$data))) result$data else list()
    if (!is.null(result$meta[["total_count"]])) total <- as_int(result$meta[["total_count"]])
    for (item in batch) {
      seen <- seen + 1L
      if (!is.null(stop_at) && stop_at(item)) {
        return(structure(items, total = total))
      }
      if (is.null(keep) || keep(item)) {
        items[[length(items) + 1L]] <- item
      }
      if (!is.null(limit) && length(items) >= limit) {
        return(structure(items, total = total))
      }
    }
    if (length(batch) < per_page || (!is.null(total) && seen >= total)) {
      return(structure(items, total = total))
    }
    page <- page + 1L
  }
}

#' Make an API call and return the `data` member of the envelope.
#'
#' With `auth = TRUE` a token is required (signing in or prompting if needed);
#' if it turns out to be expired and a password is known, zindi signs in again
#' and retries once. With `auth = FALSE` a known token is still sent.
#' @noRd
zindi_call <- function(method, path, query = list(), auth = TRUE, body = NULL, envelope = FALSE,
                       send_token = TRUE) {
  send <- function(token) {
    req <- zindi_request(method, path, token = token, query = query)
    if (!is.null(body)) req <- body(req)
    result <- zindi_unwrap(perform(req))
    if (envelope) result else result$data
  }
  token <- if (send_token) current_token(required = auth)
  result <- tryCatch(send(token), zindi_auth_error = function(cnd) cnd)
  if (!inherits(result, "zindi_auth_error")) {
    return(result)
  }
  creds <- resolve_credentials()
  if (!auth || is.null(creds$username) || is.null(creds$password)) {
    stop(result)
  }
  sign_in(creds$username, creds$password)
  send(the$token)
}

absolute_url <- function(location, base) {
  if (grepl("^[a-zA-Z][a-zA-Z0-9+.-]*://", location)) {
    return(location)
  }
  parts <- httr2::url_parse(base)
  origin <- paste0(parts$scheme, "://", parts$hostname, if (!is.null(parts$port)) paste0(":", parts$port))
  if (startsWith(location, "//")) {
    return(paste0(parts$scheme, ":", location))
  }
  if (startsWith(location, "/")) {
    return(paste0(origin, location))
  }
  paste0(origin, sub("[^/]*$", "", parts$path %||% "/"), location)
}

url_host <- function(url) httr2::url_parse(url)$hostname

#' Download `path` to `destination`, following redirects by hand so the auth
#' token is never sent to another host (for example object storage).
#' @noRd
download_to <- function(path, destination, token, progress = interactive()) {
  url <- api_url(path)
  for (i in seq_len(5)) {
    req <- zindi_request("GET", url, token = token)
    req <- httr2::req_options(req, followlocation = 0L)
    if (progress) req <- httr2::req_progress(req)
    resp <- perform(req, path = destination)
    status <- httr2::resp_status(resp)
    location <- httr2::resp_header(resp, "location")
    if (status %in% c(301L, 302L, 303L, 307L, 308L) && !is.null(location)) {
      unlink(destination)
      next_url <- absolute_url(location, url)
      if (!identical(url_host(next_url), url_host(url))) token <- NULL
      url <- next_url
      next
    }
    if (is.raw(resp$body)) {
      # Mocked or in-memory responses carry their body instead of a file.
      writeBin(resp$body, destination)
    }
    if (status >= 400) {
      body <- if (file.exists(destination)) {
        paste(readLines(destination, warn = FALSE), collapse = "\n")
      } else {
        ""
      }
      unlink(destination)
      parse_envelope(status, body, httr2::resp_status_desc(resp))
    }
    return(invisible(destination))
  }
  zindi_abort("Too many redirects while downloading the file.", class = "zindi_api_error")
}
