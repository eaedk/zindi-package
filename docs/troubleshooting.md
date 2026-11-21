# Troubleshooting

Start with `zindi doctor`: it checks the API, your credentials and your paths.

??? question "`zindi: command not found` / `'zindi' is not recognized`"
    The folder where pip puts scripts is not on your `PATH`. Use `python -m zindi ...`, or
    install with `pipx install zindi` / `uv tool install zindi`, which handle the PATH.

??? question "Login fails with the right password"
    Zindi may require a captcha for some logins. Use the token of your browser session:
    see [Using a token from the website](authentication.md#using-a-token-from-the-website).

??? question "Download refused / 401 when downloading"
    You must join a competition before downloading its data: `zindi join <id>`.

??? question "\"No competition selected\""
    Give the id (`zindi leaderboard <id>`), or run `zindi use <id>` in your project folder,
    or set `ZINDI_COMPETITION`.

??? question "My submission stays \"scoring…\""
    Scoring can take minutes on busy competitions. `zindi submissions` shows the latest
    state. `--wait` gives up after 10 minutes by default (`timeout=` in Python).

??? question "SSL / certificate errors at work or school"
    Point `SSL_CERT_FILE` to your organisation's certificate bundle, or pass your own
    `httpx.Client(verify=...)` to `zindi.Client(http_client=...)`.

??? question "Weird characters in the Windows console"
    Use Windows Terminal, or set `PYTHONUTF8=1`.

Still stuck? [Open an issue](https://github.com/eaedk/zindi-package/issues/new/choose) with
the output of `zindi doctor`.
