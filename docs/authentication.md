# Authentication

Browsing competitions, leaderboards, discussions and profiles needs no account. Joining,
downloading data, submitting and team management do.

## Where credentials come from

Every client (Python, CLI, R, JavaScript, Rust) looks in the same places, in this order:

1. **Code**: `zindi.Client(token=...)` or `Client(username=..., password=...)`.
2. **Environment variables**: `ZINDI_TOKEN`, or `ZINDI_USERNAME` and `ZINDI_PASSWORD`.
3. **Notebook secrets** (Python only): the same names in Colab's *Secrets* panel or Kaggle's
   *Add-ons → Secrets*.
4. **The credentials file** written by `zindi login`.
5. **A prompt**, if you are in a terminal or a notebook.

The first match wins. `zindi whoami` tells you which one was used.

## The credentials file

`zindi login` stores your username and token (never your password) in `credentials.json`:

| OS | Location |
|---|---|
| Windows | `%APPDATA%\zindi\credentials.json` |
| macOS, Linux | `~/.config/zindi/credentials.json` (or `$XDG_CONFIG_HOME/zindi/`) |
| Anywhere | `$ZINDI_CONFIG_DIR/credentials.json` if that variable is set |

On macOS and Linux the file is readable by your user only. `zindi logout` deletes it.

## Colab and Kaggle

Open the key icon (Colab) or *Add-ons → Secrets* (Kaggle), add `ZINDI_USERNAME` and
`ZINDI_PASSWORD` (or `ZINDI_TOKEN`), and allow the notebook to read them. Then:

```python
import zindi
client = zindi.Client()   # picks the secrets up automatically
```

This keeps passwords out of notebooks you share.

## CI and scripts

Set `ZINDI_TOKEN` as a secret in your CI. For a script that must log in non-interactively:

```bash
printf '%s\n' "$ZINDI_PASSWORD" | zindi login -u "$ZINDI_USERNAME" --password-stdin
```

## Using a token from the website

If logging in with a password fails (for example because the site asks for a captcha),
you can reuse the token of your browser session:

1. Log in on [zindi.world](https://zindi.world).
2. Open the browser developer tools → *Application* (or *Storage*) → *Local Storage* →
   `https://zindi.world`, and find the `persist:primary` entry. Its `cookie` value contains
   `auth_token`.
3. Run `zindi login --token <the token>`.

Treat this token like a password. Logging out of the website, or changing your password,
invalidates it.

## Forgot your password?

Reset it at <https://zindi.world/passwords/request>. Login errors show this link too.
