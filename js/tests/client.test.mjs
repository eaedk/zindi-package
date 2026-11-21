import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach, describe, test } from "node:test";

import {
  APIError,
  AuthenticationError,
  Client,
  CompetitionNotSetError,
  entryIncludes,
  NotFoundError,
  SecretCodeRequiredError,
  ZindiError,
} from "../dist/index.js";
import { anonClient, API, COMP, fixture, isolate, MockApi, tokenClient } from "./helpers.mjs";

let env;
let api;
beforeEach(() => {
  env = isolate();
  api = new MockApi();
});
afterEach(() => env.restore());

describe("competitions", () => {
  test("lists open competitions without login", async () => {
    const route = api.on("GET", "/competitions", { json: fixture("competitions") });

    const comps = await anonClient(api).competitions();

    assert.deepEqual(
      comps.map((c) => c.id),
      [COMP, "Swahili-ASR-Hackathon"],
    );
    const { url, headers } = route.lastCall;
    assert.equal(url.searchParams.get("active"), "true");
    assert.equal(url.searchParams.get("page"), "0");
    assert.equal(url.searchParams.get("per_page"), "50");
    assert.equal(headers.has("auth-token"), false);
    assert.equal(comps[0].raw.id, COMP);
  });

  test("sends filters and enforces them client-side", async () => {
    const route = api.on("GET", "/competitions", { json: fixture("competitions") });

    const comps = await anonClient(api).competitions({
      status: "all",
      kind: "hackathon",
      reward: "knowledge",
      search: "asr",
    });

    const params = route.lastCall.url.searchParams;
    assert.equal(params.get("kind[]"), "hackathon");
    assert.equal(params.get("prize"), "knowledge");
    assert.equal(params.get("query"), "asr");
    assert.equal(params.has("active"), false);
    assert.deepEqual(
      comps.map((c) => c.id),
      ["Swahili-ASR-Hackathon"],
    );
  });

  test("closed status and limit", async () => {
    api.on("GET", "/competitions", { json: fixture("competitions") });
    const client = anonClient(api);
    assert.deepEqual(
      (await client.competitions({ status: "closed" })).map((c) => c.id),
      ["Air-Quality-Lagos"],
    );
    assert.equal((await client.competitions({ status: "all", limit: 1 })).length, 1);
  });

  test("paginates until the total is reached", async () => {
    const data = fixture("competitions").data;
    const first = { data: Array.from({ length: 25 }, () => data.slice(0, 2)).flat(), meta: { total_count: 51 } };
    const second = { data: data.slice(2), meta: { total_count: 51 } };
    const route = api.on("GET", "/competitions", { json: first }, { json: second });

    const comps = await anonClient(api).competitions({ status: "all", limit: null });

    assert.equal(comps.length, 51);
    assert.deepEqual(
      route.calls.map((c) => c.url.searchParams.get("page")),
      ["0", "1"],
    );
  });

  test("competition details", async () => {
    api.on("GET", `/competitions/${COMP}`, { json: fixture("competition") });

    const comp = await anonClient(api).competition(COMP);

    assert.equal(comp.title, "Crop Yield Forecast Challenge");
    assert.equal(comp.metric, "RMSE");
    assert.equal(comp.maxTeamSize, 4);
    assert.equal(comp.dailySubmissionLimit, 10);
    assert.equal(comp.status, "open");
    assert.deepEqual(
      comp.dataFiles.slice(0, 2).map((f) => f.filename),
      ["Train.csv", "Test.csv"],
    );
    assert.equal(comp.dataFiles[0].sizeHuman, "1.0 MB");
    assert.equal(comp.dataFiles[3].isArchiveOfEverything, true);
    assert.match(comp.description, /maize/);
    assert.match(comp.rules, /10 submissions per day/);
    assert.equal(comp.url, `https://zindi.world/competitions/${COMP}`);
    assert.ok(comp.endTime instanceof Date);
    assert.equal(comp.endTime.getUTCFullYear(), 2026);
    assert.equal(comp.raw.difficulty, "intermediate");
  });

  test("a missing competition throws NotFoundError", async () => {
    api.on("GET", "/competitions/nope", { status: 404, json: { data: { errors: { message: "not found" } } } });
    await assert.rejects(anonClient(api).competition("nope"), NotFoundError);
  });

  test("competition id resolution order", async () => {
    const client = anonClient(api);
    await assert.rejects(client.resolveCompetition(), CompetitionNotSetError);
    writeFileSync(join(env.work, "zindi.toml"), 'competition = "From-File"\n');
    assert.equal(await client.resolveCompetition(), "From-File");
    process.env.ZINDI_COMPETITION = "From-Env";
    assert.equal(await client.resolveCompetition(), "From-Env");
    await client.use("From-Use");
    assert.equal(await client.resolveCompetition(), "From-Use");
    assert.equal(await client.resolveCompetition("Explicit"), "Explicit");
  });

  test("the competition option of the constructor is the default", async () => {
    const client = anonClient(api, { competition: "From-Option" });
    assert.equal(await client.resolveCompetition(), "From-Option");
  });

  test("use() with save writes the project file", async () => {
    await anonClient(api).use(COMP, { save: true });
    assert.equal(readFileSync(join(env.work, "zindi.toml"), "utf8"), `competition = "${COMP}"\n`);
  });
});

