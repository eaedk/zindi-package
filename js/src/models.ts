/**
 * Typed views over Zindi API objects.
 *
 * Models are plain objects. Every model keeps the untouched API payload in `raw`, so nothing
 * the platform returns is ever out of reach, even fields this package does not know about yet.
 *
 * @module
 */

import { isObject, isTruthy, type JsonObject } from "./util.js";

const WEB_URL = "https://zindi.world";
const DAILY_LIMIT = /maximum of\s+(\d+)\s+submissions?\s+per\s+day/i;

// -- parsing helpers ---------------------------------------------------------------------------

/** Parse an ISO 8601 timestamp as returned by Zindi (`2026-11-18T10:02:11.000Z`). */
export function parseTime(value: unknown): Date | null {
  if (!value || typeof value !== "string") return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

function toInt(value: unknown): number | null {
  if (typeof value === "number") return Number.isFinite(value) ? Math.trunc(value) : null;
  if (typeof value === "boolean") return value ? 1 : 0;
  if (typeof value === "string" && /^\s*[+-]?\d+\s*$/.test(value)) return Number.parseInt(value, 10);
  return null;
}

function toFloat(value: unknown): number | null {
  if (typeof value === "number") return Number.isNaN(value) ? null : value;
  if (typeof value === "string" && value.trim() !== "") {
    const parsed = Number(value);
    return Number.isNaN(parsed) ? null : parsed;
  }
  return null;
}

function toStr(value: unknown): string | null {
  return value === null || value === undefined ? null : String(value);
}

function first(raw: JsonObject, ...keys: string[]): unknown {
  for (const key of keys) {
    if (raw[key] !== null && raw[key] !== undefined) return raw[key];
  }
  return undefined;
}

function objectOrNull(value: unknown): JsonObject | null {
  return isObject(value) ? value : null;
}

const ENTITIES: Record<string, string> = {
  amp: "&",
  lt: "<",
  gt: ">",
  quot: '"',
  apos: "'",
  nbsp: " ",
  ndash: "–",
  mdash: "—",
  hellip: "…",
  laquo: "«",
  raquo: "»",
  lsquo: "‘",
  rsquo: "’",
  ldquo: "“",
  rdquo: "”",
  eacute: "é",
  egrave: "è",
  agrave: "à",
  ccedil: "ç",
  copy: "©",
  reg: "®",
  deg: "°",
  times: "×",
  euro: "€",
};

function unescapeHtml(text: string): string {
  return text.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (match, entity: string) => {
    if (entity[0] === "#") {
      const hex = entity[1] === "x" || entity[1] === "X";
      const code = Number.parseInt(entity.slice(hex ? 2 : 1), hex ? 16 : 10);
      return Number.isFinite(code) && code <= 0x10ffff ? String.fromCodePoint(code) : match;
    }
    return ENTITIES[entity.toLowerCase()] ?? match;
  });
}

/** Very small HTML-to-text conversion, good enough for rules and descriptions. */
export function htmlToText(content: string | null | undefined): string {
  if (!content) return "";
  let text = content.replace(/<\s*(br|\/p|\/li|\/h\d|\/div)\s*\/?>/gi, "\n");
  text = text.replace(/<\s*li[^>]*>/gi, "• ");
  text = unescapeHtml(text.replace(/<[^>]+>/g, ""));
  return text.replace(/\n{3,}/g, "\n\n").trim();
}

/** A file size in human units, e.g. `1.0 MB`. */
export function humanSize(size: number | null): string {
  if (size === null) return "?";
  let value = size;
  for (const unit of ["B", "KB", "MB", "GB"]) {
    if (value < 1024 || unit === "GB") {
      return unit === "B" ? `${value.toFixed(0)} ${unit}` : `${value.toFixed(1)} ${unit}`;
    }
    value /= 1024;
  }
  return `${value.toFixed(1)} GB`;
}

// -- models ------------------------------------------------------------------------------------

/** A Zindi user profile. */
export interface User {
  /** The username, e.g. `amina_dev`. */
  username: string;
  /** First name, when public. */
  firstName: string | null;
  /** Last name, when public. */
  lastName: string | null;
  /** First and last name joined by a space (empty when neither is known). */
  fullName: string;
  /** Professional title. */
  title: string | null;
  /** Country name. */
  country: string | null;
  /** Two-letter country code (lower case). */
  countrycode: string | null;
  /** Zindi points. */
  points: number | null;
  /** Global rank. */
  rank: number | null;
  /** Best competition rank ever achieved. */
  bestRank: number | null;
  /** Number of competitions entered. */
  competitionsCount: number | null;
  /** Number of submissions made. */
  submissionsCount: number | null;
  /** Gold medals. */
  goldMedals: number;
  /** Silver medals. */
  silverMedals: number;
  /** Bronze medals. */
  bronzeMedals: number;
  /** When the account was created. */
  joinedAt: Date | null;
  /** The profile page on zindi.world. */
  url: string;
  /** The untouched API payload. */
  raw: JsonObject;
}

