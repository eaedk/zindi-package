/**
 * The {@link Client}: everything you can do on Zindi, from JavaScript.
 *
 * @module
 */

import * as fs from "node:fs";
import { mkdir, open, readFile, rename, rm, stat } from "node:fs/promises";
import { basename, extname, join } from "node:path";

import { canSignIn, resolveCredentials } from "./auth.js";
import { deleteCredentials, readProjectCompetition, saveCredentials, writeProjectCompetition } from "./config.js";
import {
  AlreadyJoinedError,
  APIError,
  AuthenticationError,
  CompetitionNotSetError,
  PASSWORD_RESET_URL,
  ZindiError,
} from "./errors.js";
import { type FetchLike, type QueryParams, type RequestOptions, type SleepFn, Transport } from "./http.js";
import {
  type Competition,
  type DataFile,
  type Discussion,
  entryIncludes,
  type LeaderboardEntry,
  parseCompetition,
  parseDiscussion,
  parseLeaderboardEntry,
  parseParticipation,
  parseSubmission,
  parseSubmissionLimits,
  parseTeam,
  parseUser,
  type Participation,
  type Submission,
  type SubmissionLimits,
  type Team,
  type User,
} from "./models.js";
import { isObject, isTruthy, type JsonObject, segment } from "./util.js";

/** Which competitions to list. */
export type CompetitionStatus = "open" | "closed" | "all";

/** Competition kinds. */
export type CompetitionKind = "competition" | "hackathon";

/** Reward types. */
export type RewardType = "prize" | "points" | "knowledge";

/** A competition id; `null` or `undefined` means "the default competition". */
export type CompetitionId = string | null | undefined;

/** Options of {@link Client}. */
export interface ClientOptions {
  /** An auth token. Overrides every other source of credentials. */
  token?: string;
  /** Your Zindi username (used with `password` to sign in). */
  username?: string;
  /** Your Zindi password. Prefer environment variables or a saved login. */
  password?: string;
  /** Default competition id for this client. */
  competition?: string;
  /** API root. Defaults to `$ZINDI_API_URL`, then `https://api.zindi.world/v1`. */
  baseUrl?: string;
  /** Network timeout in seconds. Defaults to 30. */
  timeout?: number;
  /** Retries for transient failures (network errors, HTTP 429/5xx on reads). Defaults to 3. */
  maxRetries?: number;
  /** Read the credentials file written by `login()` or `zindi login`. Defaults to `true`. */
  useSavedCredentials?: boolean;
  /** A custom `fetch` implementation (proxies, tests, instrumentation). */
  fetch?: FetchLike;
  /** A custom sleep function, taking seconds. Used between retries and polls. */
  sleep?: SleepFn;
}

/** Options of {@link Client.login}. */
export interface LoginOptions {
  /** Save the auth token (never the password) in the shared credentials file. Defaults to `true`. */
  save?: boolean;
}

/** Options of {@link Client.logout}. */
export interface LogoutOptions {
  /** Also delete the saved credentials file. Defaults to `true`. */
  forget?: boolean;
}

/** Options of {@link Client.use}. */
export interface UseOptions {
  /** Also write the competition to `zindi.toml` in the current folder. Defaults to `false`. */
  save?: boolean;
}

/** Options of {@link Client.competitions}. */
export interface CompetitionsOptions {
  /** `"open"` (default), `"closed"` or `"all"`. */
  status?: CompetitionStatus;
  /** `"competition"` or `"hackathon"`; both when left out. */
  kind?: CompetitionKind;
  /** `"prize"`, `"points"` or `"knowledge"`; any when left out. */
  reward?: RewardType;
  /** Free-text search on titles. */
  search?: string;
  /** Maximum number of results (default 50); `null` for all of them. */
  limit?: number | null;
}

/** Options of {@link Client.join}. */
export interface JoinOptions {
  /** The secret code of a private competition. */
  secretCode?: string;
}

