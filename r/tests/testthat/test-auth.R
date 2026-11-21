test_that("login saves the token but never the password", {
  local_zindi()
  api <- local_api(list("POST auth/signin" = fixture_response("signin_ok")))

  user <- zindi_login("amina_dev", "hunter2")

  expect_s3_class(user, "zindi_user")
  expect_equal(user$username, "amina_dev")
  body <- last_request(api)$body
  expect_equal(body$type, "form")
  expect_equal(unclass(body$data$username), "amina_dev")
  expect_equal(unclass(body$data$password), "hunter2")
  expect_null(request_headers(last_request(api))[["auth-token"]])

  path <- file.path(zindi_config_dir(), "credentials.json")
  saved <- jsonlite::read_json(path)
  expect_equal(saved$auth_token, "tok_3f9a1c2e7b")
  expect_equal(saved$username, "amina_dev")
  expect_false(is.null(saved$saved_at))
  expect_false(any(grepl("hunter2", readLines(path))))
  skip_on_os("windows")
  expect_equal(format(file.mode(path)), "600")
})

test_that("a wrong password points to the reset page", {
  local_zindi()
  local_api(list("POST auth/signin" = fixture_response("signin_wrong_password")))
  expect_error(zindi_login("amina_dev", "nope"), "passwords/request", class = "zindi_auth_error")
  expect_false(file.exists(file.path(zindi_config_dir(), "credentials.json")))
})

test_that("saved credentials are used", {
  local_zindi()
  save_credentials("amina_dev", "tok_saved")
  api <- local_api(list("GET me" = fixture_response("me")))
  expect_equal(zindi_whoami()$username, "amina_dev")
  expect_equal(request_headers(last_request(api))[["auth-token"]], "tok_saved")
  expect_equal(request_headers(last_request(api))[["auth_token"]], "tok_saved")
})

test_that("ZINDI_TOKEN beats the saved file", {
  local_zindi()
  save_credentials("someone", "tok_saved")
  withr::local_envvar(ZINDI_TOKEN = "tok_env")
  api <- local_api(list("GET me" = fixture_response("me")))
  zindi_whoami()
  expect_equal(request_headers(last_request(api))[["auth-token"]], "tok_env")
})

test_that("a session token beats everything", {
  local_zindi()
  withr::local_envvar(ZINDI_TOKEN = "tok_env")
  zindi_auth(token = "tok_session")
  api <- local_api(list("GET me" = fixture_response("me")))
  zindi_whoami()
  expect_equal(request_headers(last_request(api))[["auth-token"]], "tok_session")
})

test_that("ZINDI_USERNAME and ZINDI_PASSWORD sign in lazily, once", {
  local_zindi()
  withr::local_envvar(ZINDI_USERNAME = "amina_dev", ZINDI_PASSWORD = "hunter2")
  api <- local_api(list(
    "POST auth/signin" = fixture_response("signin_ok"),
    "GET me" = fixture_response("me"),
    "GET competitions/x" = fixture_response("competition")
  ))
  zindi_competition("x")
  expect_equal(calls_to(api, "POST auth/signin"), 0)
  zindi_whoami()
  zindi_whoami()
  expect_equal(calls_to(api, "POST auth/signin"), 1)
  expect_equal(request_headers(last_request(api))[["auth-token"]], "tok_3f9a1c2e7b")
  expect_false(file.exists(file.path(zindi_config_dir(), "credentials.json")))
})

test_that("an expired token is refreshed once", {
  local_zindi()
  zindi_auth(token = "old", username = "amina_dev", password = "hunter2")
  api <- local_api(list(
    "POST auth/signin" = fixture_response("signin_ok"),
    "GET me" = list(fixture_response("error_not_authorized", 401), fixture_response("me"))
  ))
  expect_equal(zindi_whoami()$username, "amina_dev")
  expect_equal(calls_to(api, "POST auth/signin"), 1)
  expect_equal(calls_to(api, "GET me"), 2)
  expect_equal(request_headers(last_request(api))[["auth-token"]], "tok_3f9a1c2e7b")
})

test_that("'not authorized' inside a 200 envelope is an auth error", {
  local_zindi()
  zindi_auth(token = "tok_test")
  local_api(list("GET me" = fixture_response("error_not_authorized")))
  expect_error(zindi_whoami(), class = "zindi_auth_error")
})

test_that("calls that need a login explain how to log in", {
  local_zindi()
  expect_error(zindi_whoami(), "zindi_login", class = "zindi_auth_error")
  expect_error(zindi_login(), "username", class = "zindi_auth_error")
  expect_error(zindi_login("amina_dev"), "password", class = "zindi_auth_error")
})

test_that("logout forgets everything", {
  local_zindi()
  local_api(list("POST auth/signin" = fixture_response("signin_ok")))
  zindi_login("amina_dev", "hunter2")
  expect_true(zindi_logout())
  expect_false(file.exists(file.path(zindi_config_dir(), "credentials.json")))
  expect_null(current_token(required = FALSE))
  expect_false(zindi_logout())
})

test_that("zindi_auth() needs a token or a username and password", {
  local_zindi()
  expect_error(zindi_auth(username = "amina_dev"), class = "zindi_error")
})

test_that("the config dir follows ZINDI_CONFIG_DIR, then XDG_CONFIG_HOME", {
  skip_on_os("windows")
  withr::local_envvar(ZINDI_CONFIG_DIR = "/tmp/zindi-a", XDG_CONFIG_HOME = "/tmp/xdg")
  expect_equal(zindi_config_dir(), "/tmp/zindi-a")
  withr::local_envvar(ZINDI_CONFIG_DIR = NA)
  expect_equal(zindi_config_dir(), file.path("/tmp/xdg", "zindi"))
  withr::local_envvar(XDG_CONFIG_HOME = NA)
  expect_equal(zindi_config_dir(), file.path(path.expand("~"), ".config", "zindi"))
})

test_that("an unreadable credentials file is ignored", {
  local_zindi()
  dir.create(zindi_config_dir(), recursive = TRUE)
  writeLines("{not json", file.path(zindi_config_dir(), "credentials.json"))
  expect_null(current_token(required = FALSE))
})

test_that("profiles are parsed", {
  local_zindi()
  local_api(list("GET users/amina_dev" = fixture_response("user_profile")))
  user <- zindi_user("amina_dev")
  expect_equal(user$full_name, "Amina Diallo")
  expect_equal(unname(user$medals), c(1L, 2L, 4L))
  expect_equal(user$best_rank, 3L)
  expect_equal(user$url, "https://zindi.world/users/amina_dev")
  expect_output(print(user), "amina_dev")
})