/** Build a {@link User} from an API payload. */
export function parseUser(raw: JsonObject): User {
  const firstName = toStr(raw.first_name);
  const lastName = toStr(raw.last_name);
  const username = toStr(raw.username) ?? "";
  return {
    username,
    firstName,
    lastName,
    fullName: [firstName, lastName].filter(Boolean).join(" "),
    title: toStr(raw.title),
    country: toStr(raw.country),
    countrycode: toStr(raw.countrycode),
    points: toInt(raw.points),
    rank: toInt(raw.rank),
    bestRank: toInt(raw.best_rank),
    competitionsCount: toInt(raw.count_of_competitions),
    submissionsCount: toInt(raw.count_of_submissions),
    goldMedals: toInt(raw.user_medals_summary_gold_count) ?? 0,
    silverMedals: toInt(raw.user_medals_summary_silver_count) ?? 0,
    bronzeMedals: toInt(raw.user_medals_summary_bronze_count) ?? 0,
    joinedAt: parseTime(raw.created_at),
    url: `${WEB_URL}/users/${username}`,
    raw: { ...raw },
  };
}

/** One file of a competition's dataset. */
export interface DataFile {
  /** File name, e.g. `Train.csv`. */
  filename: string;
  /** Size in bytes, when known. */
  size: number | null;
  /** Size in human units, e.g. `1.0 MB` (`?` when unknown). */
  sizeHuman: string;
  /** What the file contains. */
  description: string | null;
  /** `true` for the archive that bundles every other file. */
  isArchiveOfEverything: boolean;
  /** The untouched API payload. */
  raw: JsonObject;
}

/** Build a {@link DataFile} from an API payload. */
export function parseDataFile(raw: JsonObject): DataFile {
  const size = toInt(raw.filesize);
  return {
    filename: toStr(raw.filename) ?? "",
    size,
    sizeHuman: humanSize(size),
    description: toStr(raw.description),
    isArchiveOfEverything: isTruthy(raw.zip_of_everything),
    raw: { ...raw },
  };
}

/**
 * A Zindi competition or hackathon.
 *
 * Objects returned by `client.competition()` carry the full details (data files, rules,
 * metric). Objects in a `client.competitions()` listing carry a summary.
 */
export interface Competition {
  /** The competition id (its slug), e.g. `Crop-Yield-Forecast-Challenge`. */
  id: string;
  /** Title (falls back to the subtitle, then the id). */
  title: string;
  /** One-line summary. */
  subtitle: string | null;
  /** `competition` or `hackathon`. */
  kind: string | null;
  /** Human readable reward, e.g. `$10 000 USD`. */
  reward: string | null;
  /** `prize`, `points` or `knowledge`. */
  rewardType: string | null;
  /** Host organisation. */
  organization: string | null;
  /** `true` while the competition accepts submissions. */
  isOpen: boolean;
  /** `open` or `closed`. */
  status: "open" | "closed";
  /** Start date. */
  startTime: Date | null;
  /** End date. */
  endTime: Date | null;
  /** Number of participants. */
  participants: number | null;
  /** `true` for private competitions that need a secret code to join. */
  secretCodeRequired: boolean;
  /** `true` when the signed-in user has joined. */
  isParticipant: boolean;
  /** Evaluation metric, e.g. `RMSE`. */
  metric: string | null;
  /** Maximum team size. */
  maxTeamSize: number | null;
  /** Data files (details only). */
  dataFiles: DataFile[];
  /** Content pages as HTML, keyed by lower-case name (`description`, `rules`, ...). */
  pages: Record<string, string>;
  /** The description, as plain text. */
  description: string;
  /** The rules, as plain text. */
  rules: string;
  /** Maximum submissions per day, read from the rules (`null` if not stated). */
  dailySubmissionLimit: number | null;
  /** The competition page on zindi.world. */
  url: string;
  /** The untouched API payload. */
  raw: JsonObject;
}

