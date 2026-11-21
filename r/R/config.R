#' Where zindi keeps its files
#'
#' `zindi_config_dir()` returns the folder that holds the shared credentials
#' file, `credentials.json`. The file stores your username and an auth token
#' (never your password) and is shared by every zindi client (R, Python,
#' JavaScript, Rust), so logging in once is enough.
#'
#' The folder is, in order of priority: `ZINDI_CONFIG_DIR`; `%APPDATA%/zindi`
#' on Windows; `$XDG_CONFIG_HOME/zindi`; `~/.config/zindi`.
#'
#' @return A path (the folder may not exist yet).
#' @export
#' @examples
#' zindi_config_dir()
zindi_config_dir <- function() {
  override <- Sys.getenv("ZINDI_CONFIG_DIR")
  if (nzchar(override)) {
    return(path.expand(override))
  }
  appdata <- Sys.getenv("APPDATA")
  if (.Platform$OS.type == "windows" && nzchar(appdata)) {
    return(file.path(appdata, "zindi"))
  }
  xdg <- Sys.getenv("XDG_CONFIG_HOME")
  if (nzchar(xdg)) {
    return(file.path(path.expand(xdg), "zindi"))
  }
  file.path(path.expand("~"), ".config", "zindi")
}

credentials_path <- function() file.path(zindi_config_dir(), "credentials.json")

load_credentials <- function() {
  path <- credentials_path()
  if (!file.exists(path)) {
    return(NULL)
  }
  data <- tryCatch(jsonlite::read_json(path, simplifyVector = FALSE), error = function(e) NULL)
  token <- data$auth_token
  if (!is.list(data) || is.null(token) || !nzchar(as_chr(token))) {
    return(NULL)
  }
  list(username = as_chr(data$username), auth_token = as_chr(token))
}

save_credentials <- function(username, auth_token) {
  path <- credentials_path()
  dir.create(dirname(path), recursive = TRUE, showWarnings = FALSE)
  payload <- list(
    username = username,
    auth_token = auth_token,
    saved_at = format(Sys.time(), "%Y-%m-%dT%H:%M:%S+00:00", tz = "UTC")
  )
  tmp <- tempfile(".credentials-", tmpdir = dirname(path), fileext = ".json")
  on.exit(unlink(tmp), add = TRUE)
  writeLines(jsonlite::toJSON(payload, auto_unbox = TRUE, pretty = TRUE), tmp, useBytes = TRUE)
  if (.Platform$OS.type != "windows") {
    Sys.chmod(tmp, "0600", use_umask = FALSE)
  }
  if (!file.rename(tmp, path)) {
    file.copy(tmp, path, overwrite = TRUE)
    if (.Platform$OS.type != "windows") Sys.chmod(path, "0600", use_umask = FALSE)
  }
  invisible(path)
}

delete_credentials <- function() {
  path <- credentials_path()
  file.exists(path) && unlink(path) == 0
}

project_file <- "zindi.toml"
competition_line <- "^\\s*competition\\s*="

find_project_file <- function(start = getwd()) {
  here <- normalizePath(start, winslash = "/", mustWork = FALSE)
  repeat {
    candidate <- file.path(here, project_file)
    if (file.exists(candidate) && !dir.exists(candidate)) {
      return(candidate)
    }
    parent <- dirname(here)
    if (identical(parent, here)) {
      return(NULL)
    }
    here <- parent
  }
}

read_project_competition <- function(start = getwd()) {
  path <- find_project_file(start)
  if (is.null(path)) {
    return(NULL)
  }
  lines <- tryCatch(readLines(path, warn = FALSE, encoding = "UTF-8"), error = function(e) character())
  line <- grep(competition_line, lines, value = TRUE, perl = TRUE)
  if (length(line) == 0) {
    return(NULL)
  }
  value <- trimws(sub(competition_line, "", line[[1]], perl = TRUE))
  value <- sub("\\s+#.*$", "", value, perl = TRUE)
  if (grepl('^".*"$', value)) {
    value <- tryCatch(jsonlite::fromJSON(value), error = function(e) NA_character_)
  } else if (grepl("^'.*'$", value)) {
    value <- substr(value, 2, nchar(value) - 1)
  }
  if (length(value) != 1 || is.na(value) || !nzchar(value)) NULL else value
}

write_project_competition <- function(competition, folder = getwd()) {
  path <- file.path(folder, project_file)
  lines <- character()
  if (file.exists(path)) {
    lines <- readLines(path, warn = FALSE, encoding = "UTF-8")
    lines <- lines[!grepl(competition_line, lines, perl = TRUE)]
  }
  # A JSON string literal is also a valid TOML basic string.
  entry <- paste("competition =", jsonlite::toJSON(competition, auto_unbox = TRUE))
  writeLines(c(entry, lines), path, useBytes = TRUE)
  invisible(path)
}
