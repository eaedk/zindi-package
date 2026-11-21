/**
 * Errors thrown by the zindi package.
 *
 * Every error extends {@link ZindiError}, so `error instanceof ZindiError` catches
 * anything this package throws on purpose.
 *
 * @module
 */

/** Where users can reset a forgotten password. Mentioned in login errors. */
export const PASSWORD_RESET_URL = "https://zindi.world/passwords/request";

/** Base class for every error thrown by this package. */
export class ZindiError extends Error {
  /**
   * @param message - A sentence that explains what went wrong.
   * @param options - Standard error options (`cause`).
   */
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = new.target.name;
  }
}

/** Options accepted by {@link APIError} and its subclasses. */
export interface APIErrorOptions {
  /** HTTP status of the response, when there was one. */
  statusCode?: number;
  /** The `errors` object returned by Zindi, when there is one. */
  errors?: unknown;
  /** The underlying error (for example a network failure). */
  cause?: unknown;
}

/** The Zindi API answered with an error, or could not be reached. */
export class APIError extends ZindiError {
  /** HTTP status of the response, or `undefined` when no response was received. */
  readonly statusCode: number | undefined;
  /** The `errors` object returned by Zindi, when there is one. */
  readonly errors: unknown;

  /**
   * @param message - A readable description of the error.
   * @param options - Status code, raw `errors` object and cause.
   */
  constructor(message: string, options: APIErrorOptions = {}) {
    super(message, options.cause === undefined ? undefined : { cause: options.cause });
    this.statusCode = options.statusCode;
    this.errors = options.errors;
  }
}

/** Missing, wrong or expired credentials. */
export class AuthenticationError extends APIError {}

/** The competition, user or file does not exist (or is not visible to you). */
export class NotFoundError extends APIError {}

/** Zindi asked us to slow down (HTTP 429) and retries did not help. */
export class RateLimitError extends APIError {}

/** The competition is private: joining it needs a secret code. */
export class SecretCodeRequiredError extends APIError {}

/** You are already a participant. `Client.join()` swallows this one and returns `false`. */
export class AlreadyJoinedError extends APIError {}

/** An operation needs a competition, but none was given or selected. */
export class CompetitionNotSetError extends ZindiError {
  constructor() {
    super(
      "No competition selected. Pass a competition id, call " +
        "`client.use('<competition-id>')`, set ZINDI_COMPETITION, " +
        "or run `zindi use <competition-id>`.",
    );
  }
}
