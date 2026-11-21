# zindi (JavaScript / TypeScript)

[![npm](https://img.shields.io/npm/v/zindi?label=npm&color=f5a623)](https://www.npmjs.com/package/zindi)
[![CI](https://github.com/eaedk/zindi-package/actions/workflows/js.yml/badge.svg)](https://github.com/eaedk/zindi-package/actions/workflows/js.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](https://github.com/eaedk/zindi-package/blob/main/LICENSE)

Compete on [Zindi](https://zindi.world) from Node.js: browse competitions, join, download
data, submit, wait for your score, follow the leaderboard and manage your team.

- Zero runtime dependencies, ES modules, bundled TypeScript types.
- Node.js 18 or newer on Windows, macOS and Linux (uses the built-in `fetch`).
- Same behaviour and the same login as the [Python, R and Rust clients](https://github.com/eaedk/zindi-package).

**[Documentation](https://eaedk.github.io/zindi-package/)** ·
[Repository](https://github.com/eaedk/zindi-package) ·
[Changelog](https://github.com/eaedk/zindi-package/blob/main/js/CHANGELOG.md)

## Install

```bash
npm install zindi
pnpm add zindi
yarn add zindi
bun add zindi
```

## Quick start

```ts
import { Client } from "zindi";

const zindi = new Client(); // finds your credentials by itself, see below

// Browsing needs no login.
const open = await zindi.competitions({ status: "open", limit: 10 });
const comp = await zindi.competition("Crop-Yield-Forecast-Challenge");
console.log(comp.title, comp.metric, comp.dataFiles.map((f) => f.filename));

// Compete.
await zindi.join(comp.id);
await zindi.download(comp.id, "data");
const sub = await zindi.submit("submission.csv", {
  competition: comp.id,
  comment: "lgbm v1",
  wait: true, // resolves once Zindi has scored it
});
console.log(sub.publicScore, await zindi.rank(comp.id));
```

A runnable tour lives in [`examples/quickstart.mjs`](https://github.com/eaedk/zindi-package/blob/main/js/examples/quickstart.mjs).

### Default competition

Methods that act on a competition take its id as first argument (or as the `competition`
option). Leave it out, or pass `undefined`, and the client uses, in order:

1. the competition set with `client.use(id)` (or the `competition` constructor option),
2. the `ZINDI_COMPETITION` environment variable,
3. `competition = "..."` in the nearest `zindi.toml` (current folder or a parent), which
   `client.use(id, { save: true })` and the Python CLI's `zindi use` write.

```ts
await zindi.use("Crop-Yield-Forecast-Challenge");
await zindi.download(); // same as download("Crop-Yield-Forecast-Challenge", ".")
const board = await zindi.leaderboard(undefined, { limit: 10 });
```

## Authentication

Browsing competitions, leaderboards, profiles and discussions needs no account. Everything
else needs credentials, which the client looks for in this order:

1. the `token` option: `new Client({ token })`,
2. the `ZINDI_TOKEN` environment variable,
3. `ZINDI_USERNAME` and `ZINDI_PASSWORD` (the client signs in on the first call that needs it),
4. the credentials file saved by `login()` or by any other zindi client
   (`zindi login` from the Python CLI, for instance).

```ts
const zindi = new Client();
await zindi.login("amina_dev", process.env.ZINDI_PASSWORD); // saves the token, never the password
```

The credentials file is `credentials.json` in `$ZINDI_CONFIG_DIR`, or `%APPDATA%\zindi`
on Windows, or `$XDG_CONFIG_HOME/zindi`, or `~/.config/zindi`. It is written atomically and
readable by you only. Pass `{ save: false }` to `login()` to keep the token in memory, and
call `logout()` to forget it (and delete the file).

When the client knows your password and Zindi rejects an expired token, it signs in again
and retries the call once.

## Client options

| Option | Default | Description |
|---|---|---|
| `token` | | Auth token; overrides every other credential source. |
| `username`, `password` | | Sign in with these when a token is needed. |
| `competition` | | Default competition id. |
| `baseUrl` | `$ZINDI_API_URL` or `https://api.zindi.world/v1` | API root. |
| `timeout` | `30` | Network timeout, in seconds. |
| `maxRetries` | `3` | Retries for network errors and HTTP 429/5xx on reads. |
| `useSavedCredentials` | `true` | Read the shared credentials file. |
| `fetch` | global `fetch` | Your own fetch (proxy, instrumentation, tests). |
| `sleep` | `setTimeout` | `(seconds) => Promise<void>`, used between retries and polls. |

## Methods

All methods are `async`. `id` is a competition id and is optional wherever the default
competition applies.

| Method | Returns | Login | Description |
|---|---|:---:|---|
| `login(username?, password?, { save = true })` | `User` | | Sign in; save the token for later sessions. |
| `logout({ forget = true })` | `void` | | Forget the token and delete the saved file. |
| `isAuthenticated()` | `boolean` | | Whether credentials are available. |
| `use(id, { save = false })` | `string` | | Set the default competition (and write `zindi.toml`). |
| `resolveCompetition(id?)` | `string` | | The competition id a method would use. |
| `me()` | `User` | ✔ | Your profile. |
| `user(username)` | `User` | | Anyone's public profile. |
| `competitions({ status, kind, reward, search, limit })` | `Competition[]` | | List competitions. `status`: `"open"` (default), `"closed"`, `"all"`; `kind`: `"competition"`, `"hackathon"`; `reward`: `"prize"`, `"points"`, `"knowledge"`; `limit`: 50 by default, `null` for all. |
| `competition(id?)` | `Competition` | | Details: data files, rules, metric, team size. |
| `join(id?, { secretCode })` | `boolean` | ✔ | Join; `false` if you were already in. |
| `download(id?, dest = ".", { files, overwrite })` | `string[]` | ✔ | Download data files; skips files already there with the right size. |
| `submit(path, { comment, competition, wait, timeout = 600, pollInterval = 10 })` | `Submission` | ✔ | Upload a submission, optionally waiting for its score. |
| `submissions(id?, { limit })` | `Submission[]` | ✔ | Your submissions, newest first. |
| `waitForScore(id, submissionId, { timeout, pollInterval })` | `Submission` | ✔ | Poll until a submission is scored or failed. |
| `limits(id?)` | `SubmissionLimits` | ✔ | Submissions left today (`remainingToday`). |
| `leaderboard(id?, { limit, includeUnranked = false })` | `LeaderboardEntry[]` | | The leaderboard, stopping at the first row without a score. |
| `myParticipation(id?)` | `Participation` | ✔ | Your rank, best score and submission count. |
| `rank(id?)` | `number \| null` | ✔ | Your rank, or `null` without a valid score. |
| `team(id?)` | `Team \| null` | ✔ | Your team, or `null` if you compete alone. |
| `createTeam(name, { competition, invite })` | `Team` | ✔ | Create a team (you lead it) and invite people. |
| `invite(usernames, { competition })` | `string[]` | ✔ | Invite one or more users; pending invitations count as invited. |
| `disbandTeam(id?)` | `void` | ✔ | Disband your team. |
| `discussions(id?, { limit = 20 })` | `Discussion[]` | | Discussion threads. |

### Models

Models are plain objects with camelCase fields: `Competition`, `DataFile`, `LeaderboardEntry`,
`Participation`, `Submission`, `SubmissionLimits`, `Team`, `User` and `Discussion`. Dates are
`Date` objects (or `null`), and every model keeps the untouched API payload in `raw`, so fields
this package does not know about yet are never out of reach:

```ts
const comp = await zindi.competition("Crop-Yield-Forecast-Challenge");
comp.endTime;              // Date
comp.dailySubmissionLimit; // 10, read from the rules
comp.raw.difficulty;       // "intermediate"
```

## Downloads

`download()` saves every data file except the "everything in one zip" archive (unless that
archive is the only file), or just the names you pass in `files`. Files that are already
there with the expected size are skipped, so it is cheap to call at the top of every script.
Each file is written to `<name>.part` first and renamed once complete, so an interrupted
download never leaves a truncated file behind. Redirects to object storage are followed
without sending your auth token to the storage host.

## Error handling

Every error thrown on purpose extends `ZindiError`:

| Error | When |
|---|---|
| `AuthenticationError` | Not logged in, wrong password, expired token, or a download refused because you have not joined. |
| `NotFoundError` | Unknown competition, user or file. |
| `RateLimitError` | Zindi kept answering HTTP 429 after the retries. |
| `SecretCodeRequiredError` | Joining a private competition without `secretCode`. |
| `AlreadyJoinedError` | Already a participant (`join()` returns `false` instead of throwing it). |
| `APIError` | Any other API error; base class of the five above. Has `statusCode` and the raw `errors`. |
| `CompetitionNotSetError` | No competition given and no default configured. |
| `ZindiError` | Base class, also used for local problems (missing submission file, unknown file name, score wait timed out). |

```ts
import { AuthenticationError, SecretCodeRequiredError, ZindiError } from "zindi";

try {
  await zindi.join("Private-Challenge");
} catch (error) {
  if (error instanceof SecretCodeRequiredError) {
    await zindi.join("Private-Challenge", { secretCode: process.env.SECRET_CODE });
  } else if (error instanceof AuthenticationError) {
    console.error(error.message); // explains how to log in
  } else if (error instanceof ZindiError) {
    console.error("Zindi said:", error.message);
  } else {
    throw error;
  }
}
```

Network errors and HTTP 429/500/502/503/504 on `GET` and `DELETE` requests are retried up to
`maxRetries` times with exponential backoff (0.5 s, 1 s, 2 s, ..., at most 30 s, honouring
`Retry-After`). A `POST` is replayed only when the connection could not be established, so a
submission is never sent twice.

## Development

```bash
npm install
npm run typecheck
npm test            # builds, then runs node --test against ../spec/fixtures
npm pack --dry-run
```

The tests never touch the network: they inject a fake `fetch` and load the JSON fixtures
shared by every client in [`spec/fixtures`](https://github.com/eaedk/zindi-package/tree/main/spec/fixtures).

## License

MIT. Not affiliated with Zindi.