/** Options of {@link Client.download}. */
export interface DownloadOptions {
  /**
   * Only these file names. By default, every file except the "everything in one zip" archive,
   * unless that archive is the only file.
   */
  files?: string[];
  /** Download again even if the file is already there with the expected size. */
  overwrite?: boolean;
}

/** Options shared by the methods that wait for a score. */
export interface WaitOptions {
  /** Give up waiting after this many seconds. Defaults to 600. */
  timeout?: number;
  /** Seconds between two checks. Defaults to 10. */
  pollInterval?: number;
}

/** Options of {@link Client.submit}. */
export interface SubmitOptions extends WaitOptions {
  /** A note to remember what this submission was. */
  comment?: string;
  /** The competition to submit to (default: the client's competition). */
  competition?: CompetitionId;
  /** Wait until Zindi has scored the submission and return it with its score. */
  wait?: boolean;
}

/** Options of {@link Client.submissions}. */
export interface SubmissionsOptions {
  /** Maximum number of submissions to return. */
  limit?: number | null;
}

/** Options of {@link Client.leaderboard}. */
export interface LeaderboardOptions {
  /** Maximum number of rows to return. */
  limit?: number | null;
  /** Keep the rows without a valid score (they come last). Defaults to `false`. */
  includeUnranked?: boolean;
}

/** Options naming a competition. */
export interface CompetitionOptions {
  /** The competition (default: the client's competition). */
  competition?: CompetitionId;
}

/** Options of {@link Client.createTeam}. */
export interface CreateTeamOptions extends CompetitionOptions {
  /** Usernames to invite right after creating the team. */
  invite?: string[];
}

/** Options of {@link Client.discussions}. */
export interface DiscussionsOptions {
  /** Maximum number of threads (default 20); `null` for all of them. */
  limit?: number | null;
}

interface CallOptions extends RequestOptions {
  authRequired?: boolean;
}

interface PaginateOptions {
  params?: QueryParams;
  perPage?: number;
  limit?: number | null;
  authRequired?: boolean;
}

const MIME_TYPES: Record<string, string> = {
  ".csv": "text/csv",
  ".tsv": "text/tab-separated-values",
  ".txt": "text/plain",
  ".json": "application/json",
  ".zip": "application/zip",
  ".gz": "application/gzip",
  ".parquet": "application/vnd.apache.parquet",
};

type OpenAsBlob = (path: string, options?: { type?: string }) => Promise<Blob>;

async function fileBlob(path: string): Promise<Blob> {
  const type = MIME_TYPES[extname(path).toLowerCase()] ?? "application/octet-stream";
  // fs.openAsBlob (Node >= 19.8) streams the file instead of loading it in memory.
  const openAsBlob = (fs as { openAsBlob?: OpenAsBlob }).openAsBlob;
  if (typeof openAsBlob === "function") return openAsBlob(path, { type });
  return new Blob([await readFile(path)], { type });
}

async function isFile(path: string): Promise<boolean> {
  try {
    return (await stat(path)).isFile();
  } catch {
    return false;
  }
}

async function fileSize(path: string): Promise<number | null> {
  try {
    const info = await stat(path);
    return info.isFile() ? info.size : null;
  } catch {
    return null;
  }
}

async function writeBody(response: Response, path: string): Promise<void> {
  const handle = await open(path, "w");
  try {
    if (response.body) {
      for await (const chunk of response.body as unknown as AsyncIterable<Uint8Array>) {
        await handle.write(chunk);
      }
    }
  } finally {
    await handle.close();
  }
}

function chooseFiles(available: DataFile[], wanted: string[] | undefined): DataFile[] {
  if (wanted !== undefined) {
    const byName = new Map(available.map((file) => [file.filename, file]));
    const missing = wanted.filter((name) => !byName.has(name));
    if (missing.length > 0) {
      const known = [...byName.keys()].join(", ") || "none";
      throw new ZindiError(`Unknown file(s): ${missing.join(", ")}. Available: ${known}`);
    }
    return wanted.map((name) => byName.get(name) as DataFile);
  }
  const individual = available.filter((file) => !file.isArchiveOfEverything);
  return individual.length > 0 ? individual : [...available];
}

