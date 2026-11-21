#' Your team in a competition
#'
#' @inheritParams zindi_competition
#' @return A `zindi_team` object (a list with `id`, `title`, `leader`,
#'   `members`, `pending_invitations` and the untouched API payload in `raw`),
#'   or `NULL` if you compete alone.
#' @seealso [zindi_team_create()], [zindi_team_invite()],
#'   [zindi_team_disband()].
#' @export
#' @examples
#' \dontrun{
#' zindi_team("Crop-Yield-Forecast-Challenge")
#' }
zindi_team <- function(id = NULL) {
  id <- zindi_competition_id(id)
  data <- tryCatch(
    zindi_call("GET", team_path(id)),
    zindi_error = function(cnd) {
      not_found <- identical(cnd$status_code, 404L) ||
        grepl("not found", conditionMessage(cnd), ignore.case = TRUE)
      if (!not_found) stop(cnd)
      NULL
    }
  )
  if (!is.list(data) || length(data) == 0) {
    return(NULL)
  }
  new_team(data)
}

team_path <- function(id, ...) paste0("competitions/", url_segment(id), "/my_team", ...)

new_team <- function(raw) {
  names_of <- function(items) {
    out <- vapply(items %||% list(), function(item) {
      if (is.list(item)) as_chr(item[["username"]]) else as_chr(item)
    }, character(1))
    out[!is.na(out)]
  }
  leader <- raw[["leader"]]
  structure(
    list(
      id = as_chr(raw[["id"]]),
      title = as_chr(raw[["title"]] %||% ""),
      leader = as_chr(raw[["leader_username"]] %||%
        (if (is.list(leader)) leader[["username"]] else leader)),
      members = names_of(raw[["users"]] %||% raw[["members"]]),
      pending_invitations = names_of(raw[["invited_users"]] %||% raw[["invitations"]]),
      raw = raw
    ),
    class = "zindi_team"
  )
}

#' @export
format.zindi_team <- function(x, ...) {
  fields <- c(
    Leader = x$leader,
    Members = paste(x$members, collapse = ", "),
    Invited = paste(x$pending_invitations, collapse = ", ")
  )
  fields <- fields[!is.na(fields) & nzchar(fields)]
  c(paste0("<zindi_team> ", x$title), paste0("  ", format(names(fields)), "  ", fields))
}

#' @export
print.zindi_team <- function(x, ...) {
  cat(format(x, ...), sep = "\n")
  invisible(x)
}

#' Create, grow and disband a team
#'
#' `zindi_team_create()` creates a team (you become its leader) and
#' optionally invites people. `zindi_team_invite()` invites users to your
#' team; people who already have a pending invitation count as invited.
#' `zindi_team_disband()` disbands your team (you must be its leader).
#'
#' @param name The team name.
#' @param invite,usernames Usernames to invite.
#' @inheritParams zindi_submit
#' @param id A competition id. See [zindi_competition_id()] for the default.
#' @return `zindi_team_create()`: the new team (a `zindi_team` object).
#'   `zindi_team_invite()`: the usernames invited, invisibly.
#'   `zindi_team_disband()`: `TRUE`, invisibly.
#' @export
#' @examples
#' \dontrun{
#' zindi_team_create("Lagos Learners", invite = "tolu_a")
#' zindi_team_invite(c("zawadi_k", "kofi_ml"))
#' zindi_team_disband()
#' }
zindi_team_create <- function(name, invite = character(), competition = NULL) {
  id <- zindi_competition_id(competition)
  check_string(name, "name")
  data <- zindi_call("POST", team_path(id), body = function(req) {
    httr2::req_body_form(req, title = name)
  })
  team <- new_team(if (is.list(data) && length(data)) data else list(title = name))
  cli::cli_alert_success("Created team {.val {team$title}} in {.val {id}}.")
  if (length(invite) > 0) {
    zindi_team_invite(invite, competition = id)
  }
  team
}

#' @rdname zindi_team_create
#' @export
zindi_team_invite <- function(usernames, competition = NULL) {
  id <- zindi_competition_id(competition)
  for (name in usernames) {
    tryCatch(
      zindi_call("POST", team_path(id, "/invite"), body = function(req) {
        httr2::req_body_form(req, username = name)
      }),
      zindi_error = function(cnd) {
        if (!grepl("already invited", conditionMessage(cnd), ignore.case = TRUE)) stop(cnd)
      }
    )
    cli::cli_alert_success("Invited {.val {name}}.")
  }
  invisible(as.character(usernames))
}

#' @rdname zindi_team_create
#' @export
zindi_team_disband <- function(id = NULL) {
  id <- zindi_competition_id(id)
  zindi_call("DELETE", team_path(id))
  cli::cli_alert_success("Disbanded your team in {.val {id}}.")
  invisible(TRUE)
}
