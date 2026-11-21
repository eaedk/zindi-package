test_that("open competitions are listed without a login", {
  local_zindi()
  api <- local_api(list("GET competitions" = fixture_response("competitions")))

  comps <- zindi_competitions()

  expect_s3_class(comps, "data.frame")
  expect_equal(comps$id, c(competition_id, "Swahili-ASR-Hackathon"))
  expect_named(comps, c(
    "id", "title", "kind", "reward", "reward_type", "organization", "is_open",
    "start_time", "end_time", "participants", "secret_code_required", "is_participant"
  ))
  expect_s3_class(comps$end_time, "POSIXct")
  expect_equal(attr(comps$end_time, "tzone"), "UTC")
  expect_equal(comps$participants, c(812L, 95L))
  expect_equal(attr(comps, "total"), 3L)
  query <- request_query(last_request(api))
  expect_equal(query$active, "true")
  expect_equal(query$page, "0")
  expect_null(request_headers(last_request(api))[["auth-token"]])
})

test_that("competition filters are sent and enforced", {
  local_zindi()
  api <- local_api(list("GET competitions" = fixture_response("competitions")))

  comps <- zindi_competitions("all", kind = "hackathon", reward = "knowledge", search = "asr")

  query <- request_query(last_request(api))
  expect_equal(query[["kind[]"]], "hackathon")
  expect_equal(query$prize, "knowledge")
  expect_equal(query$query, "asr")
  expect_null(query$active)
  expect_equal(comps$id, "Swahili-ASR-Hackathon")
  expect_error(zindi_competitions(kind = "contest"))
})

test_that("closed status and limit", {
  local_zindi()
  local_api(list("GET competitions" = fixture_response("competitions")))
  expect_equal(zindi_competitions("closed")$id, "Air-Quality-Lagos")
  expect_equal(nrow(zindi_competitions("all", limit = 1)), 1)
})

test_that("competitions are paginated until the total is reached", {
  local_zindi()
  data <- fixture("competitions")$data
  first <- list(data = rep(data[1:2], 25), meta = list(total_count = 51))
  second <- list(data = data[3], meta = list(total_count = 51))
  api <- local_api(list("GET competitions" = list(json_response(first), json_response(second))))

  comps <- zindi_competitions("all", limit = NULL)

  expect_equal(nrow(comps), 51)
  pages <- vapply(api$requests, function(req) request_query(req)$page, character(1))
  expect_equal(pages, c("0", "1"))
})

test_that("an empty listing is a zero-row data frame", {
  local_zindi()
  local_api(list("GET competitions" = json_response(list(data = list(), meta = list(total_count = 0)))))
  comps <- zindi_competitions()
  expect_equal(nrow(comps), 0)
  expect_s3_class(comps$start_time, "POSIXct")
})

test_that("competition details", {
  local_zindi()
  local_api(list("GET competitions/Crop-Yield-Forecast-Challenge" = fixture_response("competition")))

  comp <- zindi_competition(competition_id)

  expect_s3_class(comp, "zindi_competition")
  expect_equal(comp$title, "Crop Yield Forecast Challenge")
  expect_equal(comp$metric, "RMSE")
  expect_equal(comp$max_team_size, 4L)
  expect_equal(comp$daily_submission_limit, 10L)
  expect_equal(comp$data_files$filename[1:2], c("Train.csv", "Test.csv"))
  expect_equal(comp$data_files$size[[1]], 1048576)
  expect_equal(comp$data_files$zip_of_everything, c(FALSE, FALSE, FALSE, TRUE))
  expect_match(comp$description, "maize")
  expect_match(comp$rules, "10 submissions per day")
  expect_equal(comp$url, "https://zindi.world/competitions/Crop-Yield-Forecast-Challenge")
  expect_equal(format(comp$end_time, "%Y"), "2026")
  expect_output(print(comp), "Train.csv +1.0 MB")
})

test_that("a missing competition is not found", {
  local_zindi()
  local_api(list("GET competitions/nope" =
    json_response(list(data = list(errors = list(message = "not found"))), 404)))
  expect_error(zindi_competition("nope"), class = "zindi_not_found")
})