/**
 * A connection to Zindi.
 *
 * Most methods take an optional competition id. When you leave it out (or pass `null`), the
 * client uses, in order: the competition set with {@link Client.use}, the `ZINDI_COMPETITION`
 * environment variable, then the `zindi.toml` file of your project.
 *
 * You do not need to log in to browse competitions and leaderboards. For everything else the
 * client finds your credentials by itself: the `token` option, then `ZINDI_TOKEN`, then
 * `ZINDI_USERNAME` and `ZINDI_PASSWORD` (signing in lazily), then the shared credentials file.
 *
 * @example
 * ```ts
 * import { Client } from "zindi";
 *
 * const zindi = new Client();
 * const open = await zindi.competitions({ status: "open" });
 * await zindi.join("Crop-Yield-Forecast-Challenge");
 * await zindi.download("Crop-Yield-Forecast-Challenge", "data");
 * ```
 */
export class Client {
  /** The default competition of this client, set with {@link Client.use}. */
  competitionId: string | null;

  private readonly http: Transport;
  private readonly sleep: SleepFn;
  private readonly useSaved: boolean;
  private _username: string | null;
  private _password: string | null;
  private _credentialsSource: string | null;
  private resolving: Promise<void> | null;

  /** @param options - See {@link ClientOptions}. */
  constructor(options: ClientOptions = {}) {
    const sleep: SleepFn =
      options.sleep ??
      ((seconds) => new Promise((resolve) => setTimeout(resolve, Math.max(0, seconds) * 1000)));
    this.http = new Transport({
      baseUrl: options.baseUrl,
      token: options.token ?? null,
      timeout: options.timeout,
      maxRetries: options.maxRetries,
      fetch: options.fetch,
      sleep,
    });
    this.sleep = sleep;
    this.useSaved = options.useSavedCredentials ?? true;
    this._username = options.username ?? null;
    this._password = options.password ?? null;
    this._credentialsSource = options.token ? "argument" : null;
    this.resolving = options.token ? Promise.resolve() : null;
    this.competitionId = options.competition ?? null;
  }

  // -- authentication --------------------------------------------------------------------------

  /** The current auth token, if any (does not trigger a login). */
  get token(): string | null {
    return this.http.token;
  }

  /** The username of the logged-in user, if known. */
  get username(): string | null {
    return this._username;
  }

  /** Where the credentials came from, e.g. `"environment variables"`. */
  get credentialsSource(): string | null {
    return this._credentialsSource;
  }

  /** The API root this client talks to. */
  get baseUrl(): string {
    return this.http.baseUrl;
  }

  /** `true` if credentials are available (a token, or a username and password). */
  async isAuthenticated(): Promise<boolean> {
    await this.resolveCredentials();
    return Boolean(this.http.token || (this._username && this._password));
  }

  /**
   * Sign in with a username and password, and return your profile.
   *
   * Missing values are taken from the client options, then from `ZINDI_USERNAME` and
   * `ZINDI_PASSWORD`. With `save` (the default) the auth token, never the password, is stored
   * in the shared credentials file, so later sessions and the other zindi clients are logged
   * in too.
   *
   * @throws {@link AuthenticationError} when the username or password is missing or wrong.
   */
  async login(username?: string, password?: string, options: LoginOptions = {}): Promise<User> {
    let user = username || this._username || null;
    let pass = password || this._password || null;
    if (!(user && pass)) {
      const found = await resolveCredentials({ useSaved: false });
      if (found && canSignIn(found)) {
        user = user || found.username;
        pass = pass || found.password;
      }
    }
    if (!user) throw new AuthenticationError("A username is required to log in.");
    if (!pass) throw new AuthenticationError("A password is required to log in.");

    const data = await this.signIn(user, pass);
    if (options.save ?? true) {
      await saveCredentials(this._username ?? user, this.http.token ?? "");
    }
    const profile = isObject(data.user) && isTruthy(data.user) ? data.user : { username: this._username };
    return parseUser(profile);
  }

