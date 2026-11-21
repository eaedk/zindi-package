# R

```r
install.packages("remotes")
remotes::install_github("eaedk/zindi-package", subdir = "r")
```

```r
library(zindi)

zindi_login()                                   # or set ZINDI_TOKEN
zindi_competitions(status = "open")             # a data.frame
zindi_use("Crop-Yield-Forecast-Challenge")
zindi_join()
zindi_download(destination = "data")
zindi_submit("submission.csv", comment = "first try", wait = TRUE)
head(zindi_leaderboard(), 10)
```

Errors are classed conditions (`zindi_auth_error`, `zindi_not_found`, ...), so you can
handle them with `tryCatch()`. Logging in from R also logs in the CLI and the other
clients: they share the same credentials file.

The function reference is in the
[package README](https://github.com/eaedk/zindi-package/tree/main/r#readme) and in R with
`help(package = "zindi")`.
