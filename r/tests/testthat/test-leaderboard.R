board_route <- "GET competitions/Crop-Yield-Forecast-Challenge/participations"
mine_route <- "GET competitions/Crop-Yield-Forecast-Challenge/participations/my_participation"

test_that("the leaderboard stops at the first unranked row", {
  local_zindi()
  api <- local_api(stats::setNames(list(fixture_response("leaderboard")), board_route))

  board <- zindi_leaderboard(competition_id)

  expect_named(board, c(
    "rank", "name", "score", "is_team", "username", "members", "submission_count",
    "last_submission_at"
  ))
  expect_equal(board$rank, 1:3)
  expect_equal(board$name[[2]], "Lagos Learners")
  expect_true(board$is_team[[2]])
  expect_true(is.na(board$username[[2]]))
  expect_equal(board$members[[2]], "tolu_a, amina_dev")
  expect_equal(board$score[[1]], 0.4123)
  expect_equal(board$last_submission_at[[1]], as.POSIXct("2026-11-18 10:02:11", tz = "UTC"))
  expect_equal(attr(board, "total"), 4L)
  expect_equal(request_query(last_request(api))$per_page, "100")
  expect_equal(nrow(zindi_leaderboard(competition_id, include_unranked = TRUE)), 4)
  expect_equal(nrow(zindi_leaderboard(competition_id, limit = 2)), 2)
})

test_that("rank and participation come from my_participation", {
  local_zindi()
  zindi_auth(token = "tok_test")
  local_api(stats::setNames(list(fixture_response("my_participation")), mine_route))
  expect_equal(zindi_rank(competition_id), 2L)
  mine <- zindi_my_participation(competition_id)
  expect_equal(mine$team_title, "Lagos Learners")
  expect_equal(mine$submission_count, 41L)
  expect_equal(mine$score, 0.4377)
})

test_that("rank falls back to a search of the leaderboard", {
  local_zindi()
  zindi_auth(token = "t", username = "zawadi_k")
  routes <- list(json_response(list(data = list(public_rank = NULL))), fixture_response("leaderboard"))
  names(routes) <- c(mine_route, board_route)
  local_api(routes)
  expect_equal(zindi_rank(competition_id), 3L)

  zindi_auth(token = "t", username = "AMINA_DEV")
  expect_equal(zindi_rank(competition_id), 2L)

  zindi_auth(token = "t", username = "nobody")
  expect_true(is.na(zindi_rank(competition_id)))
})