/** Build a {@link Competition} from an API payload. */
export function parseCompetition(raw: JsonObject): Competition {
  const pages: Record<string, string> = {};
  for (const page of Array.isArray(raw.pages) ? raw.pages : []) {
    if (!isObject(page)) continue;
    const key = String(page.url_title || page.title || "").toLowerCase();
    if (key) pages[key] = String(page.content_html || "");
  }
  const rulesObject = objectOrNull(raw.rules);
  if (rulesObject && rulesObject.content_html && !("rules" in pages)) {
    pages.rules = String(rulesObject.content_html);
  }
  const id = toStr(raw.id) ?? "";
  const isOpen = isTruthy(raw.open);
  const rulesText = htmlToText(pages.rules);
  const limit = DAILY_LIMIT.exec(rulesText);
  return {
    id,
    title: String(first(raw, "title", "subtitle", "id") || ""),
    subtitle: toStr(raw.subtitle),
    kind: toStr(raw.kind),
    reward: toStr(raw.reward),
    rewardType: toStr(raw.reward_type),
    organization: toStr(raw.organization),
    isOpen,
    status: isOpen ? "open" : "closed",
    startTime: parseTime(raw.start_time),
    endTime: parseTime(raw.end_time),
    participants: toInt(raw.participations_count),
    secretCodeRequired: isTruthy(raw.secret_code_required),
    isParticipant: isTruthy(raw.is_participant),
    metric: toStr(first(raw, "error_metric")),
    maxTeamSize: toInt(raw.max_number_of_participants_per_team),
    dataFiles: (Array.isArray(raw.datafiles) ? raw.datafiles : []).filter(isObject).map(parseDataFile),
    pages,
    description: htmlToText(pages.description || Object.values(pages)[0] || ""),
    rules: rulesText,
    dailySubmissionLimit: limit ? Number.parseInt(limit[1] ?? "", 10) : null,
    url: `${WEB_URL}/competitions/${id}`,
    raw: { ...raw },
  };
}

/** One row of a leaderboard (a user or a team). */
export interface LeaderboardEntry {
  /** Rank (private once revealed, public before); `null` without a valid score. */
  rank: number | null;
  /** Team title or username. */
  name: string;
  /** Best score (private once revealed, public before). */
  score: number | null;
  /** `true` when the row is a team. */
  isTeam: boolean;
  /** The username, for a solo participant. */
  username: string | null;
  /** The team id, for a team. */
  teamId: string | null;
  /** Usernames of the team members. */
  members: string[];
  /** Number of submissions. */
  submissionCount: number;
  /** When the best submission was made. */
  lastSubmissionAt: Date | null;
  /** Two-letter country code. */
  countrycode: string | null;
  /** The untouched API payload. */
  raw: JsonObject;
}

/** Build a {@link LeaderboardEntry} from an API payload. */
export function parseLeaderboardEntry(raw: JsonObject): LeaderboardEntry {
  const user = objectOrNull(raw.user);
  const team = objectOrNull(raw.team);
  const members = (team && Array.isArray(team.members) ? team.members : [])
    .filter(isObject)
    .filter((member) => member.username)
    .map((member) => String(member.username));
  let country: unknown = user?.countrycode;
  if (!country && team && Array.isArray(team.countrycodes) && team.countrycodes.length > 0) {
    country = team.countrycodes[0];
  }
  return {
    rank: toInt(first(raw, "private_rank", "public_rank")),
    name: String(team?.title || user?.username || "?"),
    score: toFloat(first(raw, "best_private_score", "best_public_score")),
    isTeam: team !== null,
    username: toStr(user?.username),
    teamId: toStr(team?.id),
    members,
    submissionCount: toInt(raw.submission_count) ?? 0,
    lastSubmissionAt: parseTime(first(raw, "best_private_submitted_at", "best_public_submitted_at")),
    countrycode: toStr(country),
    raw: { ...raw },
  };
}

/** `true` if `username` is the entry's user or a member of the entry's team (case-insensitive). */
export function entryIncludes(entry: LeaderboardEntry, username: string): boolean {
  const wanted = username.toLowerCase();
  return (
    (entry.username ?? "").toLowerCase() === wanted ||
    entry.members.some((member) => member.toLowerCase() === wanted)
  );
}

/** Your own standing in a competition. */
export interface Participation {
  /** Your rank; `null` without a valid score. */
  rank: number | null;
  /** Your best score. */
  score: number | null;
  /** Number of submissions. */
  submissionCount: number;
  /** Your team, if you are in one. */
  teamTitle: string | null;
  /** The untouched API payload. */
  raw: JsonObject;
}

/** Build a {@link Participation} from an API payload. */
export function parseParticipation(raw: JsonObject): Participation {
  const team = objectOrNull(raw.team);
  return {
    rank: toInt(first(raw, "private_rank", "public_rank")),
    score: toFloat(first(raw, "best_private_score", "best_public_score")),
    submissionCount: toInt(raw.submission_count) ?? 0,
    teamTitle: toStr(team?.title),
    raw: { ...raw },
  };
}

