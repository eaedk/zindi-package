# Installation

## Python library and CLI

=== "pip"

    ```bash
    pip install zindi
    ```

=== "uv (project)"

    ```bash
    uv add zindi
    ```

=== "uv (CLI only)"

    ```bash
    uv tool install zindi      # or run without installing: uvx zindi competitions
    ```

=== "pipx"

    ```bash
    pipx install zindi
    ```

=== "Notebook"

    ```python
    %pip install -q zindi
    ```

Optional extras:

| Extra | Adds | Install |
|---|---|---|
| `pandas` | `.to_pandas()` on every result list | `pip install "zindi[pandas]"` |
| `notebook` | the `zindi.explore()` widget (already on Colab) | `pip install "zindi[notebook]"` |
| `all` | both | `pip install "zindi[all]"` |

Check the installation:

```bash
zindi --version
zindi doctor
```

### Windows notes

- If `zindi` is "not recognized", Python's `Scripts` folder is not on your `PATH`. Use
  `python -m zindi ...` instead, or install with `pipx`/`uv tool`, which manage the PATH for you.
- Works in PowerShell, `cmd.exe` and Windows Terminal. Colors and emoji render best in
  Windows Terminal.

### Behind a proxy

`zindi` uses [httpx](https://www.python-httpx.org/), which honours the standard
`HTTPS_PROXY` and `SSL_CERT_FILE` environment variables.

## Other languages

| Language | Install |
|---|---|
| JavaScript / TypeScript | `npm install zindi` |
| R | `remotes::install_github("eaedk/zindi-package", subdir = "r")` |
| Rust | `cargo add zindi` |
