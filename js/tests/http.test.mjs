import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, test } from "node:test";

import {
  AlreadyJoinedError,
  APIError,
  AuthenticationError,
  Client,
  CompetitionNotSetError,
  DEFAULT_BASE_URL,
  errorMessage,
  NotFoundError,
  RateLimitError,
  SecretCodeRequiredError,
  USER_AGENT,
  VERSION,
  ZindiError,
} from "../dist/index.js";
import { raiseForErrors } from "../dist/http.js";
import { anonClient, API, connectError, fakeSleep, fixture, isolate, MockApi, timeoutError, tokenClient } from "./helpers.mjs";

let env;
let api;
beforeEach(() => {
  env = isolate();
  api = new MockApi();
});
afterEach(() => env.restore());

describe("retries", () => {
  test("transient errors on reads are retried with exponential backoff", async () => {
    const route = api.on("GET", "/users/amina_dev", { status: 503 }, connectError(), { json: fixture("user_profile") });
    const { sleeps, sleep } = fakeSleep();

    const user = await anonClient(api, { sleep }).user("amina_dev");

    assert.equal(user.username, "amina_dev");
    assert.equal(route.callCount, 3);
    assert.deepEqual(sleeps, [0.5, 1]);
  });

  test("gives up after maxRetries", async () => {
    const route = api.on("GET", "/users/x", { status: 429, json: { data: { errors: { message: "slow down" } } } });
    const { sleeps, sleep } = fakeSleep();

    await assert.rejects(anonClient(api, { sleep }).user("x"), RateLimitError);

    assert.equal(route.callCount, 4);
    assert.deepEqual(sleeps, [0.5, 1, 2]);
  });

  test("Retry-After is honoured and capped at 30 seconds", async () => {
    api.on(
      "GET",
      "/users/amina_dev",
      { status: 503, headers: { "Retry-After": "2" } },
      { status: 503, headers: { "Retry-After": "120" } },
      { json: fixture("user_profile") },
    );
    const { sleeps, sleep } = fakeSleep();
    await anonClient(api, { sleep }).user("amina_dev");
    assert.deepEqual(sleeps, [2, 30]);
  });

  test("DELETE is retried like reads", async () => {
    const route = api.on("DELETE", "/competitions/C/my_team", { status: 502 }, { json: fixture("team_disbanded") });
    await tokenClient(api).disbandTeam("C");
    assert.equal(route.callCount, 2);
  });

  test("a POST is not replayed after a timeout", async () => {
    const route = api.on("POST", "/competitions/C/participations", timeoutError());
    await assert.rejects(tokenClient(api).join("C"), (error) => {
      assert.ok(error instanceof APIError);
      assert.match(error.message, /Could not reach Zindi/);
      return true;
    });
    assert.equal(route.callCount, 1);
  });

  test("a POST is not replayed after a 503", async () => {
    const route = api.on("POST", "/competitions/C/participations", { status: 503 });
    await assert.rejects(tokenClient(api).join("C"), (error) => error instanceof APIError && error.statusCode === 503);
    assert.equal(route.callCount, 1);
  });

  test("a POST is retried when the connection could not be established", async () => {
    const route = api.on("POST", "/competitions/C/participations", connectError(), { json: fixture("join_ok") });
    assert.equal(await tokenClient(api).join("C"), true);
    assert.equal(route.callCount, 2);
  });
});

describe("responses", () => {
  test("a non-JSON success is reported", async () => {
    api.on("GET", "/users/x", { body: "<html>maintenance</html>" });
    await assert.rejects(anonClient(api).user("x"), { name: "APIError", message: /non-JSON/ });
  });

  test("a non-JSON error is mapped by status", async () => {
    api.on("GET", "/users/x", { status: 404, body: "<html>nope</html>" });
    await assert.rejects(anonClient(api).user("x"), NotFoundError);
  });

  test("a top-level errors member is an error", async () => {
    api.on("GET", "/users/x", { json: { errors: { base: "Something broke" } } });
    await assert.rejects(anonClient(api).user("x"), { name: "APIError", message: "Something broke" });
  });

  test("an HTTP error without errors is still an error", async () => {
    api.on("GET", "/users/x", { status: 500, json: { data: {} } });
    await assert.rejects(anonClient(api, { maxRetries: 0 }).user("x"), (error) => {
      assert.ok(error instanceof APIError);
      assert.equal(error.statusCode, 500);
      return true;
    });
  });

  test("requests identify the client", async () => {
    const route = api.on("GET", "/users/amina_dev", { json: fixture("user_profile") });
    await anonClient(api).user("amina_dev");
    const { headers } = route.lastCall;
    assert.equal(headers.get("user-agent"), `zindi-js/${VERSION} (+https://github.com/eaedk/zindi-package)`);
    assert.equal(headers.get("user-agent"), USER_AGENT);
    assert.equal(headers.get("current-url"), "https://zindi.world/");
    assert.equal(headers.get("accept"), "application/json");
  });
});

describe("base URL", () => {
  test("comes from ZINDI_API_URL, then the option, without trailing slash", () => {
    assert.equal(new Client().baseUrl, API);
    assert.equal(new Client({ baseUrl: "https://example.test/v2/" }).baseUrl, "https://example.test/v2");
    delete process.env.ZINDI_API_URL;
    assert.equal(new Client().baseUrl, DEFAULT_BASE_URL);
    assert.equal(DEFAULT_BASE_URL, "https://api.zindi.world/v1");
  });
});

describe("error messages", () => {
  const shapes = [
    [{ message: "already in" }, "already in"],
    [{ base: ["a", "b"] }, "a; b"],
    [{ password: "wrong password" }, "password: wrong password"],
    [{ error: { message: "deep" } }, "deep"],
    [{ a: 1, b: ["x", "y"] }, "a: 1; b: x; y"],
    ["plain", "plain"],
    [null, "Unknown error"],
    [undefined, "Unknown error"],
  ];
  for (const [errors, expected] of shapes) {
    test(`${JSON.stringify(errors) ?? "undefined"} -> ${expected}`, () => {
      assert.equal(errorMessage(errors), expected);
    });
  }

  const mapping = [
    [200, { message: "already in" }, AlreadyJoinedError],
    [200, { message: "You have already joined" }, AlreadyJoinedError],
    [200, { message: "This competition requires a secret code to join." }, SecretCodeRequiredError],
    [401, "whatever", AuthenticationError],
    [403, "whatever", AuthenticationError],
    [200, { message: "not authorized" }, AuthenticationError],
    [200, { message: "Not authenticated" }, AuthenticationError],
    [404, "whatever", NotFoundError],
    [200, { message: "not found" }, NotFoundError],
    [429, "slow down", RateLimitError],
    [500, "boom", APIError],
  ];
  for (const [status, errors, expected] of mapping) {
    test(`HTTP ${status} ${JSON.stringify(errors)} -> ${expected.name}`, () => {
      assert.throws(
        () => raiseForErrors(status, errors),
        (error) => {
          assert.equal(error.constructor, expected);
          assert.ok(error instanceof ZindiError);
          assert.equal(error.statusCode, status);
          assert.deepEqual(error.errors, errors);
          return true;
        },
      );
    });
  }

  test("every error extends ZindiError and keeps its name", () => {
    const error = new CompetitionNotSetError();
    assert.ok(error instanceof ZindiError);
    assert.equal(error.name, "CompetitionNotSetError");
    assert.ok(new RateLimitError("x") instanceof APIError);
  });
});

test("VERSION matches package.json", () => {
  const pkg = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  assert.equal(VERSION, pkg.version);
});
