#' Discussions of a competition
#'
#' Lists the discussion threads of a competition, newest first. No login is
#' needed.
#'
#' @inheritParams zindi_competition
#' @param limit Maximum number of rows; `NULL` for all of them.
#' @return A data frame with the columns `id`, `title`, `author`, `date`
#'   (POSIXct, UTC), `comments_count`, `pinned` and `url`.
#' @export
#' @examples
#' \dontrun{
#' zindi_discussions("Crop-Yield-Forecast-Challenge", limit = 5)
#' }
zindi_discussions <- function(id = NULL, limit = 20) {
  id <- zindi_competition_id(id)
  items <- zindi_paginate(paste0("competitions/", url_segment(id), "/discussions"),
    per_page = 20L, limit = limit, auth = FALSE)
  rows <- lapply(items, function(raw) {
    thread <- as_chr(raw[["id"]] %||% "")
    slug <- as_chr(raw[["url_title"]])
    list(
      id = thread,
      title = as_chr(raw[["title"]] %||% ""),
      author = as_chr(as_record(raw[["user"]])[["username"]]),
      date = parse_time(first_of(raw, "published_at", "created_at")),
      comments_count = as_int(raw[["comments_count"]] %||% 0L),
      pinned = as_lgl(raw[["pinned"]]),
      url = paste0(web_url, "/competitions/", id, "/discussions/", thread,
        if (!is.na(slug)) paste0("/", slug) else "")
    )
  })
  build_df(rows, list(
    id = character(), title = character(), author = character(),
    date = .POSIXct(numeric(), tz = "UTC"), comments_count = integer(), pinned = logical(),
    url = character()
  ))
}
