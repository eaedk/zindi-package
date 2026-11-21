# Security policy

## Supported versions

Only the latest release of each package (Python, JavaScript, R, Rust) receives fixes.

## Reporting a vulnerability

Please **do not open a public issue** for security problems. Use GitHub's
[private vulnerability reporting](https://github.com/eaedk/zindi-package/security/advisories/new)
or email emmanuelkoupoh@gmail.com. You will get an answer within a week.

## How the clients handle your credentials

- Passwords are only sent to `https://api.zindi.world/v1/auth/signin` and are never written
  to disk.
- The auth token is saved in `credentials.json` in your config folder with permissions
  restricted to your user (on Unix-like systems).
- When a download redirects to a storage host, the token is not forwarded.
- Run `zindi logout` to delete the saved token. If you think a token leaked, change your
  Zindi password, which invalidates existing sessions.
