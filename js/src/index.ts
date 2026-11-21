/**
 * Zindi for JavaScript and TypeScript: join competitions, download data, submit and climb the
 * leaderboard of https://zindi.world.
 *
 * @example
 * ```ts
 * import { Client } from "zindi";
 *
 * const zindi = new Client();
 * for (const comp of await zindi.competitions()) console.log(comp.id, comp.reward);
 * ```
 *
 * @packageDocumentation
 */

export {
  Client,
  type ClientOptions,
  type CompetitionId,
  type CompetitionKind,
  type CompetitionOptions,
  type CompetitionsOptions,
  type CompetitionStatus,
  type CreateTeamOptions,
  type DiscussionsOptions,
  type DownloadOptions,
  type JoinOptions,
  type LeaderboardOptions,
  type LoginOptions,
  type LogoutOptions,
  type RewardType,
  type SubmissionsOptions,
  type SubmitOptions,
  type UseOptions,
  type WaitOptions,
} from "./client.js";
export {
  AlreadyJoinedError,
  APIError,
  type APIErrorOptions,
  AuthenticationError,
  CompetitionNotSetError,
  NotFoundError,
  PASSWORD_RESET_URL,
  RateLimitError,
  SecretCodeRequiredError,
  ZindiError,
} from "./errors.js";
export {
  DEFAULT_BASE_URL,
  errorMessage,
  type FetchLike,
  type SleepFn,
  USER_AGENT,
  WEB_URL,
} from "./http.js";
export {
  type Competition,
  type DataFile,
  type Discussion,
  entryIncludes,
  htmlToText,
  type LeaderboardEntry,
  parseCompetition,
  parseDataFile,
  parseDiscussion,
  parseLeaderboardEntry,
  parseParticipation,
  parseSubmission,
  parseSubmissionLimits,
  parseTeam,
  parseTime,
  parseUser,
  type Participation,
  type Submission,
  type SubmissionLimits,
  type Team,
  type User,
} from "./models.js";
export { type Credentials, resolveCredentials, type ResolveOptions } from "./auth.js";
export {
  configDir,
  credentialsPath,
  deleteCredentials,
  findProjectFile,
  loadCredentials,
  PROJECT_FILE,
  readProjectCompetition,
  saveCredentials,
  type SavedCredentials,
  writeProjectCompetition,
} from "./config.js";
export type { JsonObject } from "./util.js";
export { VERSION } from "./version.js";