/** One of your submissions. */
export interface Submission {
  /** Submission id. */
  id: string;
  /** Raw status: `initial` (being scored), `successful`, or a failure status. */
  status: string;
  /** Name of the uploaded file. */
  filename: string | null;
  /** Your comment. */
  comment: string | null;
  /** When it was submitted. */
  createdAt: Date | null;
  /** Public leaderboard score. */
  publicScore: number | null;
  /** Private leaderboard score (once revealed). */
  privateScore: number | null;
  /** Error message for a failed submission. */
  statusDescription: string | null;
  /** `true` when selected for the private leaderboard. */
  selected: boolean;
  /** `true` when scoring failed. */
  isFailed: boolean;
  /** `true` when a score is available. */
  isScored: boolean;
  /** `true` while Zindi is still scoring. */
  isPending: boolean;
  /** Private score when available, else public score. */
  score: number | null;
  /** `failed`, `scored` or `scoring…`. */
  statusLabel: string;
  /** The comment, or the error message for a failed submission. */
  note: string;
  /** The untouched API payload. */
  raw: JsonObject;
}

/** Build a {@link Submission} from an API payload. */
export function parseSubmission(raw: JsonObject): Submission {
  const status = String(raw.status || "unknown");
  const publicScore = toFloat(raw.public_score);
  const privateScore = toFloat(raw.private_score);
  const comment = toStr(raw.comment);
  const statusDescription = toStr(raw.status_description);
  const isFailed = status !== "successful" && status !== "initial";
  const isScored = !isFailed && (publicScore !== null || privateScore !== null);
  const isPending = !isFailed && !isScored;
  return {
    id: toStr(raw.id) ?? "",
    status,
    filename: toStr(raw.filename),
    comment,
    createdAt: parseTime(raw.created_at),
    publicScore,
    privateScore,
    statusDescription,
    selected: isTruthy(raw.chosen ?? raw.selected),
    isFailed,
    isScored,
    isPending,
    score: privateScore ?? publicScore,
    statusLabel: isFailed ? "failed" : isScored ? "scored" : "scoring…",
    note: (isFailed ? statusDescription : comment) || "",
    raw: { ...raw },
  };
}

/** How many submissions you can still make. */
export interface SubmissionLimits {
  /** Submissions left today, when Zindi says. */
  remainingToday: number | null;
  /** The untouched API payload. */
  raw: JsonObject;
}

/** Build {@link SubmissionLimits} from an API payload. */
export function parseSubmissionLimits(raw: JsonObject): SubmissionLimits {
  return { remainingToday: toInt(first(raw, "today", "remaining_today")), raw: { ...raw } };
}

/** Your team in a competition. */
export interface Team {
  /** Team id. */
  id: string | null;
  /** Team name. */
  title: string;
  /** Username of the team leader. */
  leader: string | null;
  /** Usernames of the members. */
  members: string[];
  /** Usernames with a pending invitation. */
  pendingInvitations: string[];
  /** The untouched API payload. */
  raw: JsonObject;
}

function usernames(items: unknown): string[] {
  const out: string[] = [];
  for (const item of Array.isArray(items) ? items : []) {
    if (isObject(item) && item.username) out.push(String(item.username));
    else if (typeof item === "string") out.push(item);
  }
  return out;
}

/** Build a {@link Team} from an API payload. */
export function parseTeam(raw: JsonObject): Team {
  const leader = raw.leader;
  return {
    id: toStr(raw.id),
    title: String(raw.title || ""),
    leader: toStr(raw.leader_username ?? (isObject(leader) ? leader.username : leader)),
    members: usernames(raw.users ?? raw.members),
    pendingInvitations: usernames(raw.invited_users ?? raw.invitations),
    raw: { ...raw },
  };
}

/** A discussion thread. */
export interface Discussion {
  /** Thread id. */
  id: string;
  /** Thread title. */
  title: string;
  /** Username of the author. */
  author: string | null;
  /** When it was published. */
  createdAt: Date | null;
  /** Number of replies. */
  commentsCount: number;
  /** `true` when pinned by the hosts. */
  pinned: boolean;
  /** The competition the thread belongs to. */
  competition: string | null;
  /** The thread page on zindi.world. */
  url: string;
  /** The untouched API payload. */
  raw: JsonObject;
}

/** Build a {@link Discussion} from an API payload. */
export function parseDiscussion(raw: JsonObject, competition: string | null = null): Discussion {
  const user = objectOrNull(raw.user);
  const id = toStr(raw.id) ?? "";
  const slug = raw.url_title;
  let url = `${WEB_URL}/discussions/${id}`;
  if (competition && slug) url = `${WEB_URL}/competitions/${competition}/discussions/${id}/${String(slug)}`;
  else if (competition) url = `${WEB_URL}/competitions/${competition}/discussions/${id}`;
  return {
    id,
    title: String(raw.title || ""),
    author: toStr(user?.username),
    createdAt: parseTime(first(raw, "published_at", "created_at")),
    commentsCount: toInt(raw.comments_count) ?? 0,
    pinned: isTruthy(raw.pinned),
    competition,
    url,
    raw: { ...raw },
  };
}
