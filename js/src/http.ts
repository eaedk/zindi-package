/**
 * Low-level HTTP layer: envelopes, errors, retries and auth headers.
 *
 * Most people never touch this module directly: `Client` wraps it.
 *
 * @module
 */

import {
  AlreadyJoinedError,
  APIError,
  type APIErrorOptions,
  AuthenticationError,
  NotFoundError,
  RateLimitError,
  SecretCodeRequiredError,
} from "./errors.js";
import { isObject, isTruthy, type JsonObject } from "./util.js";
import { VERSION } from "./version.js";

/** The default API root. Override it with the `baseUrl` option or `ZINDI_API_URL`. */
export const DEFAULT_BASE_URL = "https://api.zindi.world/v1";

/** The Zindi website. */
export const WEB_URL = "https://zindi.world";

/** The `User-Agent` header sent with every request. */
export const USER_AGENT = `zindi-js/${VERSION} (+https://github.com/eaedk/zindi-package)`;

const RETRY_STATUSES = new Set([429, 500, 502, 503, 504]);
const IDEMPOTENT = new Set(["GET", "HEAD", "OPTIONS", "DELETE"]);
const REDIRECT_STATUSES = new Set([301, 302, 303, 307, 308]);
const MAX_BACKOFF = 30;
const MAX_REDIRECTS = 5;

// Error codes meaning the request never reached the server, so even a POST is safe to replay.
const CONNECT_ERROR_CODES = new Set([
  "ECONNREFUSED",
  "ENOTFOUND",
  "EAI_AGAIN",
  "EHOSTUNREACH",
  "ENETUNREACH",
  "UND_ERR_CONNECT_TIMEOUT",
]);

/**
 * The subset of the `fetch` API the client needs. Pass your own (for proxies, tests or
 * instrumentation) with the `fetch` option of `Client`.
 */
export type FetchLike = (input: string, init: RequestInit) => Promise<Response>;

/** Wait for the given number of **seconds**. Injectable so that tests run instantly. */
export type SleepFn = (seconds: number) => Promise<void>;

/** Query string values. `null` and `undefined` are left out. */
export type QueryParams = Record<string, string | number | boolean | null | undefined>;

/** Options for one API request. */
export interface RequestOptions {
  /** Send the auth token, when one is known. Defaults to `true`. */
  auth?: boolean;
  /** Query string parameters. */
  params?: QueryParams;
  /** A JSON body. `null` and `undefined` send no body. */
  json?: unknown;
  /** A form-encoded body (`application/x-www-form-urlencoded`). */
  form?: Record<string, string>;
  /** A multipart body. */
  multipart?: FormData;
}

/** One page of a list endpoint. */
export interface Page {
  /** The items of this page. */
  items: JsonObject[];
  /** `meta.total_count`, when the API returns it. */
  total: number | null;
}

/** Options of {@link Transport}. */
export interface TransportOptions {
  /** API root. Defaults to `$ZINDI_API_URL`, then {@link DEFAULT_BASE_URL}. */
  baseUrl?: string;
  /** Auth token to send with requests. */
  token?: string | null;
  /** Network timeout in seconds. Defaults to 30. */
  timeout?: number;
  /** Retries for transient failures. Defaults to 3. */
  maxRetries?: number;
  /** A custom `fetch` implementation. Defaults to the global `fetch`. */
  fetch?: FetchLike;
  /** A custom sleep function. */
  sleep?: SleepFn;
}

/** The API root from `ZINDI_API_URL`, or the default one. */
export function baseUrlFromEnv(): string {
  return (process.env.ZINDI_API_URL || DEFAULT_BASE_URL).replace(/\/+$/, "");
}

/** Turn Zindi's `errors` object (whatever its shape) into one readable sentence. */
export function errorMessage(errors: unknown): string {
  if (errors === null || errors === undefined) return "Unknown error";
  if (typeof errors === "string") return errors;
  if (Array.isArray(errors)) return errors.map(errorMessage).join("; ");
  if (isObject(errors)) {
    for (const key of ["message", "base", "error"]) {
      if (key in errors) return errorMessage(errors[key]);
    }
    return Object.entries(errors)
      .map(([key, value]) => `${key}: ${errorMessage(value)}`)
      .join("; ");
  }
  return String(errors);
}