  private async signIn(username: string, password: string): Promise<JsonObject> {
    let data: unknown;
    try {
      data = await this.http.request("POST", "auth/signin", {
        auth: false,
        form: { username, password },
      });
    } catch (error) {
      if (error instanceof APIError) {
        const fields = isObject(error.errors) ? error.errors : {};
        if ("password" in fields || "username" in fields || error.message.toLowerCase().includes("login")) {
          throw new AuthenticationError(
            `Login failed: ${error.message}. Check your username and password. ` +
              `Forgot your password? Reset it at ${PASSWORD_RESET_URL}`,
            { statusCode: error.statusCode, errors: error.errors },
          );
        }
      }
      throw error;
    }
    const payload = isObject(data) ? data : {};
    const token = payload.auth_token;
    if (!token) throw new AuthenticationError("Zindi did not return an auth token.");
    this.http.token = String(token);
    const user = isObject(payload.user) ? payload.user : {};
    this._username = String(user.username || username);
    this._password = password;
    this.resolving = Promise.resolve();
    this._credentialsSource = this._credentialsSource ?? "password";
    return payload;
  }

  /** Forget the token in memory and, with `forget` (the default), the saved credentials file. */
  async logout(options: LogoutOptions = {}): Promise<void> {
    this.http.token = null;
    this._password = null;
    this.resolving = Promise.resolve();
    if (options.forget ?? true) await deleteCredentials();
  }

  private resolveCredentials(): Promise<void> {
    this.resolving ??= this.findCredentials();
    return this.resolving;
  }

  private async findCredentials(): Promise<void> {
    if (this._username && this._password) {
      this._credentialsSource = "argument";
      return;
    }
    const found = await resolveCredentials({ useSaved: this.useSaved });
    if (found === null) return;
    this._credentialsSource = found.source;
    this._username = this._username ?? found.username;
    if (found.token) this.http.token = found.token;
    if (canSignIn(found)) this._password = found.password;
  }

  private async ensureToken(): Promise<void> {
    await this.resolveCredentials();
    if (this.http.token) return;
    if (this._username && this._password) {
      await this.signIn(this._username, this._password);
      return;
    }
    throw new AuthenticationError(
      "You are not logged in. Call `client.login()`, run `zindi login`, " +
        "or set ZINDI_TOKEN (or ZINDI_USERNAME and ZINDI_PASSWORD).",
    );
  }

  /** Make an API call, signing in first if needed and once more if the token expired. */
  private async call(method: string, path: string, options: CallOptions = {}): Promise<unknown> {
    const { authRequired = true, ...request } = options;
    if (authRequired) await this.ensureToken();
    else await this.resolveCredentials();
    try {
      return await this.http.request(method, path, request);
    } catch (error) {
      if (!(error instanceof AuthenticationError) || !(authRequired && this._username && this._password)) {
        throw error;
      }
      await this.signIn(this._username, this._password);
      return this.http.request(method, path, request);
    }
  }

  // -- competition selection -------------------------------------------------------------------

  /**
   * Make `competition` the default for this client.
   *
   * With `save` it is also written to `zindi.toml` in the current folder, which future clients
   * (and the CLI) started from this project pick up.
   */
  async use(competition: string, options: UseOptions = {}): Promise<string> {
    this.competitionId = competition;
    if (options.save) await writeProjectCompetition(competition);
    return competition;
  }

  /**
   * The competition id a method would use for `competition`.
   *
   * @throws {@link CompetitionNotSetError} when no competition is given or configured.
   */
  async resolveCompetition(competition?: CompetitionId): Promise<string> {
    const id =
      competition ||
      this.competitionId ||
      process.env.ZINDI_COMPETITION ||
      (await readProjectCompetition());
    if (!id) throw new CompetitionNotSetError();
    return id;
  }

  // -- users -----------------------------------------------------------------------------------

  /** Your own profile. */
  async me(): Promise<User> {
    // `/me` answers `{ auth_token, user: {...} }`; older versions sent the user itself.
    const data = asObject(await this.call("GET", "me"));
    const user = data["user"];
    return parseUser(user !== null && typeof user === "object" && !Array.isArray(user) ? asObject(user) : data);
  }

