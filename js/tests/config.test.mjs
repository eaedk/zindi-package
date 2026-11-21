import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, test } from "node:test";

import {
  configDir,
  credentialsPath,
  deleteCredentials,
  findProjectFile,
  htmlToText,
  loadCredentials,
  readProjectCompetition,
  saveCredentials,
  writeProjectCompetition,
} from "../dist/index.js";
import { parseProjectCompetition } from "../dist/config.js";
import { isolate } from "./helpers.mjs";

let env;
beforeEach(() => {
  env = isolate();
});
afterEach(() => env.restore());

test("the config dir follows ZINDI_CONFIG_DIR, then XDG_CONFIG_HOME, then ~/.config", () => {
  assert.equal(configDir(), join(env.root, "config"));
  assert.equal(credentialsPath(), join(env.root, "config", "credentials.json"));
  if (process.platform === "win32") return; // %APPDATA% wins there
  delete process.env.ZINDI_CONFIG_DIR;
  process.env.XDG_CONFIG_HOME = join(env.root, "xdg");
  assert.equal(configDir(), join(env.root, "xdg", "zindi"));
  delete process.env.XDG_CONFIG_HOME;
  assert.equal(configDir(), join(homedir(), ".config", "zindi"));
});

test("credentials round trip", async () => {
  assert.equal(await loadCredentials(), null);
  const path = await saveCredentials("amina_dev", "tok_1");
  assert.equal(path, credentialsPath());
  assert.deepEqual(await loadCredentials(), { username: "amina_dev", authToken: "tok_1" });
  assert.equal(await deleteCredentials(), true);
  assert.equal(await deleteCredentials(), false);
});

test("unreadable credentials are ignored", async () => {
  mkdirSync(join(env.root, "config"), { recursive: true });
  writeFileSync(credentialsPath(), "{not json");
  assert.equal(await loadCredentials(), null);
  writeFileSync(credentialsPath(), JSON.stringify({ username: "x" }));
  assert.equal(await loadCredentials(), null);
});

test("project file round trip keeps other keys", async () => {
  writeFileSync(join(env.root, "zindi.toml"), '# my project\ncompetition = "Old"\nother = 1\n');
  await writeProjectCompetition("New-One", env.root);
  const nested = join(env.root, "a", "b");
  mkdirSync(nested, { recursive: true });

  assert.equal(await readProjectCompetition(nested), "New-One");
  assert.equal(await findProjectFile(nested), join(env.root, "zindi.toml"));
  assert.equal(
    readFileSync(join(env.root, "zindi.toml"), "utf8"),
    'competition = "New-One"\n# my project\nother = 1\n',
  );
});

test("no project file means no competition", async () => {
  assert.equal(await readProjectCompetition(env.work), null);
});

test("simple TOML parsing", () => {
  assert.equal(parseProjectCompetition('competition = "A-B"'), "A-B");
  assert.equal(parseProjectCompetition("competition='Lit'  # note"), "Lit");
  assert.equal(parseProjectCompetition('  competition = "Q\\"uote" # c'), 'Q"uote');
  assert.equal(parseProjectCompetition('# competition = "No"\nname = "x"'), null);
  assert.equal(parseProjectCompetition('[tool]\ncompetition = "Nested"'), null);
  assert.equal(parseProjectCompetition('competition = ""'), null);
});

test("html to text", () => {
  assert.equal(htmlToText("<ul><li>One &amp; two</li><li>Three</li></ul>"), "• One & two\n• Three");
  assert.equal(htmlToText("<p>a</p><br/><br/><br/><p>b&#33;</p>"), "a\n\nb!");
  assert.equal(htmlToText(null), "");
});
