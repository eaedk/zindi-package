web_url <- "https://zindi.world"

`%||%` <- function(x, y) if (is.null(x)) y else x

compact <- function(x) x[!vapply(x, is.null, logical(1))]

# Scalar coercions that turn NULL (missing JSON values) into NA.
as_chr <- function(x) {
  if (is.null(x) || length(x) == 0) NA_character_ else as.character(x[[1]])
}

as_int <- function(x) {
  if (is.null(x) || length(x) == 0) {
    return(NA_integer_)
  }
  suppressWarnings(as.integer(x[[1]]))
}

as_num <- function(x) {
  if (is.null(x) || length(x) == 0) {
    return(NA_real_)
  }
  suppressWarnings(as.numeric(x[[1]]))
}

as_lgl <- function(x, default = FALSE) {
  if (is.null(x) || length(x) == 0 || is.na(x[[1]])) default else isTRUE(as.logical(x[[1]]))
}

# The first non-NULL value among `keys` of a list.
first_of <- function(x, ...) {
  for (key in c(...)) {
    if (!is.null(x[[key]])) {
      return(x[[key]])
    }
  }
  NULL
}

as_record <- function(x) if (is.list(x)) x else list()

#' Parse an ISO 8601 timestamp such as `2026-11-18T10:02:11.000Z`.
#' @noRd
parse_time <- function(x) {
  if (is.null(x) || length(x) == 0 || !is.character(x) || is.na(x[[1]]) || !nzchar(x[[1]])) {
    return(.POSIXct(NA_real_, tz = "UTC"))
  }
  value <- x[[1]]
  offset <- 0
  zone <- regmatches(value, regexpr("([+-]\\d{2}:?\\d{2}|Z)$", value))
  if (length(zone) == 1) {
    value <- substr(value, 1, nchar(value) - nchar(zone))
    if (zone != "Z") {
      digits <- gsub(":", "", zone, fixed = TRUE)
      sign <- if (startsWith(digits, "-")) -1 else 1
      offset <- sign * (as.integer(substr(digits, 2, 3)) * 3600 + as.integer(substr(digits, 4, 5)) * 60)
    }
  }
  value <- sub("\\.\\d+$", "", value)
  parsed <- as.POSIXct(value, format = "%Y-%m-%dT%H:%M:%S", tz = "UTC")
  if (is.na(parsed)) {
    parsed <- as.POSIXct(value, format = "%Y-%m-%d", tz = "UTC")
  }
  .POSIXct(as.numeric(parsed) - offset, tz = "UTC")
}

# Build a data frame from a list of records (named lists of scalars).
# `proto` gives the columns, in order, as zero-length vectors of the right type.
build_df <- function(records, proto) {
  columns <- lapply(names(proto), function(name) {
    template <- proto[[name]]
    if (inherits(template, "POSIXct")) {
      values <- vapply(records, function(r) as.numeric(r[[name]] %||% NA_real_), numeric(1))
      return(.POSIXct(values, tz = "UTC"))
    }
    cast <- switch(typeof(template),
      character = as.character,
      integer = as.integer,
      double = as.double,
      logical = as.logical
    )
    vapply(records, function(r) cast(r[[name]] %||% NA), template[NA_integer_])
  })
  names(columns) <- names(proto)
  df <- as.data.frame(columns, stringsAsFactors = FALSE, check.names = FALSE)
  rownames(df) <- NULL
  df
}

# A very small HTML-to-text conversion, good enough for rules and descriptions.
html_to_text <- function(content) {
  if (is.null(content) || length(content) == 0 || is.na(content) || !nzchar(content)) {
    return("")
  }
  text <- gsub("<\\s*(br|/p|/li|/h\\d|/div)\\s*/?>", "\n", content, ignore.case = TRUE, perl = TRUE)
  text <- gsub("<\\s*li[^>]*>", "\u2022 ", text, ignore.case = TRUE, perl = TRUE)
  text <- gsub("<[^>]+>", "", text, perl = TRUE)
  entities <- c(
    "&nbsp;" = " ", "&lt;" = "<", "&gt;" = ">", "&quot;" = "\"",
    "&#39;" = "'", "&apos;" = "'", "&amp;" = "&"
  )
  for (entity in names(entities)) {
    text <- gsub(entity, entities[[entity]], text, fixed = TRUE)
  }
  text <- gsub("\n{3,}", "\n\n", text, perl = TRUE)
  trimws(text)
}

format_size <- function(bytes) {
  vapply(bytes, function(size) {
    if (is.na(size)) {
      return("?")
    }
    units <- c("B", "KB", "MB", "GB")
    i <- 1
    while (size >= 1024 && i < length(units)) {
      size <- size / 1024
      i <- i + 1
    }
    if (i == 1) sprintf("%.0f B", size) else sprintf("%.1f %s", size, units[[i]])
  }, character(1))
}

format_time <- function(x, format = "%Y-%m-%d %H:%M") {
  ifelse(is.na(x), "", format(x, format, tz = "UTC"))
}

url_segment <- function(x) curl::curl_escape(x)

can_prompt <- function() {
  interactive() && !nzchar(Sys.getenv("ZINDI_NO_PROMPT"))
}