  /** The public profile of any user. */
  async user(username: string): Promise<User> {
    return parseUser(asObject(await this.call("GET", `users/${segment(username)}`, { authRequired: false })));
  }

  // -- competitions ----------------------------------------------------------------------------

  /** List competitions. No login needed. */
  async competitions(options: CompetitionsOptions = {}): Promise<Competition[]> {
    const { status = "open", kind, reward, search } = options;
    const limit = options.limit === undefined ? 50 : options.limit;
    const params: QueryParams = { "kind[]": kind, prize: reward, query: search };
    if (status === "open") params.active = "true";
    const items: Competition[] = [];
    for await (const raw of this.paginate("competitions", { params, perPage: 50, authRequired: false })) {
      const comp = parseCompetition(raw);
      if (status === "open" && !comp.isOpen) continue;
      if (status === "closed" && comp.isOpen) continue;
      if (reward && comp.rewardType && comp.rewardType !== reward) continue;
      if (kind && comp.kind && comp.kind !== kind) continue;
      items.push(comp);
      if (limit !== null && items.length >= limit) break;
    }
    return items;
  }

  /** Full details of a competition: data files, rules, metric, team size... No login needed. */
  async competition(competition?: CompetitionId): Promise<Competition> {
    const cid = await this.resolveCompetition(competition);
    return parseCompetition(asObject(await this.call("GET", `competitions/${segment(cid)}`, { authRequired: false })));
  }

  /**
   * Join a competition (accepting its rules).
   *
   * @returns `true` if you just joined, `false` if you were already in.
   * @throws {@link SecretCodeRequiredError} when the competition is private and no code was given.
   */
  async join(competition?: CompetitionId, options: JoinOptions = {}): Promise<boolean> {
    const cid = await this.resolveCompetition(competition);
    const json = options.secretCode ? { secret_code: options.secretCode } : undefined;
    try {
      await this.call("POST", `competitions/${segment(cid)}/participations`, { json });
    } catch (error) {
      if (error instanceof AlreadyJoinedError) return false;
      throw error;
    }
    return true;
  }

  // -- data ------------------------------------------------------------------------------------

  /**
   * Download a competition's data files into `destination` (default: the current folder).
   *
   * Files that are already there with the expected size are skipped, so this is cheap to call
   * at the top of every script. Downloads go to a temporary `.part` file first, so an
   * interrupted download never leaves a truncated file behind.
   *
   * @returns The paths of the files on disk.
   * @throws {@link AuthenticationError} when Zindi refuses the download (join first).
   */
  async download(
    competition?: CompetitionId,
    destination = ".",
    options: DownloadOptions = {},
  ): Promise<string[]> {
    const cid = await this.resolveCompetition(competition);
    const comp = await this.competition(cid);
    const chosen = chooseFiles(comp.dataFiles, options.files);
    for (const file of chosen) {
      if (!file.filename || basename(file.filename) !== file.filename) {
        throw new ZindiError(`Refusing to download ${JSON.stringify(file.filename)}: unsafe file name.`);
      }
    }
    await mkdir(destination, { recursive: true });
    await this.ensureToken();

    const paths: string[] = [];
    for (const file of chosen) {
      const path = join(destination, file.filename);
      if (!options.overwrite) {
        const size = await fileSize(path);
        if (size !== null && (file.size === null || size === file.size)) {
          paths.push(path);
          continue;
        }
      }
      await this.downloadFile(cid, file, path);
      paths.push(path);
    }
    return paths;
  }

  private async downloadFile(cid: string, file: DataFile, path: string): Promise<void> {
    const part = `${path}.part`;
    try {
      const response = await this.http.download(`competitions/${segment(cid)}/files/${segment(file.filename)}`);
      await writeBody(response, part);
      await rename(part, path);
    } catch (error) {
      await rm(part, { force: true });
      if (error instanceof AuthenticationError) {
        throw new AuthenticationError(
          `Zindi refused the download of ${file.filename}: ${error.message}. ` +
            `Have you joined the competition? Try \`zindi join ${cid}\`.`,
          { statusCode: error.statusCode, errors: error.errors },
        );
      }
      throw error;
    }
  }

