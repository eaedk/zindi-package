submissions_route <- "GET competitions/Crop-Yield-Forecast-Challenge/submissions"
submit_route <- "POST competitions/Crop-Yield-Forecast-Challenge/submissions"

test_that("a submission uploads the file and the comment", {
  local_zindi()
  zindi_auth(token = "tok_test")
  api <- local_api(stats::setNames(list(fixture_response("submission_created")), submit_route))
  writeLines(c("ID,Yield", "1,2.5"), "submission.csv")

  sub <- zindi_submit("submission.csv", comment = "first try", competition = competition_id)

  expect_equal(sub$id, "sub_d4")
  expect_equal(sub$state, "pending")
  req <- last_request(api)
  expect_equal(req$body$type, "multipart")
  expect_equal(req$body$data$comment, "first try")
  expect_s3_class(req$body$data$file, "form_file")
  expect_equal(basename(req$body$data$file$path), "submission.csv")
  expect_null(req$policies$retry_max_tries)
})

test_that("wait = TRUE polls until the submission is scored", {
  local_zindi()
  zindi_auth(token = "tok_test")
  pending <- list(data = list(fixture("submission_created")$data))
  routes <- list(
    fixture_response("submission_created"),
    list(json_response(pending), fixture_response("submission_scored"))
  )
  names(routes) <- c(submit_route, submissions_route)
  api <- local_api(routes)
  writeLines("x", "submission.csv")

  sub <- zindi_submit("submission.csv", competition = competition_id, wait = TRUE, poll_interval = 0)

  expect_equal(sub$state, "scored")
  expect_equal(sub$public_score, 0.4211)
  expect_equal(calls_to(api, submissions_route), 2)
})

test_that("waiting for a score times out", {
  local_zindi()
  zindi_auth(token = "tok_test")
  pending <- list(data = list(fixture("submission_created")$data))
  local_api(stats::setNames(list(json_response(pending)), submissions_route))
  expect_error(
    zindi_wait_for_score("sub_d4", competition = competition_id, timeout = 0),
    "not scored", class = "zindi_timeout"
  )
})

test_that("a missing submission file is reported before any request", {
  local_zindi()
  expect_error(zindi_submit("nope.csv", competition = competition_id), "not found")
})

test_that("submissions and their states", {
  local_zindi()
  zindi_auth(token = "tok_test")
  local_api(stats::setNames(list(fixture_response("submissions")), submissions_route))

  subs <- zindi_submissions(competition_id)

  expect_equal(subs$state, c("pending", "scored", "failed"))
  expect_equal(subs$status_description[[3]], "Missing column: Yield")
  expect_equal(subs$score[[2]], 0.4377)
  expect_equal(subs$selected, c(FALSE, TRUE, FALSE))
  expect_s3_class(subs$created_at, "POSIXct")
  expect_equal(nrow(zindi_submissions(competition_id, limit = 1)), 1)
})

test_that("remaining submissions", {
  local_zindi()
  zindi_auth(token = "tok_test")
  local_api(list(
    "GET competitions/Crop-Yield-Forecast-Challenge/submissions/limits" =
      fixture_response("submission_limits")
  ))
  expect_equal(zindi_limits(competition_id)$remaining_today, 7L)
})