describe("joining", () => {
  test("sends the token and reports a new participation", async () => {
    const route = api.on("POST", `/competitions/${COMP}/participations`, { json: fixture("join_ok") });
    assert.equal(await tokenClient(api).join(COMP), true);
    const { headers, body } = route.lastCall;
    assert.equal(headers.get("auth-token"), "tok_test");
    assert.equal(headers.get("auth_token"), "tok_test");
    assert.equal(body, "");
  });

  test("returns false when already in", async () => {
    api.on("POST", `/competitions/${COMP}/participations`, { json: fixture("join_already") });
    assert.equal(await tokenClient(api).join(COMP), false);
  });

  test("private competitions need a secret code", async () => {
    const route = api.on("POST", `/competitions/${COMP}/participations`, { json: fixture("join_secret_required") });
    const client = tokenClient(api);
    await assert.rejects(client.join(COMP), SecretCodeRequiredError);
    route.replies = [{ json: fixture("join_ok") }];
    route.calls = [];
    assert.equal(await client.join(COMP, { secretCode: "s3cret" }), true);
    assert.deepEqual(JSON.parse(route.lastCall.body), { secret_code: "s3cret" });
    assert.equal(route.lastCall.headers.get("content-type"), "application/json");
  });
});

describe("downloads", () => {
  function detailWith(files) {
    const detail = fixture("competition");
    detail.data.datafiles = files;
    return detail;
  }

  test("skips the everything archive and files already there", async () => {
    api.on("GET", `/competitions/${COMP}`, { json: fixture("competition") });
    const train = api.on("GET", `/competitions/${COMP}/files/Train.csv`, { body: "a".repeat(1048576) });
    api.on("GET", `/competitions/${COMP}/files/Test.csv`, { body: "b".repeat(262144) });
    api.on("GET", `/competitions/${COMP}/files/SampleSubmission.csv`, { body: "c".repeat(2048) });
    const archive = api.on("GET", `/competitions/${COMP}/files/all_data.zip`, { body: "zip" });
    const client = tokenClient(api);
    const dest = join(env.root, "data");

    const paths = await client.download(COMP, dest);

    assert.deepEqual(
      paths.map((p) => p.split(/[\\/]/).pop()),
      ["Train.csv", "Test.csv", "SampleSubmission.csv"],
    );
    assert.equal(statSync(join(dest, "Train.csv")).size, 1048576);
    assert.equal(archive.called, false);
    assert.deepEqual(
      readdirSync(dest).filter((name) => name.endsWith(".part")),
      [],
    );

    await client.download(COMP, dest);
    assert.equal(train.callCount, 1); // second run: already there, same size
  });

  test("downloads only the selected files, and overwrites on request", async () => {
    api.on("GET", `/competitions/${COMP}`, { json: fixture("competition") });
    const route = api.on("GET", `/competitions/${COMP}/files/SampleSubmission.csv`, { body: "c".repeat(2048) });
    const client = tokenClient(api);

    await client.download(COMP, env.root, { files: ["SampleSubmission.csv"] });
    await client.download(COMP, env.root, { files: ["SampleSubmission.csv"], overwrite: true });

    assert.equal(route.callCount, 2);
    await assert.rejects(client.download(COMP, env.root, { files: ["nope.csv"] }), {
      name: "ZindiError",
      message: /Unknown file/,
    });
  });

  test("downloads the archive when it is the only file", async () => {
    api.on("GET", `/competitions/${COMP}`, {
      json: detailWith([{ filename: "all_data.zip", filesize: 3, zip_of_everything: true }]),
    });
    const archive = api.on("GET", `/competitions/${COMP}/files/all_data.zip`, { body: "zip" });
    await tokenClient(api).download(COMP, env.root);
    assert.equal(archive.callCount, 1);
  });

  test("follows redirects without leaking the token", async () => {
    const signedUrl = "https://storage.example.com/signed/Train.csv";
    api.on("GET", `/competitions/${COMP}`, { json: detailWith([{ filename: "Train.csv", filesize: 4 }]) });
    const file = api.on("GET", `/competitions/${COMP}/files/Train.csv`, {
      status: 302,
      headers: { Location: signedUrl },
    });
    const signed = api.on("GET", signedUrl, { body: "data" });

    await tokenClient(api).download(COMP, env.root);

    assert.equal(readFileSync(join(env.root, "Train.csv"), "utf8"), "data");
    assert.equal(file.lastCall.init.redirect, "manual");
    assert.equal(file.lastCall.headers.get("auth-token"), "tok_test");
    const { headers } = signed.lastCall;
    assert.equal(headers.has("auth-token"), false);
    assert.equal(headers.has("auth_token"), false);
    assert.match(headers.get("user-agent"), /^zindi-js\//);
  });

  test("a refused download suggests joining and leaves nothing behind", async () => {
    api.on("GET", `/competitions/${COMP}`, { json: detailWith([{ filename: "Train.csv", filesize: 4 }]) });
    api.on("GET", `/competitions/${COMP}/files/Train.csv`, {
      status: 401,
      json: { data: { errors: { message: "not authorized" } } },
    });

    await assert.rejects(tokenClient(api).download(COMP, env.root), (error) => {
      assert.ok(error instanceof AuthenticationError);
      assert.match(error.message, new RegExp(`zindi join ${COMP}`));
      assert.equal(error.statusCode, 401);
      return true;
    });
    assert.equal(existsSync(join(env.root, "Train.csv")), false);
    assert.equal(existsSync(join(env.root, "Train.csv.part")), false);
  });

  test("refuses file names that would escape the destination", async () => {
    api.on("GET", `/competitions/${COMP}`, { json: detailWith([{ filename: "../evil.csv", filesize: 4 }]) });
    await assert.rejects(tokenClient(api).download(COMP, join(env.root, "data")), {
      name: "ZindiError",
      message: /unsafe/,
    });
  });

  test("needs a login", async () => {
    api.on("GET", `/competitions/${COMP}`, { json: fixture("competition") });
    await assert.rejects(anonClient(api).download(COMP, env.root), AuthenticationError);
  });
});

describe("submissions", () => {
  test("uploads the file and the comment as multipart", async () => {
    const route = api.on("POST", `/competitions/${COMP}/submissions`, { json: fixture("submission_created") });
    const path = join(env.root, "submission.csv");
    writeFileSync(path, "ID,Yield\n1,2.5\n");

    const sub = await tokenClient(api).submit(path, { comment: "first try", competition: COMP });

    assert.equal(sub.id, "sub_d4");
    assert.equal(sub.isPending, true);
    const { body, headers, init } = route.lastCall;
    // fetch adds the multipart Content-Type (with its boundary) by itself.
    assert.ok(init.body instanceof FormData);
    assert.equal(headers.has("content-type"), false);
    assert.ok(body.includes('name="comment"') && body.includes("first try"));
    assert.ok(body.includes('filename="submission.csv"') && body.includes("ID,Yield"));
    assert.ok(body.includes("Content-Type: text/csv"));
  });

  test("wait polls until the submission is scored", async () => {
    api.on("POST", `/competitions/${COMP}/submissions`, { json: fixture("submission_created") });
    const pending = { data: [fixture("submission_created").data] };
    const history = api.on("GET", `/competitions/${COMP}/submissions`, { json: pending }, { json: fixture("submission_scored") });
    const path = join(env.root, "submission.csv");
    writeFileSync(path, "x");
    const polls = [];

    const sub = await tokenClient(api, { sleep: async (s) => void polls.push(s) }).submit(path, {
      competition: COMP,
      wait: true,
      pollInterval: 5,
    });

    assert.equal(sub.isScored, true);
    assert.ok(Math.abs(sub.publicScore - 0.4211) < 1e-9);
    assert.equal(history.callCount, 2);
    assert.deepEqual(polls, [5]);
  });

  test("waitForScore times out", async () => {
    api.on("GET", `/competitions/${COMP}/submissions`, { json: { data: [fixture("submission_created").data] } });
    await assert.rejects(tokenClient(api).waitForScore(COMP, "sub_d4", { timeout: 0 }), {
      name: "ZindiError",
      message: /not scored/,
    });
  });

  test("a missing file is reported", async () => {
    await assert.rejects(tokenClient(api).submit(join(env.root, "nope.csv"), { competition: COMP }), (error) => {
      assert.ok(error instanceof ZindiError);
      assert.match(error.message, /not found/);
      return true;
    });
  });

  test("lists submissions with their statuses", async () => {
    const route = api.on("GET", `/competitions/${COMP}/submissions`, { json: fixture("submissions") });
    const client = tokenClient(api);

    const subs = await client.submissions(COMP);

    assert.deepEqual(
      subs.map((s) => s.statusLabel),
      ["scoring…", "scored", "failed"],
    );
    assert.equal(subs[2].note, "Missing column: Yield");
    assert.equal(subs[1].score, 0.4377);
    assert.equal(subs[1].selected, true);
    assert.ok(subs[0].createdAt instanceof Date);
    assert.equal(route.lastCall.url.searchParams.get("per_page"), "1000");
    assert.equal((await client.submissions(COMP, { limit: 1 })).length, 1);
  });

  test("limits", async () => {
    api.on("GET", `/competitions/${COMP}/submissions/limits`, { json: fixture("submission_limits") });
    assert.equal((await tokenClient(api).limits(COMP)).remainingToday, 7);
  });
});

describe("leaderboard", () => {
  test("stops at the first unranked row", async () => {
    const route = api.on("GET", `/competitions/${COMP}/participations`, { json: fixture("leaderboard") });
    const client = anonClient(api);

    const board = await client.leaderboard(COMP);

    assert.deepEqual(
      board.map((e) => e.rank),
      [1, 2, 3],
    );
    const team = board[1];
    assert.equal(team.isTeam, true);
    assert.equal(team.name, "Lagos Learners");
    assert.equal(team.countrycode, "ng");
    assert.equal(entryIncludes(team, "AMINA_DEV"), true);
    assert.equal(board[0].lastSubmissionAt.toISOString(), "2026-11-18T10:02:11.000Z");
    assert.equal(route.lastCall.url.searchParams.get("per_page"), "100");
    assert.equal((await client.leaderboard(COMP, { includeUnranked: true })).length, 4);
    assert.equal((await client.leaderboard(COMP, { limit: 2 })).length, 2);
  });

  test("rank comes from my participation", async () => {
    api.on("GET", `/competitions/${COMP}/participations/my_participation`, { json: fixture("my_participation") });
    const client = tokenClient(api);
    assert.equal(await client.rank(COMP), 2);
    assert.equal((await client.myParticipation(COMP)).teamTitle, "Lagos Learners");
  });

  test("rank falls back to a leaderboard search", async () => {
    api.on("GET", `/competitions/${COMP}/participations/my_participation`, { json: { data: { public_rank: null } } });
    api.on("GET", `/competitions/${COMP}/participations`, { json: fixture("leaderboard") });
    const client = new Client({ token: "t", username: "zawadi_k", fetch: api.fetch });
    assert.equal(await client.rank(COMP), 3);
  });
});

describe("teams", () => {
  test("create, invite, show and disband", async () => {
    const create = api.on("POST", `/competitions/${COMP}/my_team`, { json: fixture("team_created") });
    const invite = api.on(
      "POST",
      `/competitions/${COMP}/my_team/invite`,
      { json: fixture("team_invite_ok") },
      { json: fixture("team_invite_already") },
    );
    api.on("GET", `/competitions/${COMP}/my_team`, { json: fixture("my_team") });
    const disband = api.on("DELETE", `/competitions/${COMP}/my_team`, { json: fixture("team_disbanded") });
    const client = tokenClient(api);

    const team = await client.createTeam("Lagos Learners", { competition: COMP, invite: ["zawadi_k", "tolu_a"] });

    assert.equal(team.title, "Lagos Learners");
    assert.equal(team.leader, "amina_dev");
    assert.equal(create.lastCall.body, "title=Lagos+Learners");
    assert.equal(create.lastCall.headers.get("content-type"), "application/x-www-form-urlencoded");
    assert.equal(invite.callCount, 2);
    assert.deepEqual(
      invite.calls.map((c) => c.body),
      ["username=zawadi_k", "username=tolu_a"],
    );
    const shown = await client.team(COMP);
    assert.deepEqual(shown.members, ["amina_dev", "tolu_a"]);
    assert.deepEqual(shown.pendingInvitations, ["zawadi_k"]);
    await client.disbandTeam(COMP);
    assert.equal(disband.called, true);
  });

  test("team() is null when competing alone", async () => {
    api.on("GET", `/competitions/${COMP}/my_team`, { status: 404, json: { data: { errors: { message: "not found" } } } });
    assert.equal(await tokenClient(api).team(COMP), null);
  });

  test("invite() accepts one username and raises other errors", async () => {
    api.on("POST", `/competitions/${COMP}/my_team/invite`, { json: { data: { errors: { base: "User does not exist" } } } });
    await assert.rejects(tokenClient(api).invite("ghost", { competition: COMP }), (error) => {
      assert.ok(error instanceof APIError);
      assert.match(error.message, /does not exist/);
      return true;
    });
  });

  test("being the leader of another team is an APIError", async () => {
    api.on("POST", `/competitions/${COMP}/my_team`, { json: fixture("team_already_leader") });
    await assert.rejects(tokenClient(api).createTeam("Again", { competition: COMP }), {
      name: "APIError",
      message: /leader of one team/,
    });
  });
});

describe("users and discussions", () => {
  test("user profile", async () => {
    api.on("GET", "/users/amina_dev", { json: fixture("user_profile") });
    const user = await anonClient(api).user("amina_dev");
    assert.equal(user.fullName, "Amina Diallo");
    assert.deepEqual([user.goldMedals, user.silverMedals, user.bronzeMedals], [1, 2, 4]);
    assert.equal(user.competitionsCount, 14);
    assert.equal(user.url, "https://zindi.world/users/amina_dev");
    assert.equal(user.joinedAt.toISOString(), "2023-04-02T09:12:44.000Z");
  });

  test("me() requires a login", async () => {
    await assert.rejects(anonClient(api).me(), (error) => {
      assert.ok(error instanceof AuthenticationError);
      assert.match(error.message, /zindi login/);
      return true;
    });
  });

  test("discussions", async () => {
    api.on("GET", `/competitions/${COMP}/discussions`, { json: fixture("discussions") });
    const threads = await anonClient(api).discussions(COMP);
    assert.equal(threads.length, 2);
    assert.equal(threads[0].author, "zawadi_k");
    assert.equal(threads[0].id, "7001");
    assert.equal(threads[0].createdAt.toISOString().slice(0, 10), "2026-11-15");
    assert.ok(threads[0].url.startsWith(`https://zindi.world/competitions/${COMP}/discussions/7001`));
    assert.equal((await anonClient(api).discussions(COMP, { limit: 1 })).length, 1);
  });
});

test("requests go to the configured base URL", async () => {
  const route = api.on("GET", "/users/amina_dev", { json: fixture("user_profile") });
  const client = anonClient(api);
  assert.equal(client.baseUrl, API);
  await client.user("amina_dev");
  assert.equal(route.callCount, 1);
});