  // -- submissions -----------------------------------------------------------------------------

  /**
   * Upload a submission file.
   *
   * With `wait`, the promise resolves once Zindi has scored the submission.
   */
  async submit(path: string, options: SubmitOptions = {}): Promise<Submission> {
    const cid = await this.resolveCompetition(options.competition);
    if (!(await isFile(path))) throw new ZindiError(`Submission file not found: ${path}`);
    await this.ensureToken();
    const form = new FormData();
    form.append("file", await fileBlob(path), basename(path));
    form.append("comment", options.comment ?? "");
    const data = await this.call("POST", `competitions/${segment(cid)}/submissions`, { multipart: form });
    const submission = parseSubmission(asObject(data));
    if (options.wait && submission.id) {
      return this.waitForScore(cid, submission.id, {
        timeout: options.timeout,
        pollInterval: options.pollInterval,
      });
    }
    return submission;
  }

  /** Your submissions to a competition, newest first. */
  async submissions(competition?: CompetitionId, options: SubmissionsOptions = {}): Promise<Submission[]> {
    const cid = await this.resolveCompetition(competition);
    const { limit } = options;
    const data = await this.call("GET", `competitions/${segment(cid)}/submissions`, {
      params: { per_page: limit || 1000 },
    });
    const items = (Array.isArray(data) ? data : []).filter(isObject).map(parseSubmission);
    return limit === null || limit === undefined ? items : items.slice(0, limit);
  }

  /**
   * Poll until a submission is scored (or failed) and return it.
   *
   * @throws {@link ZindiError} when it is still not scored after `timeout` seconds.
   */
  async waitForScore(competition: CompetitionId, submissionId: string, options: WaitOptions = {}): Promise<Submission> {
    const cid = await this.resolveCompetition(competition);
    const timeout = options.timeout ?? 600;
    const pollInterval = options.pollInterval ?? 10;
    const deadline = Date.now() + timeout * 1000;
    for (;;) {
      for (const submission of await this.submissions(cid)) {
        if (submission.id === String(submissionId) && !submission.isPending) return submission;
      }
      if (Date.now() >= deadline) {
        throw new ZindiError(
          `Submission ${submissionId} was not scored within ${Math.round(timeout)}s. ` +
            `Check later with \`zindi submissions ${cid}\`.`,
        );
      }
      await this.sleep(pollInterval);
    }
  }

  /** How many submissions you can still make today. */
  async limits(competition?: CompetitionId): Promise<SubmissionLimits> {
    const cid = await this.resolveCompetition(competition);
    return parseSubmissionLimits(asObject(await this.call("GET", `competitions/${segment(cid)}/submissions/limits`)));
  }

  // -- leaderboard -----------------------------------------------------------------------------

  /** The leaderboard of a competition (private ranks once they are revealed). No login needed. */
  async leaderboard(competition?: CompetitionId, options: LeaderboardOptions = {}): Promise<LeaderboardEntry[]> {
    const cid = await this.resolveCompetition(competition);
    const { limit, includeUnranked = false } = options;
    const rows: LeaderboardEntry[] = [];
    for await (const raw of this.paginate(`competitions/${segment(cid)}/participations`, {
      perPage: 100,
      authRequired: false,
    })) {
      const entry = parseLeaderboardEntry(raw);
      // Unranked rows come last: nobody after this one has a valid score.
      if (entry.rank === null && !includeUnranked) break;
      rows.push(entry);
      if (limit !== null && limit !== undefined && rows.length >= limit) break;
    }
    return rows;
  }

  /** Your rank, best score and submission count in a competition. */
  async myParticipation(competition?: CompetitionId): Promise<Participation> {
    const cid = await this.resolveCompetition(competition);
    const data = await this.call("GET", `competitions/${segment(cid)}/participations/my_participation`);
    return parseParticipation(asObject(data));
  }

