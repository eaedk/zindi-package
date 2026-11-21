// Shared test helpers: the repository's JSON fixtures, an isolated environment per test,
// and a tiny fetch mock that records every request.

import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { Client } from "../dist/index.js";

export const API = "https://api.test/v1";
export const COMP = "Crop-Yield-Forecast-Challenge";

const FIXTURES =
  process.env.ZINDI_FIXTURES ?? fileURLToPath(new URL("../../spec/fixtures/", import.meta.url));

/** Load a JSON fixture shared by every client of this repository. */
export function fixture(name) {
  return JSON.parse(readFileSync(join(FIXTURES, `${name}.json`), "utf8"));
}

/**
 * Give the current test its own config dir and working dir, and no ambient credentials.
 * Returns the paths and a `restore()` function for `afterEach`.
 */
export function isolate() {
  const savedEnv = { ...process.env };
  const savedCwd = process.cwd();
  const root = realpathSync(mkdtempSync(join(tmpdir(), "zindi-js-")));
  const work = join(root, "work");
  mkdirSync(work);
  for (const key of Object.keys(process.env)) {
    if (key.startsWith("ZINDI_") && key !== "ZINDI_FIXTURES") delete process.env[key];
  }
  process.env.ZINDI_CONFIG_DIR = join(root, "config");
  process.env.ZINDI_API_URL = API;
  process.chdir(work);
  return {
    root,
    work,
    restore() {
      process.chdir(savedCwd);
      for (const key of Object.keys(process.env)) {
        if (!(key in savedEnv)) delete process.env[key];
      }
      Object.assign(process.env, savedEnv);
      try {
        rmSync(root, { recursive: true, force: true, maxRetries: 3 });
      } catch {
        // A file still held open (Windows) must not fail the test; the OS cleans tmp later.
      }
    },
  };
}

/** A network error like the one undici throws when the connection is refused. */
export function connectError() {
  const cause = Object.assign(new Error("connect ECONNREFUSED 127.0.0.1:443"), { code: "ECONNREFUSED" });
  return new TypeError("fetch failed", { cause });
}

/** A timeout like the one `AbortSignal.timeout()` produces. */
export function timeoutError() {
  return new DOMException("The operation was aborted due to timeout", "TimeoutError");
}

function toResponse(reply) {
  const { status = 200, json, body, headers = {} } = reply;
  if (json !== undefined) {
    return new Response(JSON.stringify(json), {
      status,
      headers: { "content-type": "application/json", ...headers },
    });
  }
  return new Response(body ?? null, { status, headers });
}

/**
 * A fetch mock. Register routes with `on(method, path, ...replies)`; each reply is a
 * `{status, json, body, headers}` object, an Error to throw, or a function of the call.
 * Replies are used in order and the last one repeats.
 */
export class MockApi {
  constructor() {
    this.routes = [];
    this.calls = [];
    this.fetch = this.fetch.bind(this);
  }

  on(method, path, ...replies) {
    const route = {
      method,
      url: path.startsWith("http") ? path : `${API}${path}`,
      replies,
      calls: [],
      get called() {
        return this.calls.length > 0;
      },
      get callCount() {
        return this.calls.length;
      },
      get lastCall() {
        return this.calls[this.calls.length - 1];
      },
    };
    this.routes.unshift(route);
    return route;
  }

  async fetch(input, init = {}) {
    const url = new URL(input);
    const bare = `${url.origin}${url.pathname}`;
    const method = (init.method ?? "GET").toUpperCase();
    const headers = new Headers(init.headers);
    const body = init.body === undefined || init.body === null ? "" : await new Response(init.body).text();
    const call = { method, url, headers, body, init };
    this.calls.push(call);
    const route = this.routes.find((r) => r.method === method && r.url === bare);
    if (!route) {
      return toResponse({ status: 418, json: { data: { errors: { message: `No mock for ${method} ${bare}` } } } });
    }
    route.calls.push(call);
    const reply = route.replies[Math.min(route.calls.length, route.replies.length) - 1];
    if (typeof reply === "function") return reply(call);
    if (reply instanceof Error || reply instanceof DOMException) throw reply;
    return toResponse(reply);
  }
}

/** Records sleeps instead of waiting. */
export function fakeSleep() {
  const sleeps = [];
  const sleep = async (seconds) => {
    sleeps.push(seconds);
  };
  return { sleeps, sleep };
}

/** A client with a token, wired to the mock. */
export function tokenClient(api, options = {}) {
  return new Client({ token: "tok_test", fetch: api.fetch, sleep: async () => {}, ...options });
}

/** An anonymous client that ignores saved credentials, wired to the mock. */
export function anonClient(api, options = {}) {
  return new Client({ useSavedCredentials: false, fetch: api.fetch, sleep: async () => {}, ...options });
}