test_that("the competition id is resolved in order", {
  local_zindi()
  expect_error(zindi_competition_id(), class = "zindi_competition_not_set")
  writeLines('competition = "From-File"', "zindi.toml")
  dir.create("sub/dir", recursive = TRUE)
  withr::with_dir("sub/dir", expect_equal(zindi_competition_id(), "From-File"))
  withr::local_envvar(ZINDI_COMPETITION = "From-Env")
  expect_equal(zindi_competition_id(), "From-Env")
  zindi_use("From-Use")
  expect_equal(zindi_competition_id(), "From-Use")
  expect_equal(zindi_competition_id("Explicit"), "Explicit")
})

test_that("zindi_use(save = TRUE) writes the project file and keeps other entries", {
  local_zindi()
  writeLines(c("# my project", 'competition = "Old"', "other = 1"), "zindi.toml")
  zindi_use(competition_id, save = TRUE)
  lines <- readLines("zindi.toml")
  expect_equal(lines[[1]], paste0('competition = "', competition_id, '"'))
  expect_true("other = 1" %in% lines)
  expect_equal(read_project_competition(), competition_id)
})

test_that("joining sends the token and reports a new participation", {
  local_zindi()
  zindi_auth(token = "tok_test")
  api <- local_api(list(
    "POST competitions/Crop-Yield-Forecast-Challenge/participations" = fixture_response("join_ok")
  ))
  expect_true(zindi_join(competition_id))
  req <- last_request(api)
  expect_equal(req$method, "POST")
  expect_equal(request_headers(req)[["auth-token"]], "tok_test")
  expect_null(req$body)
})

test_that("joining when already in returns FALSE", {
  local_zindi()
  zindi_auth(token = "tok_test")
  local_api(list(
    "POST competitions/Crop-Yield-Forecast-Challenge/participations" = fixture_response("join_already")
  ))
  expect_false(zindi_join(competition_id))
})

test_that("joining a private competition needs its secret code", {
  local_zindi()
  zindi_auth(token = "tok_test")
  api <- local_api(list(
    "POST competitions/Crop-Yield-Forecast-Challenge/participations" = function(req) {
      if (is.null(req$body)) fixture_response("join_secret_required") else fixture_response("join_ok")
    }
  ))
  expect_error(zindi_join(competition_id), class = "zindi_secret_code_required")
  expect_true(zindi_join(competition_id, secret_code = "s3cret"))
  body <- last_request(api)$body
  expect_equal(body$type, "json")
  expect_equal(body$data, list(secret_code = "s3cret"))
})

# -- downloads -----------------------------------------------------------------------------

detail_with <- function(files) {
  detail <- fixture("competition")
  detail$data$datafiles <- files
  json_response(detail)
}

file_route <- function(name) paste0("GET competitions/Crop-Yield-Forecast-Challenge/files/", name)

test_that("downloads skip the all-in-one archive and files already there", {
  local_zindi()
  zindi_auth(token = "tok_test")
  routes <- list(raw_response(as.raw(rep(1, 1048576))), raw_response(as.raw(rep(2, 262144))),
    raw_response(as.raw(rep(3, 2048))), raw_response("zip"))
  names(routes) <- file_route(c("Train.csv", "Test.csv", "SampleSubmission.csv", "all_data.zip"))
  api <- local_api(c(
    list("GET competitions/Crop-Yield-Forecast-Challenge" = fixture_response("competition")),
    routes
  ))

  paths <- zindi_download(competition_id, "data")

  expect_equal(basename(paths), c("Train.csv", "Test.csv", "SampleSubmission.csv"))
  expect_equal(file.size("data/Train.csv"), 1048576)
  expect_equal(calls_to(api, file_route("all_data.zip")), 0)
  expect_length(list.files("data", pattern = "\\.part$"), 0)
  expect_equal(request_headers(last_request(api))[["auth-token"]], "tok_test")

  zindi_download(competition_id, "data")
  expect_equal(calls_to(api, file_route("Train.csv")), 1)
})