/** Throw the most specific error for an error response. */
export function raiseForErrors(statusCode: number, errors: unknown): never {
  const message = errorMessage(errors);
  const lowered = message.toLowerCase();
  const options: APIErrorOptions = { statusCode, errors };
  if (lowered.includes("already in") || lowered.includes("already joined")) {
    throw new AlreadyJoinedError(message, options);
  }
  if (lowered.includes("secret code")) {
    throw new SecretCodeRequiredError(message, options);
  }
  if (
    statusCode === 401 ||
    statusCode === 403 ||
    lowered.includes("not authorized") ||
    lowered.includes("not authenticated")
  ) {
    throw new AuthenticationError(message, options);
  }
  if (statusCode === 404 || lowered === "not found") {
    throw new NotFoundError(message, options);
  }
  if (statusCode === 429) {
    throw new RateLimitError(message, options);
  }
  throw new APIError(message, options);
}

function errorCode(error: unknown): string | undefined {
  let current: unknown = error;
  for (let depth = 0; depth < 4 && current; depth++) {
    if (isObjectLike(current) && typeof current.code === "string") return current.code;
    current = isObjectLike(current) ? current.cause : undefined;
  }
  return undefined;
}

function isObjectLike(value: unknown): value is { code?: unknown; cause?: unknown } {
  return typeof value === "object" && value !== null;
}

function isConnectError(error: unknown): boolean {
  const code = errorCode(error);
  return code !== undefined && CONNECT_ERROR_CODES.has(code);
}

function unreachable(error: unknown): APIError {
  const name = errorCode(error) ?? (error instanceof Error ? error.name : "Error");
  const cause = error instanceof Error && error.cause instanceof Error ? error.cause : error;
  const detail = cause instanceof Error ? cause.message : String(cause);
  return new APIError(`Could not reach Zindi (${name}): ${detail}`, { cause: error });
}

async function discard(response: Response): Promise<void> {
  try {
    await response.body?.cancel();
  } catch {
    // The body was already consumed or closed: nothing to free.
  }
}

const defaultSleep: SleepFn = (seconds) =>
  new Promise((resolve) => setTimeout(resolve, Math.max(0, seconds) * 1000));

/** A thin wrapper around `fetch` that speaks Zindi's conventions. */
export class Transport {
  /** The API root, without a trailing slash. */
  readonly baseUrl: string;
  /** The auth token sent with requests, if any. */
  token: string | null;
  /** How many times transient failures are retried. */
  readonly maxRetries: number;
  /** Network timeout in seconds. */
  readonly timeout: number;
  private readonly fetchFn: FetchLike;
  private readonly sleep: SleepFn;

  /** @param options - See {@link TransportOptions}. */
  constructor(options: TransportOptions = {}) {
    this.baseUrl = (options.baseUrl || baseUrlFromEnv()).replace(/\/+$/, "");
    this.token = options.token ?? null;
    this.maxRetries = options.maxRetries ?? 3;
    this.timeout = options.timeout ?? 30;
    // Look the global fetch up at call time, so it can be replaced after construction.
    this.fetchFn = options.fetch ?? ((input, init) => globalThis.fetch(input, init));
    this.sleep = options.sleep ?? defaultSleep;
  }

  /** The absolute URL of an API path. Absolute URLs are returned unchanged. */
  url(path: string, params?: QueryParams): string {
    const base = /^https?:\/\//.test(path) ? path : `${this.baseUrl}/${path.replace(/^\/+/, "")}`;
    const query = new URLSearchParams();
    for (const [key, value] of Object.entries(params ?? {})) {
      if (value !== null && value !== undefined) query.append(key, String(value));
    }
    const qs = query.toString();
    return qs ? `${base}${base.includes("?") ? "&" : "?"}${qs}` : base;
  }

  /** Headers sent with every request; the auth token is included when `auth` is true. */
  headers(auth = true): Record<string, string> {
    const headers: Record<string, string> = {
      "Current-Url": `${WEB_URL}/`,
      "User-Agent": USER_AGENT,
    };
    if (auth && this.token) {
      // The web app sends `auth-token`; older endpoints read `auth_token`.
      headers["auth-token"] = this.token;
      headers["auth_token"] = this.token;
    }
    return headers;
  }

  /** Seconds to wait before retry number `attempt` (0-based). Honours `Retry-After`. */
  backoff(attempt: number, response?: Response): number {
    const retryAfter = response?.headers.get("retry-after");
    if (retryAfter) {
      const seconds = Number(retryAfter);
      if (Number.isFinite(seconds)) return Math.max(0, Math.min(seconds, MAX_BACKOFF));
    }
    return Math.min(0.5 * 2 ** attempt, MAX_BACKOFF);
  }

