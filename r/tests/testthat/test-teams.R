team_route <- "competitions/Crop-Yield-Forecast-Challenge/my_team"

test_that("a team is created, grown, shown and disbanded", {
  local_zindi()
  zindi_auth(token = "tok_test")
  routes <- list(
    fixture_response("team_created"),
    list(fixture_response("team_invite_ok"), fixture_response("team_invite_already")),
    fixture_response("my_team"),
    fixture_response("team_disbanded")
  )
  names(routes) <- c(
    paste("POST", team_route), paste0("POST ", team_route, "/invite"),
    paste("GET", team_route), paste("DELETE", team_route)
  )
  api <- local_api(routes)

  team <- zindi_team_create("Lagos Learners", invite = c("zawadi_k", "tolu_a"),
    competition = competition_id)

  expect_s3_class(team, "zindi_team")
  expect_equal(team$title, "Lagos Learners")
  expect_equal(team$leader, "amina_dev")
  create <- api$requests[[1]]
  expect_equal(create$body$type, "form")
  expect_equal(curl::curl_unescape(create$body$data$title), "Lagos Learners")
  invites <- Filter(function(req) endsWith(req$url, "/invite"), api$requests)
  expect_length(invites, 2)
  expect_equal(curl::curl_unescape(invites[[1]]$body$data$username), "zawadi_k")

  shown <- zindi_team(competition_id)
  expect_equal(shown$members, c("amina_dev", "tolu_a"))
  expect_equal(shown$pending_invitations, "zawadi_k")
  expect_output(print(shown), "Lagos Learners")

  expect_true(zindi_team_disband(competition_id))
  expect_equal(calls_to(api, paste("DELETE", team_route)), 1)
})

test_that("zindi_team() is NULL for solo participants", {
  local_zindi()
  zindi_auth(token = "tok_test")
  local_api(stats::setNames(
    list(json_response(list(data = list(errors = list(message = "not found"))), 404)),
    paste("GET", team_route)
  ))
  expect_null(zindi_team(competition_id))
})

test_that("other invitation errors are raised", {
  local_zindi()
  zindi_auth(token = "tok_test")
  local_api(stats::setNames(
    list(json_response(list(data = list(errors = list(base = "User does not exist"))))),
    paste0("POST ", team_route, "/invite")
  ))
  expect_error(zindi_team_invite("ghost", competition = competition_id), "does not exist",
    class = "zindi_api_error")
})

test_that("creating a second team is refused", {
  local_zindi()
  zindi_auth(token = "tok_test")
  local_api(stats::setNames(list(fixture_response("team_already_leader")), paste("POST", team_route)))
  expect_error(zindi_team_create("Again", competition = competition_id), "leader of one team")
})
