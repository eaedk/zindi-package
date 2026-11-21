import assert from "node:assert/strict";
import { existsSync, readFileSync, statSync } from "node:fs";
import { afterEach, beforeEach, test } from "node:test";

import { AuthenticationError, Client, credentialsPath, saveCredentials } from "../dist/index.js";
import { anonClient, fixture, isolate, MockApi } from "./helpers.mjs";

let env;
let api;
beforeEach(() => {
  env = isolate();
  api = new MockApi();
});
afterEach(() => env.restore());

test("login saves the token but never the password", async () => {
  const route = api.on("POST", "/auth/signin", { json: fixture("signin_ok") });
  const client = anonClient(api);

  const user = await client.login("amina_dev", "hunter2");

  assert.equal(user.username, "amina_dev");
  assert.equal(client.token, "tok_3f9a1c2e7b");
  assert.equal(client.username, "amina_dev");
  assert.equal(route.lastCall.body, "username=amina_dev&password=hunter2");
  assert.equal(route.lastCall.headers.has("auth-token"), false);
  const text = readFileSync(credentialsPath(), "utf8");
  const saved = JSON.parse(text);
  assert.equal(saved.username, "amina_dev");
  assert.equal(saved.auth_token, "tok_3f9a1c2e7b");
  assert.match(saved.saved_at, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\+00:00$/);
  assert.equal(text.includes("hunter2"), false);
  if (process.platform !== "win32") {
    assert.equal(statSync(credentialsPath()).mode & 0o777, 0o600);
  }
});

test("login with save: false writes nothing", async () => {
  api.on("POST", "/auth/signin", { json: fixture("signin_ok") });
  await anonClient(api).login("amina_dev", "hunter2", { save: false });
  assert.equal(existsSync(credentialsPath()), false);
});

test("a wrong password points to the reset page", async () => {
  api.on("POST", "/auth/signin", { json: fixture("signin_wrong_password") });
  await assert.rejects(anonClient(api).login("amina_dev", "nope"), (error) => {
    assert.ok(error instanceof AuthenticationError);
    assert.match(error.message, /https:\/\/zindi\.world\/passwords\/request/);
    assert.match(error.message, /wrong password/);
    return true;
  });
  assert.equal(existsSync(credentialsPath()), false);
});

test("login takes missing values from the environment", async () => {
  process.env.ZINDI_USERNAME = "amina_dev";
  process.env.ZINDI_PASSWORD = "hunter2";
  const route = api.on("POST", "/auth/signin", { json: fixture("signin_ok") });
  await anonClient(api).login(undefined, undefined, { save: false });
  assert.equal(route.lastCall.body, "username=amina_dev&password=hunter2");
});

test("login without values explains what is missing", async () => {
  await assert.rejects(anonClient(api).login(), { name: "AuthenticationError", message: /username/ });
  await assert.rejects(anonClient(api).login("amina_dev"), { name: "AuthenticationError", message: /password/ });
});

test("saved credentials are used", async () => {
  await saveCredentials("amina_dev", "tok_saved");
  const route = api.on("GET", "/me", { json: fixture("me") });
  const client = new Client({ fetch: api.fetch });

  assert.equal((await client.me()).username, "amina_dev");

  assert.equal(client.credentialsSource, credentialsPath());
  assert.equal(client.username, "amina_dev");
  assert.equal(route.lastCall.headers.get("auth-token"), "tok_saved");
});

test("useSavedCredentials: false ignores the saved file", async () => {
  await saveCredentials("amina_dev", "tok_saved");
  const client = new Client({ fetch: api.fetch, useSavedCredentials: false });
  assert.equal(await client.isAuthenticated(), false);
});

test("ZINDI_TOKEN beats the saved file", async () => {
  await saveCredentials("someone", "tok_saved");
  process.env.ZINDI_TOKEN = "tok_env";
  const route = api.on("GET", "/me", { json: fixture("me") });
  const client = new Client({ fetch: api.fetch });

  await client.me();

  assert.equal(client.credentialsSource, "environment variables");
  assert.equal(route.lastCall.headers.get("auth-token"), "tok_env");
});

test("an explicit token beats the environment", async () => {
  process.env.ZINDI_TOKEN = "tok_env";
  const route = api.on("GET", "/me", { json: fixture("me") });
  const client = new Client({ token: "tok_arg", fetch: api.fetch });

  await client.me();

  assert.equal(client.credentialsSource, "argument");
  assert.equal(route.lastCall.headers.get("auth-token"), "tok_arg");
});

test("ZINDI_USERNAME and ZINDI_PASSWORD sign in lazily", async () => {
  process.env.ZINDI_USERNAME = "amina_dev";
  process.env.ZINDI_PASSWORD = "hunter2";
  const signin = api.on("POST", "/auth/signin", { json: fixture("signin_ok") });
  const me = api.on("GET", "/me", { json: fixture("me") });
  const client = new Client({ fetch: api.fetch });

  assert.equal(signin.called, false);
  await client.me();

  assert.equal(signin.callCount, 1);
  assert.equal(me.lastCall.headers.get("auth-token"), "tok_3f9a1c2e7b");
  assert.equal(existsSync(credentialsPath()), false); // lazy sign-in does not save anything
});

test("an expired token is refreshed once", async () => {
  const signin = api.on("POST", "/auth/signin", { json: fixture("signin_ok") });
  const me = api.on("GET", "/me", { status: 401, json: fixture("error_not_authorized") }, { json: fixture("me") });
  const client = new Client({ token: "old", username: "amina_dev", password: "hunter2", fetch: api.fetch });

  assert.equal((await client.me()).username, "amina_dev");

  assert.equal(client.token, "tok_3f9a1c2e7b");
  assert.equal(signin.callCount, 1);
  assert.equal(me.callCount, 2);
  assert.equal(me.calls[0].headers.get("auth-token"), "old");
  assert.equal(me.calls[1].headers.get("auth-token"), "tok_3f9a1c2e7b");
});

test("without a password an expired token is an error", async () => {
  api.on("GET", "/me", { status: 401, json: fixture("error_not_authorized") });
  await assert.rejects(new Client({ token: "old", fetch: api.fetch }).me(), AuthenticationError);
});

test("'not authorized' inside a 200 envelope is an AuthenticationError", async () => {
  api.on("GET", "/me", { json: fixture("error_not_authorized") });
  await assert.rejects(new Client({ token: "t", fetch: api.fetch }).me(), AuthenticationError);
});

test("logout forgets everything", async () => {
  api.on("POST", "/auth/signin", { json: fixture("signin_ok") });
  const client = anonClient(api);
  await client.login("amina_dev", "hunter2");
  assert.equal(existsSync(credentialsPath()), true);

  await client.logout();

  assert.equal(client.token, null);
  assert.equal(existsSync(credentialsPath()), false);
  assert.equal(await client.isAuthenticated(), false);
});
