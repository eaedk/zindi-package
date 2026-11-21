password_reset_url <- "https://zindi.world/passwords/request"

#' Turn Zindi's `errors` object, whatever its shape, into one readable sentence.
#' @noRd
error_message <- function(errors) {
  if (is.null(errors) || length(errors) == 0) {
    return("Unknown error")
  }
  if (is.character(errors) && length(errors) == 1) {
    return(errors)
  }
  if (is.list(errors) && !is.null(names(errors)) && all(nzchar(names(errors)))) {
    for (key in c("message", "base", "error")) {
      if (!is.null(errors[[key]])) {
        return(error_message(errors[[key]]))
      }
    }
    parts <- vapply(names(errors), function(key) {
      paste0(key, ": ", error_message(errors[[key]]))
    }, character(1))
    return(paste(parts, collapse = "; "))
  }
  if (is.list(errors) || length(errors) > 1) {
    return(paste(vapply(errors, error_message, character(1)), collapse = "; "))
  }
  as.character(errors)
}

zindi_abort <- function(message, class = NULL, ..., call = NULL) {
  rlang::abort(message, class = c(class, "zindi_error"), ..., call = call)
}

#' Raise the most specific error for an error response.
#' @noRd
abort_for_errors <- function(status_code, errors) {
  message <- error_message(errors)
  lowered <- tolower(message)
  class <- if (grepl("already in", lowered, fixed = TRUE) ||
    grepl("already joined", lowered, fixed = TRUE)) {
    "zindi_already_joined"
  } else if (grepl("secret code", lowered, fixed = TRUE)) {
    "zindi_secret_code_required"
  } else if (status_code %in% c(401L, 403L) ||
    grepl("not authorized", lowered, fixed = TRUE) ||
    grepl("not authenticated", lowered, fixed = TRUE)) {
    "zindi_auth_error"
  } else if (status_code == 404L || lowered == "not found") {
    "zindi_not_found"
  } else if (status_code == 429L) {
    "zindi_rate_limit"
  } else {
    "zindi_api_error"
  }
  zindi_abort(message, class = class, status_code = status_code, errors = errors)
}
