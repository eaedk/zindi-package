api_root <- "https://api.test/v1"
competition_id <- "Crop-Yield-Forecast-Challenge"

# The fixtures are shared by every client of the repository, in `spec/fixtures`.
# They are absent from a source tarball, in which case the tests are skipped.
# ZINDI_FIXTURES can point at them from anywhere. Paths are made absolute now,
# because tests change the working directory.
fixtures_candidates <- c(Sys.getenv("ZINDI_FIXTURES"), testthat::test_path("../../../spec/fixtures"))
fixtures_candidates <- normalizePath(fixtures_candidates[nzchar(fixtures_candidates)],
  winslash = "/", mustWork = FALSE)

fixtures_dir <- function() {
  found <- fixtures_candidates[nzchar(fixtures_candidates) & dir.exists(fixtures_candidates)]
  if (length(found) == 0) NULL else found[[1]]
}

fixture_path <- function(name) {
  dir <- fixtures_dir()
  if (is.null(dir)) testthat::skip("shared fixtures (spec/fixtures) not found")
  file.path(dir, paste0(name, ".json"))
}

fixture <- function(name) {
  jsonlite::read_json(fixture_path(name), simplifyVector = FALSE)
}

#' A response whose body is the fixture file, byte for byte.
fixture_response <- function(name, status = 200L) {
  path <- fixture_path(name)
  httr2::response(
    status_code = status,
    headers = list(`Content-Type` = "application/json"),
    body = readBin(path, "raw", file.size(path))
  )
}

json_response <- function(body, status = 200L) {
  httr2::response(
    status_code = status,
    headers = list(`Content-Type` = "application/json"),
    body = charToRaw(as.character(jsonlite::toJSON(body, auto_unbox = TRUE, null = "null")))
  )
}

raw_response <- function(body, status = 200L, headers = list()) {
  if (is.character(body)) body <- charToRaw(body)
  httr2::response(status_code = status, headers = headers, body = body)
}

#' Isolate a test: its own config dir and working dir, no ambient credentials,
#' an empty session, and no console chatter.
local_zindi <- function(env = parent.frame()) {
  root <- withr::local_tempdir(.local_envir = env)
  work <- file.path(root, "work")
  dir.create(work)
  withr::local_envvar(
    ZINDI_TOKEN = NA, ZINDI_USERNAME = NA, ZINDI_PASSWORD = NA, ZINDI_COMPETITION = NA,
    ZINDI_CONFIG_DIR = file.path(root, "config"), ZINDI_NO_PROMPT = "1",
    ZINDI_API_URL = api_root,
    .local_envir = env
  )
  withr::local_dir(work, .local_envir = env)
  withr::local_options(cli.default_handler = function(msg) invisible(NULL), .local_envir = env)
  reset_session()
  withr::defer(reset_session(), envir = env)
  invisible(root)
}

reset_session <- function() {
  zindi_logout(forget = FALSE)
  rm(list = intersect("competition", ls(the)), envir = the)
}

#' Route mocked requests by "METHOD path" (path relative to the API root, or a
#' full URL for other hosts). A route is a response, a function of the request,
#' or a list of responses served in turn (the last one repeats).
#' Returns an environment whose `requests` field records every request.
local_api <- function(routes, env = parent.frame()) {
  log <- new.env(parent = emptyenv())
  log$requests <- list()
  served <- list()
  httr2::local_mocked_responses(function(req) {
    url <- sub("\\?.*$", "", req$url)
    path <- if (startsWith(url, api_root)) sub(paste0("^", api_root, "/"), "", url) else url
    key <- paste(req$method, path)
    log$requests[[length(log$requests) + 1L]] <- req
    log$keys <- c(log$keys, key)
    route <- routes[[key]]
    if (is.null(route)) stop("Unexpected request: ", key, call. = FALSE)
    if (is.function(route)) {
      return(route(req))
    }
    if (inherits(route, "httr2_response")) {
      return(route)
    }
    n <- (served[[key]] %||% 0L) + 1L
    served[[key]] <<- n
    route[[min(n, length(route))]]
  }, env = env)
  log
}

calls_to <- function(log, key) sum(log$keys == key)

last_request <- function(log) log$requests[[length(log$requests)]]

request_headers <- function(req) {
  if (exists("req_get_headers", asNamespace("httr2"))) {
    httr2::req_get_headers(req, redacted = "reveal")
  } else {
    unclass(req$headers)
  }
}

request_query <- function(req) httr2::url_parse(req$url)$query %||% list()
