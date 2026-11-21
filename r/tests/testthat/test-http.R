test_that("error messages are extracted from every shape of `errors`", {
  expect_equal(error_message(list(message = "already in")), "already in")
  expect_equal(error_message(list(base = list("a", "b"))), "a; b")
  expect_equal(error_message(list(password = "wrong password")), "password: wrong password")
  expect_equal(error_message("plain"), "plain")
  expect_equal(error_message(NULL), "Unknown error")
})

test_that("errors are mapped to classed conditions", {
  local_zindi()
  local_api(list(
    "GET a" = fixture_response("error_not_authorized"),
    "GET b" = json_response(list(data = list(errors = list(message = "not found"))), 404),
    "GET c" = json_response(list(data = list(errors = list(message = "slow down"))), 429),
    "GET d" = fixture_response("join_already"),
    "GET e" = fixture_response("join_secret_required"),
    "GET f" = fixture_response("team_already_leader"),
    "GET g" = raw_response("", status = 500)
  ))
  expect_error(zindi_call("GET", "a", auth = FALSE), class = "zindi_auth_error")
  expect_error(zindi_call("GET", "b", auth = FALSE), class = "zindi_not_found")
  expect_error(zindi_call("GET", "c", auth = FALSE), class = "zindi_rate_limit")
  expect_error(zindi_call("GET", "d", auth = FALSE), class = "zindi_already_joined")
  expect_error(zindi_call("GET", "e", auth = FALSE), class = "zindi_secret_code_required")
  expect_error(zindi_call("GET", "f", auth = FALSE), "one team", class = "zindi_api_error")
  expect_error(zindi_call("GET", "g", auth = FALSE), class = "zindi_api_error")

  cnd <- tryCatch(zindi_call("GET", "b", auth = FALSE), zindi_error = function(cnd) cnd)
  expect_s3_class(cnd, "zindi_error")
  expect_equal(cnd$status_code, 404L)
  expect_equal(cnd$errors, list(message = "not found"))
})

test_that("the envelope is unwrapped", {
  local_zindi()
  local_api(list("GET users/amina_dev" = fixture_response("user_profile")))
  data <- zindi_call("GET", "users/amina_dev", auth = FALSE)
  expect_equal(data$username, "amina_dev")
})

test_that("a non-JSON success is reported", {
  local_zindi()
  local_api(list("GET users/x" = raw_response("<html>maintenance</html>")))
  expect_error(zindi_user("x"), "non-JSON", class = "zindi_api_error")
})

test_that("network failures are reported as API errors", {
  local_zindi()
  local_api(list("GET users/x" = function(req) rlang::abort("boom", class = "httr2_failure")))
  expect_error(zindi_user("x"), "Could not reach Zindi", class = "zindi_api_error")
})

test_that("requests identify the client and carry the token", {
  local_zindi()
  req <- zindi_request("GET", "me", token = "tok")
  headers <- request_headers(req)
  expect_match(req$options$useragent, "^zindi-r/1\\.0\\.0 \\(\\+https://github.com/eaedk/zindi-package\\)$")
  expect_equal(headers[["Current-Url"]], "https://zindi.world/")
  expect_equal(headers[["auth-token"]], "tok")
  expect_equal(headers[["auth_token"]], "tok")
  expect_equal(req$url, "https://api.test/v1/me")

  anonymous <- request_headers(zindi_request("GET", "me"))
  expect_null(anonymous[["auth-token"]])
})

test_that("the API root comes from ZINDI_API_URL", {
  withr::local_envvar(ZINDI_API_URL = NA)
  expect_equal(zindi_base_url(), "https://api.zindi.world/v1")
  withr::local_envvar(ZINDI_API_URL = "http://localhost:9999/v1/")
  expect_equal(zindi_base_url(), "http://localhost:9999/v1")
})

test_that("reads are retried with backoff, writes are not", {
  local_zindi()
  expect_equal(zindi_request("GET", "me")$policies$retry_max_tries, 4L)
  expect_equal(zindi_request("DELETE", "x")$policies$retry_max_tries, 4L)
  expect_null(zindi_request("POST", "x")$policies$retry_max_tries)
  is_transient <- zindi_request("GET", "me")$policies$retry_is_transient
  expect_true(is_transient(raw_response("", 503)))
  expect_true(is_transient(raw_response("", 429)))
  expect_false(is_transient(raw_response("", 404)))
  expect_equal(vapply(1:4, retry_backoff, numeric(1)), c(0.5, 1, 2, 4))
  expect_equal(retry_backoff(20), 30)
})

test_that("relative redirect locations are resolved", {
  expect_equal(absolute_url("https://s.example.com/f", "https://api.test/v1/x"), "https://s.example.com/f")
  expect_equal(absolute_url("/signed/f", "https://api.test/v1/x"), "https://api.test/signed/f")
  expect_equal(absolute_url("f2", "https://api.test/v1/x"), "https://api.test/v1/f2")
})

test_that("timestamps are parsed as UTC", {
  expect_equal(
    parse_time("2026-11-18T10:02:11.000Z"),
    as.POSIXct("2026-11-18 10:02:11", tz = "UTC")
  )
  expect_equal(
    parse_time("2026-11-18T12:02:11+02:00"),
    as.POSIXct("2026-11-18 10:02:11", tz = "UTC")
  )
  expect_true(is.na(parse_time(NULL)))
})
