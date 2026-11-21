/**
 * Find credentials without making the user think about it.
 *
 * Sources are tried in this order, and the first one that yields something wins:
 *
 * 1. options passed to `new Client()` (handled by the client itself),
 * 2. the environment variable `ZINDI_TOKEN`, or `ZINDI_USERNAME` and `ZINDI_PASSWORD`,
 * 3. the credentials file written by `login()` or `zindi login`.
 *
 * @module
 */

import { credentialsPath, loadCredentials } from "./config.js";

/** What was found, and where it was found. */
export interface Credentials {
  /** Where the credentials came from, e.g. `"environment variables"` or a file path. */
  source: string;
  /** An auth token, when the source has one. */
  token: string | null;
  /** A username, when the source has one. */
  username: string | null;
  /** A password, when the source has one (never read from the credentials file). */
  password: string | null;
}

/** `true` when the credentials hold both a username and a password. */
export function canSignIn(credentials: Credentials): boolean {
  return Boolean(credentials.username && credentials.password);
}

/** Credentials from `ZINDI_TOKEN`, or `ZINDI_USERNAME` and `ZINDI_PASSWORD`. */
export function fromEnvironment(): Credentials | null {
  const token = process.env.ZINDI_TOKEN || null;
  const username = process.env.ZINDI_USERNAME || null;
  const password = process.env.ZINDI_PASSWORD || null;
  if (token || (username && password)) {
    return { source: "environment variables", token, username, password };
  }
  return null;
}

/** Credentials from the shared credentials file. */
export async function fromSavedFile(): Promise<Credentials | null> {
  const saved = await loadCredentials();
  if (saved === null) return null;
  return {
    source: credentialsPath(),
    token: saved.authToken || null,
    username: saved.username,
    password: null,
  };
}

/** Options of {@link resolveCredentials}. */
export interface ResolveOptions {
  /** Also read the credentials file. Defaults to `true`. */
  useSaved?: boolean;
}

/** Return the first credentials found, or `null`. */
export async function resolveCredentials(options: ResolveOptions = {}): Promise<Credentials | null> {
  const fromEnv = fromEnvironment();
  if (fromEnv) return fromEnv;
  if (options.useSaved ?? true) return fromSavedFile();
  return null;
}