  private async send(method: string, url: string, init: RequestInit): Promise<Response> {
    const verb = method.toUpperCase();
    for (let attempt = 0; ; attempt++) {
      let response: Response;
      try {
        response = await this.fetchFn(url, {
          ...init,
          method: verb,
          signal: AbortSignal.timeout(this.timeout * 1000),
        });
      } catch (error) {
        // A POST that may have reached the server is never replayed.
        const retryable = IDEMPOTENT.has(verb) || isConnectError(error);
        if (!retryable || attempt >= this.maxRetries) throw unreachable(error);
        await this.sleep(this.backoff(attempt));
        continue;
      }
      if (RETRY_STATUSES.has(response.status) && IDEMPOTENT.has(verb) && attempt < this.maxRetries) {
        await discard(response);
        await this.sleep(this.backoff(attempt, response));
        continue;
      }
      return response;
    }
  }

  /** Parse a response, unwrap the `{data, meta}` envelope and throw on errors. */
  async unwrap(response: Response): Promise<{ data: unknown; meta: unknown }> {
    const text = await response.text();
    let payload: unknown;
    try {
      payload = JSON.parse(text);
    } catch {
      if (response.ok) {
        const preview = text.trim().replace(/\n/g, " ").slice(0, 200);
        throw new APIError(
          `Zindi returned a non-JSON response (HTTP ${response.status}): ${preview}`,
          { statusCode: response.status },
        );
      }
      raiseForErrors(response.status, response.statusText || "HTTP error");
    }
    let data: unknown = payload;
    let meta: unknown = null;
    if (isObject(payload) && "data" in payload) {
      data = payload.data;
      meta = payload.meta ?? null;
    }
    let errors: unknown = isObject(data) ? data.errors : undefined;
    if ((errors === null || errors === undefined) && isObject(payload) && "errors" in payload) {
      errors = payload.errors;
    }
    const failed = response.status >= 400;
    if (isTruthy(errors) || failed) {
      raiseForErrors(
        response.status,
        isTruthy(errors) ? errors : response.statusText || `HTTP ${response.status}`,
      );
    }
    return { data, meta };
  }

  /** Send a request and return the `data` member of the response envelope. */
  async request(method: string, path: string, options: RequestOptions = {}): Promise<unknown> {
    const headers: Record<string, string> = {
      Accept: "application/json",
      ...this.headers(options.auth ?? true),
    };
    let body: RequestInit["body"];
    if (options.multipart) {
      body = options.multipart;
    } else if (options.form) {
      body = new URLSearchParams(options.form);
      headers["Content-Type"] = "application/x-www-form-urlencoded";
    } else if (options.json !== undefined && options.json !== null) {
      body = JSON.stringify(options.json);
      headers["Content-Type"] = "application/json";
    }
    const init: RequestInit = { headers };
    if (body !== undefined) init.body = body;
    const response = await this.send(method, this.url(path, options.params), init);
    const { data } = await this.unwrap(response);
    return data;
  }

  /** Fetch one page of a list endpoint. */
  async page(path: string, params: QueryParams, auth = true): Promise<Page> {
    const response = await this.send("GET", this.url(path, params), {
      headers: { Accept: "application/json", ...this.headers(auth) },
    });
    const { data, meta } = await this.unwrap(response);
    const items = Array.isArray(data) ? data.filter(isObject) : [];
    const rawTotal = isObject(meta) ? meta.total_count : undefined;
    const total = rawTotal === null || rawTotal === undefined ? null : Number(rawTotal);
    return { items, total: total !== null && Number.isFinite(total) ? total : null };
  }

  /**
   * Start downloading a file and return the successful response, ready to be streamed.
   *
   * Redirects (to object storage) are followed by hand, and the auth token is dropped as soon
   * as the host changes, so it never leaks to a third party.
   */
  async download(path: string): Promise<Response> {
    let url = this.url(path);
    let headers: Record<string, string> = this.headers();
    for (let hop = 0; hop < MAX_REDIRECTS; hop++) {
      const controller = new AbortController();
      // The timeout covers the wait for the response headers, not the transfer itself.
      const timer = setTimeout(() => controller.abort(), this.timeout * 1000);
      let response: Response;
      try {
        response = await this.fetchFn(url, {
          method: "GET",
          headers,
          redirect: "manual",
          signal: controller.signal,
        });
      } catch (error) {
        throw unreachable(error);
      } finally {
        clearTimeout(timer);
      }
      const location = response.headers.get("location");
      if (REDIRECT_STATUSES.has(response.status) && location) {
        const next = new URL(location, url).toString();
        await discard(response);
        if (new URL(next).host !== new URL(url).host) {
          headers = { "User-Agent": USER_AGENT };
        }
        url = next;
        continue;
      }
      if (response.status >= 400) {
        await this.unwrap(response); // always throws for an error status
      }
      return response;
    }
    throw new APIError("Too many redirects while downloading the file.");
  }
}
