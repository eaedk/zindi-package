/**
 * Where zindi keeps its files, and how it reads them.
 *
 * Two files matter:
 *
 * - **Credentials**: `<config dir>/credentials.json` holds your username and an auth token
 *   (never your password). It is shared by every zindi client (Python, R, JavaScript, Rust),
 *   so logging in once is enough.
 * - **Project file**: `zindi.toml` in your project folder (or any parent folder) remembers
 *   the competition you work on, so calls need no competition id.
 *
 * The config dir is, in order of priority: `$ZINDI_CONFIG_DIR`; `%APPDATA%\zindi` on Windows;
 * `$XDG_CONFIG_HOME/zindi`; `~/.config/zindi`.
 *
 * @module
 */

import { randomBytes } from "node:crypto";
import { chmod, mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";

import { isObject } from "./util.js";

/** Name of the project file that remembers the current competition. */
export const PROJECT_FILE = "zindi.toml";

const COMPETITION_LINE = /^\s*competition\s*=/;

/** Credentials stored on disk by `login()` or `zindi login`. */
export interface SavedCredentials {
  /** The Zindi username. */
  username: string;
  /** The auth token returned by Zindi when signing in. */
  authToken: string;
}

function expandHome(path: string): string {
  if (path === "~") return homedir();
  if (path.startsWith("~/") || path.startsWith("~\\")) return join(homedir(), path.slice(2));
  return path;
}

/** The folder that holds zindi's user-level files. */
export function configDir(): string {
  const override = process.env.ZINDI_CONFIG_DIR;
  if (override) return expandHome(override);
  const appData = process.env.APPDATA;
  if (process.platform === "win32" && appData) return join(appData, "zindi");
  const xdg = process.env.XDG_CONFIG_HOME;
  if (xdg) return join(expandHome(xdg), "zindi");
  return join(homedir(), ".config", "zindi");
}

/** The path of the shared credentials file. */
export function credentialsPath(): string {
  return join(configDir(), "credentials.json");
}

/** Read saved credentials, or `null` if there are none (or they are unreadable). */
export async function loadCredentials(): Promise<SavedCredentials | null> {
  try {
    const data: unknown = JSON.parse(await readFile(credentialsPath(), "utf8"));
    if (!isObject(data)) return null;
    const { username, auth_token: authToken } = data;
    if (username === null || username === undefined) return null;
    if (authToken === null || authToken === undefined) return null;
    return { username: String(username), authToken: String(authToken) };
  } catch {
    return null;
  }
}

function utcTimestamp(): string {
  // Same shape as the other clients: 2026-11-21T08:00:00+00:00
  return new Date().toISOString().replace(/\.\d{3}Z$/, "+00:00");
}

/**
 * Save credentials atomically, readable by the current user only (mode `0600`).
 *
 * @returns The path of the credentials file.
 */
export async function saveCredentials(username: string, authToken: string): Promise<string> {
  const path = credentialsPath();
  await mkdir(dirname(path), { recursive: true });
  const payload = { username, auth_token: authToken, saved_at: utcTimestamp() };
  const tmp = join(dirname(path), `.credentials-${randomBytes(6).toString("hex")}.json`);
  try {
    await writeFile(tmp, `${JSON.stringify(payload, null, 2)}\n`, { encoding: "utf8", mode: 0o600 });
    if (process.platform !== "win32") await chmod(tmp, 0o600);
    await rename(tmp, path);
  } catch (error) {
    await rm(tmp, { force: true });
    throw error;
  }
  return path;
}

/**
 * Delete saved credentials.
 *
 * @returns `true` if a file was removed.
 */
export async function deleteCredentials(): Promise<boolean> {
  try {
    await rm(credentialsPath());
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return false;
    throw error;
  }
}

async function isFile(path: string): Promise<boolean> {
  try {
    return (await stat(path)).isFile();
  } catch {
    return false;
  }
}

/** Find `zindi.toml` in `start` (default: the current folder) or one of its parents. */
export async function findProjectFile(start?: string): Promise<string | null> {
  let folder = resolve(start ?? process.cwd());
  for (;;) {
    const candidate = join(folder, PROJECT_FILE);
    if (await isFile(candidate)) return candidate;
    const parent = dirname(folder);
    if (parent === folder) return null;
    folder = parent;
  }
}

/**
 * Read the top-level `competition` key of a TOML document.
 *
 * This is deliberately not a full TOML parser: it understands basic (`"..."`) and literal
 * (`'...'`) strings, comments, and stops at the first table header.
 */
export function parseProjectCompetition(text: string): string | null {
  for (const line of text.split(/\r\n|\n|\r/)) {
    if (/^\s*\[/.test(line)) break;
    const match = /^\s*competition\s*=\s*(.*)$/.exec(line);
    if (!match) continue;
    const value = (match[1] ?? "").trim();
    const basic = /^"((?:[^"\\]|\\.)*)"/.exec(value);
    if (basic) {
      try {
        return JSON.parse(`"${basic[1]}"`) || null;
      } catch {
        return basic[1] || null;
      }
    }
    const literal = /^'([^']*)'/.exec(value);
    if (literal) return literal[1] || null;
    return null;
  }
  return null;
}

/** The competition id stored in the nearest `zindi.toml`, if any. */
export async function readProjectCompetition(start?: string): Promise<string | null> {
  const path = await findProjectFile(start);
  if (path === null) return null;
  try {
    return parseProjectCompetition(await readFile(path, "utf8"));
  } catch {
    return null;
  }
}

/**
 * Write (or update) `zindi.toml` in `folder` (default: the current folder) with the given
 * competition id. Other keys of the file are kept.
 *
 * @returns The path of the project file.
 */
export async function writeProjectCompetition(competition: string, folder?: string): Promise<string> {
  const path = join(folder ?? process.cwd(), PROJECT_FILE);
  let lines: string[] = [];
  if (await isFile(path)) {
    const text = await readFile(path, "utf8");
    lines = text.split(/\r\n|\n|\r/);
    if (lines[lines.length - 1] === "") lines.pop();
    lines = lines.filter((line) => !COMPETITION_LINE.test(line));
  }
  // A JSON string literal is also a valid TOML basic string.
  lines.unshift(`competition = ${JSON.stringify(competition)}`);
  await writeFile(path, `${lines.join("\n")}\n`, "utf8");
  return path;
}
