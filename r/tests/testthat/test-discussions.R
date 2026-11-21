test_that("discussions are listed with their author and date", {
  local_zindi()
  api <- local_api(list(
    "GET competitions/Crop-Yield-Forecast-Challenge/discussions" = fixture_response("discussions")
  ))

  threads <- zindi_discussions(competition_id)

  expect_equal(threads$author, c("zawadi_k", "kofi_ml"))
  expect_equal(format(threads$date[[1]], "%Y-%m-%d"), "2026-11-15")
  expect_equal(threads$id[[1]], "7001")
  expect_equal(threads$comments_count, c(4L, 1L))
  expect_equal(
    threads$url[[1]],
    paste0(
      "https://zindi.world/competitions/Crop-Yield-Forecast-Challenge/discussions/7001/",
      "is-external-weather-data-allowed"
    )
  )
  expect_equal(request_query(last_request(api))$per_page, "20")
})