  /** Your rank on the leaderboard, or `null` if you have no valid score yet. */
  async rank(competition?: CompetitionId): Promise<number | null> {
    const cid = await this.resolveCompetition(competition);
    let rank: number | null = null;
    try {
      rank = (await this.myParticipation(cid)).rank;
    } catch (error) {
      if (!(error instanceof APIError)) throw error;
    }
    if (rank !== null) return rank;
    const username = this._username;
    if (!username) return null;
    for (const entry of await this.leaderboard(cid)) {
      if (entryIncludes(entry, username)) return entry.rank;
    }
    return null;
  }

  // -- teams -----------------------------------------------------------------------------------

  /** Your team in a competition, or `null` if you compete alone. */
  async team(competition?: CompetitionId): Promise<Team | null> {
    const cid = await this.resolveCompetition(competition);
    let data: unknown;
    try {
      data = await this.call("GET", `competitions/${segment(cid)}/my_team`);
    } catch (error) {
      if (error instanceof APIError && (error.statusCode === 404 || error.message.toLowerCase().includes("not found"))) {
        return null;
      }
      throw error;
    }
    return isObject(data) && isTruthy(data) ? parseTeam(data) : null;
  }

  /** Create a team (you become its leader) and optionally invite people. */
  async createTeam(name: string, options: CreateTeamOptions = {}): Promise<Team> {
    const cid = await this.resolveCompetition(options.competition);
    const data = await this.call("POST", `competitions/${segment(cid)}/my_team`, { form: { title: name } });
    const team = parseTeam(isObject(data) ? data : { title: name });
    if (options.invite && options.invite.length > 0) {
      await this.invite(options.invite, { competition: cid });
    }
    return team;
  }

  /**
   * Invite one or more users to your team.
   *
   * People who already have a pending invitation count as invited.
   *
   * @returns The usernames that were invited.
   */
  async invite(usernames: string | string[], options: CompetitionOptions = {}): Promise<string[]> {
    const cid = await this.resolveCompetition(options.competition);
    const names = typeof usernames === "string" ? [usernames] : [...usernames];
    const invited: string[] = [];
    for (const name of names) {
      try {
        await this.call("POST", `competitions/${segment(cid)}/my_team/invite`, { form: { username: name } });
      } catch (error) {
        if (!(error instanceof APIError) || !error.message.toLowerCase().includes("already invited")) {
          throw error;
        }
      }
      invited.push(name);
    }
    return invited;
  }

  /** Disband your team (you must be its leader). */
  async disbandTeam(competition?: CompetitionId): Promise<void> {
    const cid = await this.resolveCompetition(competition);
    await this.call("DELETE", `competitions/${segment(cid)}/my_team`);
  }

  // -- discussions -----------------------------------------------------------------------------

  /** Discussion threads of a competition, newest first. No login needed. */
  async discussions(competition?: CompetitionId, options: DiscussionsOptions = {}): Promise<Discussion[]> {
    const cid = await this.resolveCompetition(competition);
    const limit = options.limit === undefined ? 20 : options.limit;
    const items: Discussion[] = [];
    for await (const raw of this.paginate(`competitions/${segment(cid)}/discussions`, {
      perPage: 20,
      limit,
      authRequired: false,
    })) {
      items.push(parseDiscussion(raw, cid));
    }
    return items;
  }

  // -- internals -------------------------------------------------------------------------------

  private async *paginate(path: string, options: PaginateOptions = {}): AsyncGenerator<JsonObject> {
    const { params = {}, perPage = 100, limit = null, authRequired = true } = options;
    if (authRequired) await this.ensureToken();
    else await this.resolveCredentials();
    let seen = 0;
    for (let page = 0; ; page++) {
      const result = await this.http.page(path, { ...params, page, per_page: perPage });
      for (const item of result.items) {
        yield item;
        seen += 1;
        if (limit !== null && seen >= limit) return;
      }
      if (result.items.length < perPage || (result.total !== null && seen >= result.total)) return;
    }
  }
}

function asObject(data: unknown): JsonObject {
  return isObject(data) ? data : {};
}