test_that("downloads of selected files, with overwrite", {
  local_zindi()
  zindi_auth(token = "tok_test")
  api <- local_api(list(
    "GET competitions/Crop-Yield-Forecast-Challenge" = fixture_response("competition"),
    "GET competitions/Crop-Yield-Forecast-Challenge/files/SampleSubmission.csv" =
      raw_response(as.raw(rep(3, 2048)))
  ))
  zindi_download(competition_id, ".", files = "SampleSubmission.csv")
  zindi_download(competition_id, ".", files = "SampleSubmission.csv", overwrite = TRUE)
  expect_equal(calls_to(api, file_route("SampleSubmission.csv")), 2)
  expect_error(zindi_download(competition_id, ".", files = "nope.csv"), "Unknown file")
})

test_that("the archive is downloaded when it is the only file", {
  local_zindi()
  zindi_auth(token = "tok_test")
  api <- local_api(list(
    "GET competitions/Crop-Yield-Forecast-Challenge" =
      detail_with(list(list(filename = "all.zip", filesize = 3, zip_of_everything = TRUE))),
    "GET competitions/Crop-Yield-Forecast-Challenge/files/all.zip" = raw_response("zip")
  ))
  expect_equal(basename(zindi_download(competition_id, ".")), "all.zip")
})

test_that("redirects are followed without leaking the token", {
  local_zindi()
  zindi_auth(token = "tok_test")
  signed <- "https://storage.example.com/signed/Train.csv"
  api <- local_api(list(
    "GET competitions/Crop-Yield-Forecast-Challenge" =
      detail_with(list(list(filename = "Train.csv", filesize = 4))),
    "GET competitions/Crop-Yield-Forecast-Challenge/files/Train.csv" =
      raw_response("", status = 302, headers = list(Location = signed)),
    "GET https://storage.example.com/signed/Train.csv" = raw_response("data")
  ))
  zindi_download(competition_id, ".")
  expect_equal(readLines("Train.csv", warn = FALSE), "data")
  req <- last_request(api)
  expect_equal(req$url, signed)
  expect_null(request_headers(req)[["auth-token"]])
  expect_null(request_headers(req)[["auth_token"]])
})

test_that("a refused download suggests joining and leaves nothing behind", {
  local_zindi()
  zindi_auth(token = "tok_test")
  local_api(list(
    "GET competitions/Crop-Yield-Forecast-Challenge" =
      detail_with(list(list(filename = "Train.csv", filesize = 4))),
    "GET competitions/Crop-Yield-Forecast-Challenge/files/Train.csv" =
      fixture_response("error_not_authorized", 401)
  ))
  expect_error(zindi_download(competition_id, "."), "zindi_join", class = "zindi_auth_error")
  expect_false(file.exists("Train.csv"))
  expect_false(file.exists("Train.csv.part"))
})

test_that("zip files are extracted with unzip = TRUE", {
  local_zindi()
  skip_if(!nzchar(Sys.which("zip")), "no zip program")
  dir.create("src/images", recursive = TRUE)
  writeLines("hello", "src/images/a.txt")
  withr::with_dir("src", utils::zip("../images.zip", "images", flags = "-rq"))
  payload <- readBin("images.zip", "raw", file.size("images.zip"))
  unlink(c("src", "images.zip"), recursive = TRUE)
  zindi_auth(token = "tok_test")
  local_api(list(
    "GET competitions/Crop-Yield-Forecast-Challenge" =
      detail_with(list(list(filename = "images.zip", filesize = length(payload)))),
    "GET competitions/Crop-Yield-Forecast-Challenge/files/images.zip" = raw_response(payload)
  ))
  zindi_download(competition_id, "data", unzip = TRUE)
  expect_equal(readLines("data/images/a.txt"), "hello")
})

test_that("archive entries outside the destination are refused", {
  expect_equal(unsafe_entries(c("a.txt", "dir/b.csv", "../evil.txt", "/etc/passwd", "c:/x", "a/../../y")),
    c("../evil.txt", "/etc/passwd", "c:/x", "a/../../y"))
})
