//! Typed views over Zindi API objects.
//!
//! Every model is built tolerantly from a [`serde_json::Value`] with its `from_json`
//! constructor: missing or oddly typed fields never fail, they just stay empty. Every model
//! also keeps the untouched API payload in `raw`, so nothing the platform returns is ever
//! out of reach, even fields this crate does not know about yet.

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Root of the Zindi website, used to build links.
pub const WEB_URL: &str = "https://zindi.world";

// -- parsing helpers -------------------------------------------------------------------------

/// The value at `key`, unless it is missing or `null`.
fn get<'a>(raw: &'a Value, key: &str) -> Option<&'a Value> {
    raw.get(key).filter(|value| !value.is_null())
}

/// The first of `keys` that holds a non-null value.
fn first<'a>(raw: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| get(raw, key))
}

fn text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn int(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|f| f.trunc() as i64)),
        Value::String(text) => text.trim().parse().ok(),
        Value::Bool(flag) => Some(i64::from(*flag)),
        _ => None,
    }
}

fn uint<T: TryFrom<i64>>(value: Option<&Value>) -> Option<T> {
    int(value).and_then(|n| T::try_from(n).ok())
}

fn float(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// Truthiness, the way the platform's JavaScript front end sees it.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(map)) => !map.is_empty(),
    }
}

/// Parse an ISO 8601 timestamp as returned by Zindi (`2026-11-18T10:02:11.000Z`).
///
/// Timestamps without an offset are taken to be UTC. Anything unparsable gives `None`.
pub fn parse_time(value: &Value) -> Option<DateTime<Utc>> {
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(parsed) = DateTime::parse_from_rfc3339(text) {
        return Some(parsed.with_timezone(&Utc));
    }
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"]
        .iter()
        .find_map(|format| NaiveDateTime::parse_from_str(text, format).ok())
        .map(|naive| naive.and_utc())
}

fn time(value: Option<&Value>) -> Option<DateTime<Utc>> {
    value.and_then(parse_time)
}

/// Usernames found in a list of `{"username": ...}` objects (or plain strings).
fn usernames(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| match item {
                    Value::String(name) => Some(name.clone()),
                    other => text(get(other, "username")),
                })
                .filter(|name| !name.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Very small HTML-to-text conversion, good enough for rules and descriptions.
pub fn html_to_text(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('>') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let tag = after[..end].trim().to_ascii_lowercase();
        let closing = tag.starts_with('/');
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        let breaks_line = matches!(
            name.as_str(),
            "p" | "li" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        );
        if name == "br" || (closing && breaks_line) {
            out.push('\n');
        } else if !closing && name == "li" {
            out.push_str("• ");
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    let text = unescape_entities(&out);

    // Collapse runs of three or more line breaks into one blank line.
    let mut collapsed = String::with_capacity(text.len());
    let mut newlines = 0;
    for c in text.chars() {
        if c == '\n' {
            newlines += 1;
            if newlines <= 2 {
                collapsed.push(c);
            }
        } else {
            newlines = 0;
            collapsed.push(c);
        }
    }
    collapsed.trim().to_owned()
}

fn unescape_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let decoded = after.find(';').filter(|&end| end <= 10).and_then(|end| {
            let entity = &after[..end];
            let c = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ => entity.strip_prefix('#').and_then(|code| {
                    let number = match code.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => code.parse().ok(),
                    };
                    number.and_then(char::from_u32)
                }),
            };
            c.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Find "maximum of N submission(s) per day" (case-insensitive) and return N.
fn find_daily_limit(text: &str) -> Option<u32> {
    fn spaces(s: &str) -> Option<&str> {
        let trimmed = s.trim_start();
        (trimmed.len() < s.len()).then_some(trimmed)
    }
    let lowered = text.to_lowercase();
    let mut haystack = lowered.as_str();
    while let Some(position) = haystack.find("maximum of") {
        let candidate = &haystack[position + "maximum of".len()..];
        haystack = candidate;
        let parsed = (|| {
            let rest = spaces(candidate)?;
            let digits_len = rest.chars().take_while(char::is_ascii_digit).count();
            if digits_len == 0 {
                return None;
            }
            let number = rest[..digits_len].parse().ok()?;
            let rest = spaces(&rest[digits_len..])?.strip_prefix("submission")?;
            let rest = rest.strip_prefix('s').unwrap_or(rest);
            let rest = spaces(rest)?.strip_prefix("per")?;
            spaces(rest)?.starts_with("day").then_some(number)
        })();
        if parsed.is_some() {
            return parsed;
        }
    }
    None
}

// -- models ----------------------------------------------------------------------------------

/// A Zindi user profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct User {
    /// Username (also the profile URL slug).
    pub username: String,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Professional title, e.g. "Data scientist".
    pub title: Option<String>,
    /// Country name.
    pub country: Option<String>,
    /// Two-letter country code.
    pub countrycode: Option<String>,
    /// Zindi points.
    pub points: Option<i64>,
    /// Global rank.
    pub rank: Option<u32>,
    /// Best global rank ever reached.
    pub best_rank: Option<u32>,
    /// Number of competitions entered.
    pub competitions_count: Option<u32>,
    /// Number of submissions made.
    pub submissions_count: Option<u32>,
    /// Gold medals.
    pub gold_medals: u32,
    /// Silver medals.
    pub silver_medals: u32,
    /// Bronze medals.
    pub bronze_medals: u32,
    /// When the account was created.
    pub joined_at: Option<DateTime<Utc>>,
    /// The untouched API payload.
    pub raw: Value,
}

impl User {
    /// Build a user from an API object.
    pub fn from_json(raw: &Value) -> Self {
        Self {
            username: text(get(raw, "username")).unwrap_or_default(),
            first_name: text(get(raw, "first_name")),
            last_name: text(get(raw, "last_name")),
            title: text(get(raw, "title")),
            country: text(get(raw, "country")),
            countrycode: text(get(raw, "countrycode")),
            points: int(get(raw, "points")),
            rank: uint(get(raw, "rank")),
            best_rank: uint(get(raw, "best_rank")),
            competitions_count: uint(get(raw, "count_of_competitions")),
            submissions_count: uint(get(raw, "count_of_submissions")),
            gold_medals: uint(get(raw, "user_medals_summary_gold_count")).unwrap_or(0),
            silver_medals: uint(get(raw, "user_medals_summary_silver_count")).unwrap_or(0),
            bronze_medals: uint(get(raw, "user_medals_summary_bronze_count")).unwrap_or(0),
            joined_at: time(get(raw, "created_at")),
            raw: raw.clone(),
        }
    }

    /// First and last name, separated by a space (empty if both are unknown).
    pub fn full_name(&self) -> String {
        [&self.first_name, &self.last_name]
            .into_iter()
            .flatten()
            .filter(|part| !part.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Link to the profile on the Zindi website.
    pub fn url(&self) -> String {
        format!("{WEB_URL}/users/{}", self.username)
    }
}

/// One file of a competition's dataset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataFile {
    /// File name, as used in the download URL.
    pub filename: String,
    /// Size in bytes, when Zindi tells it.
    pub size: Option<u64>,
    /// What the file contains.
    pub description: Option<String>,
    /// `true` for the "every file in one zip" archive.
    pub is_archive_of_everything: bool,
    /// The untouched API payload.
    pub raw: Value,
}

impl DataFile {
    /// Build a data file from an API object.
    pub fn from_json(raw: &Value) -> Self {
        Self {
            filename: text(get(raw, "filename")).unwrap_or_default(),
            size: uint(get(raw, "filesize")),
            description: text(get(raw, "description")),
            is_archive_of_everything: truthy(get(raw, "zip_of_everything")),
            raw: raw.clone(),
        }
    }

    /// The size in a human friendly unit, e.g. `"1.0 MB"` (`"?"` when unknown).
    pub fn size_human(&self) -> String {
        let Some(bytes) = self.size else {
            return "?".to_owned();
        };
        if bytes < 1024 {
            return format!("{bytes} B");
        }
        let mut size = bytes as f64 / 1024.0;
        for unit in ["KB", "MB"] {
            if size < 1024.0 {
                return format!("{size:.1} {unit}");
            }
            size /= 1024.0;
        }
        format!("{size:.1} GB")
    }
}

/// A Zindi competition or hackathon.
///
/// Values returned by [`Client::competition`](crate::Client::competition) carry the full
/// details (data files, rules, metric). Values in a
/// [`Client::competitions`](crate::Client::competitions) listing carry a summary.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Competition {
    /// The competition id (its URL slug).
    pub id: String,
    /// Title.
    pub title: String,
    /// One-line summary.
    pub subtitle: Option<String>,
    /// `"competition"` or `"hackathon"`.
    pub kind: Option<String>,
    /// Human readable reward, e.g. `"$10 000 USD"`.
    pub reward: Option<String>,
    /// `"prize"`, `"points"` or `"knowledge"`.
    pub reward_type: Option<String>,
    /// Host organisation.
    pub organization: Option<String>,
    /// `true` while the competition accepts submissions.
    pub is_open: bool,
    /// Start of the competition.
    pub start_time: Option<DateTime<Utc>>,
    /// End of the competition.
    pub end_time: Option<DateTime<Utc>>,
    /// Number of participants.
    pub participants: Option<u32>,
    /// `true` for private competitions that need a secret code to join.
    pub secret_code_required: bool,
    /// `true` when you have joined.
    pub is_participant: bool,
    /// Evaluation metric, e.g. `"RMSE"` (details only).
    pub metric: Option<String>,
    /// Maximum team size (details only).
    pub max_team_size: Option<u32>,
    /// Data files (details only).
    pub data_files: Vec<DataFile>,
    /// Content pages as `(key, html)` pairs, e.g. `("rules", "<ul>...</ul>")`.
    pub pages: Vec<(String, String)>,
    /// The untouched API payload.
    pub raw: Value,
}

impl Competition {
    /// Build a competition from an API object.
    pub fn from_json(raw: &Value) -> Self {
        let mut pages: Vec<(String, String)> = Vec::new();
        for page in get(raw, "pages")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let key = text(first(page, &["url_title", "title"]))
                .unwrap_or_default()
                .to_lowercase();
            if !key.is_empty() {
                let html = text(get(page, "content_html")).unwrap_or_default();
                pages.push((key, html));
            }
        }
        if let Some(rules) = get(raw, "rules").and_then(|rules| text(get(rules, "content_html"))) {
            if !rules.is_empty() && !pages.iter().any(|(key, _)| key == "rules") {
                pages.push(("rules".to_owned(), rules));
            }
        }
        Self {
            id: text(get(raw, "id")).unwrap_or_default(),
            title: text(first(raw, &["title", "subtitle", "id"])).unwrap_or_default(),
            subtitle: text(get(raw, "subtitle")),
            kind: text(get(raw, "kind")),
            reward: text(get(raw, "reward")),
            reward_type: text(get(raw, "reward_type")),
            organization: text(get(raw, "organization")),
            is_open: truthy(get(raw, "open")),
            start_time: time(get(raw, "start_time")),
            end_time: time(get(raw, "end_time")),
            participants: uint(get(raw, "participations_count")),
            secret_code_required: truthy(get(raw, "secret_code_required")),
            is_participant: truthy(get(raw, "is_participant")),
            metric: text(get(raw, "error_metric")),
            max_team_size: uint(get(raw, "max_number_of_participants_per_team")),
            data_files: get(raw, "datafiles")
                .and_then(Value::as_array)
                .map(|files| {
                    files
                        .iter()
                        .filter(|file| file.is_object())
                        .map(DataFile::from_json)
                        .collect()
                })
                .unwrap_or_default(),
            pages,
            raw: raw.clone(),
        }
    }

    /// Link to the competition on the Zindi website.
    pub fn url(&self) -> String {
        format!("{WEB_URL}/competitions/{}", self.id)
    }

    /// `"open"` or `"closed"`.
    pub fn status(&self) -> &'static str {
        if self.is_open {
            "open"
        } else {
            "closed"
        }
    }

    /// The HTML of a content page (`"description"`, `"rules"`, ...), if there is one.
    pub fn page(&self, key: &str) -> Option<&str> {
        self.pages
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, html)| html.as_str())
    }

    /// The competition description, as plain text.
    pub fn description(&self) -> String {
        let html = self
            .page("description")
            .or_else(|| self.pages.first().map(|(_, html)| html.as_str()))
            .unwrap_or_default();
        html_to_text(html)
    }

    /// The competition rules, as plain text.
    pub fn rules(&self) -> String {
        html_to_text(self.page("rules").unwrap_or_default())
    }

    /// Maximum submissions per day, read from the rules (`None` if not stated).
    pub fn daily_submission_limit(&self) -> Option<u32> {
        find_daily_limit(&self.rules())
    }
}

/// One row of a leaderboard (a user or a team).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LeaderboardEntry {
    /// Rank (private once revealed, public before); `None` without a valid score.
    pub rank: Option<u32>,
    /// Team title or username.
    pub name: String,
    /// Best score (private once revealed, public before).
    pub score: Option<f64>,
    /// `true` when this row is a team.
    pub is_team: bool,
    /// Username, for a solo participant.
    pub username: Option<String>,
    /// Team id, for a team.
    pub team_id: Option<String>,
    /// Team members' usernames.
    pub members: Vec<String>,
    /// Number of submissions.
    pub submission_count: u32,
    /// When the best submission was made.
    pub last_submission_at: Option<DateTime<Utc>>,
    /// Two-letter country code.
    pub countrycode: Option<String>,
    /// The untouched API payload.
    pub raw: Value,
}

impl LeaderboardEntry {
    /// Build a leaderboard row from an API object.
    pub fn from_json(raw: &Value) -> Self {
        let user = get(raw, "user").filter(|v| v.is_object());
        let team = get(raw, "team").filter(|v| v.is_object());
        let mut countrycode = user.and_then(|user| text(get(user, "countrycode")));
        if countrycode.as_deref().is_none_or(str::is_empty) {
            countrycode = team
                .and_then(|team| get(team, "countrycodes"))
                .and_then(|codes| text(codes.get(0)))
                .or(countrycode);
        }
        let name = team
            .and_then(|team| text(get(team, "title")))
            .filter(|name| !name.is_empty())
            .or_else(|| user.and_then(|user| text(get(user, "username"))))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "?".to_owned());
        Self {
            rank: uint(first(raw, &["private_rank", "public_rank"])),
            name,
            score: float(first(raw, &["best_private_score", "best_public_score"])),
            is_team: team.is_some(),
            username: user.and_then(|user| text(get(user, "username"))),
            team_id: team.and_then(|team| text(get(team, "id"))),
            members: usernames(team.and_then(|team| get(team, "members"))),
            submission_count: uint(get(raw, "submission_count")).unwrap_or(0),
            last_submission_at: time(first(
                raw,
                &["best_private_submitted_at", "best_public_submitted_at"],
            )),
            countrycode,
            raw: raw.clone(),
        }
    }

    /// The name, with `" (team)"` appended for teams.
    pub fn display_name(&self) -> String {
        if self.is_team {
            format!("{} (team)", self.name)
        } else {
            self.name.clone()
        }
    }

    /// `true` if `username` is this row's user or a member of this row's team
    /// (case-insensitive).
    pub fn includes(&self, username: &str) -> bool {
        let wanted = username.to_lowercase();
        self.username
            .iter()
            .chain(&self.members)
            .any(|name| name.to_lowercase() == wanted)
    }
}

/// Your own standing in a competition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Participation {
    /// Your rank; `None` without a valid score.
    pub rank: Option<u32>,
    /// Your best score.
    pub score: Option<f64>,
    /// Number of submissions.
    pub submission_count: u32,
    /// Your team's title, if you are in a team.
    pub team_title: Option<String>,
    /// The untouched API payload.
    pub raw: Value,
}

impl Participation {
    /// Build a participation from an API object.
    pub fn from_json(raw: &Value) -> Self {
        Self {
            rank: uint(first(raw, &["private_rank", "public_rank"])),
            score: float(first(raw, &["best_private_score", "best_public_score"])),
            submission_count: uint(get(raw, "submission_count")).unwrap_or(0),
            team_title: get(raw, "team").and_then(|team| text(get(team, "title"))),
            raw: raw.clone(),
        }
    }
}

/// One of your submissions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Submission {
    /// Submission id.
    pub id: String,
    /// Raw status: `"initial"` (being scored), `"successful"` or a failure status.
    pub status: String,
    /// Name of the uploaded file.
    pub filename: Option<String>,
    /// Your comment.
    pub comment: Option<String>,
    /// When the submission was made.
    pub created_at: Option<DateTime<Utc>>,
    /// Public leaderboard score.
    pub public_score: Option<f64>,
    /// Private leaderboard score (once revealed).
    pub private_score: Option<f64>,
    /// Error message of a failed submission.
    pub status_description: Option<String>,
    /// `true` if selected for the private leaderboard.
    pub selected: bool,
    /// The untouched API payload.
    pub raw: Value,
}

impl Submission {
    /// Build a submission from an API object.
    pub fn from_json(raw: &Value) -> Self {
        Self {
            id: text(get(raw, "id")).unwrap_or_default(),
            status: text(get(raw, "status"))
                .filter(|status| !status.is_empty())
                .unwrap_or_else(|| "unknown".to_owned()),
            filename: text(get(raw, "filename")),
            comment: text(get(raw, "comment")),
            created_at: time(get(raw, "created_at")),
            public_score: float(get(raw, "public_score")),
            private_score: float(get(raw, "private_score")),
            status_description: text(get(raw, "status_description")),
            selected: truthy(get(raw, "chosen").or_else(|| get(raw, "selected"))),
            raw: raw.clone(),
        }
    }

    /// `true` if scoring failed (any status other than `initial` and `successful`).
    pub fn is_failed(&self) -> bool {
        !matches!(self.status.as_str(), "successful" | "initial")
    }

    /// `true` once the submission has a score.
    pub fn is_scored(&self) -> bool {
        !self.is_failed() && (self.public_score.is_some() || self.private_score.is_some())
    }

    /// `true` while the submission is waiting to be scored.
    pub fn is_pending(&self) -> bool {
        !self.is_failed() && !self.is_scored()
    }

    /// The private score when revealed, else the public score.
    pub fn score(&self) -> Option<f64> {
        self.private_score.or(self.public_score)
    }

    /// `"failed"`, `"scored"` or `"scoring…"`.
    pub fn status_label(&self) -> &'static str {
        if self.is_failed() {
            "failed"
        } else if self.is_scored() {
            "scored"
        } else {
            "scoring…"
        }
    }

    /// The comment, or the error message for a failed submission.
    pub fn note(&self) -> &str {
        let note = if self.is_failed() {
            &self.status_description
        } else {
            &self.comment
        };
        note.as_deref().unwrap_or_default()
    }
}

/// How many submissions you can still make.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubmissionLimits {
    /// Submissions left today.
    pub remaining_today: Option<u32>,
    /// The untouched API payload.
    pub raw: Value,
}

impl SubmissionLimits {
    /// Build submission limits from an API object.
    pub fn from_json(raw: &Value) -> Self {
        Self {
            remaining_today: uint(first(raw, &["today", "remaining_today"])),
            raw: raw.clone(),
        }
    }
}

/// Your team in a competition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Team {
    /// Team id.
    pub id: Option<String>,
    /// Team title.
    pub title: String,
    /// Username of the team leader.
    pub leader: Option<String>,
    /// Usernames of the members.
    pub members: Vec<String>,
    /// Usernames with a pending invitation.
    pub pending_invitations: Vec<String>,
    /// The untouched API payload.
    pub raw: Value,
}

impl Team {
    /// Build a team from an API object.
    pub fn from_json(raw: &Value) -> Self {
        let leader = text(get(raw, "leader_username")).or_else(|| {
            get(raw, "leader").and_then(|leader| match leader {
                Value::Object(_) => text(get(leader, "username")),
                other => text(Some(other)),
            })
        });
        Self {
            id: text(get(raw, "id")),
            title: text(get(raw, "title")).unwrap_or_default(),
            leader,
            members: usernames(get(raw, "users").or_else(|| get(raw, "members"))),
            pending_invitations: usernames(
                get(raw, "invited_users").or_else(|| get(raw, "invitations")),
            ),
            raw: raw.clone(),
        }
    }
}

/// A discussion thread.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Discussion {
    /// Thread id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Username of the author.
    pub author: Option<String>,
    /// When the thread was published.
    pub created_at: Option<DateTime<Utc>>,
    /// Number of replies.
    pub comments_count: u32,
    /// `true` if pinned by the hosts.
    pub pinned: bool,
    /// The competition the thread belongs to.
    pub competition: Option<String>,
    /// The untouched API payload.
    pub raw: Value,
}

impl Discussion {
    /// Build a discussion from an API object, for the given competition.
    pub fn from_json(raw: &Value, competition: Option<&str>) -> Self {
        Self {
            id: text(get(raw, "id")).unwrap_or_default(),
            title: text(get(raw, "title")).unwrap_or_default(),
            author: get(raw, "user").and_then(|user| text(get(user, "username"))),
            created_at: time(first(raw, &["published_at", "created_at"])),
            comments_count: uint(get(raw, "comments_count")).unwrap_or(0),
            pinned: truthy(get(raw, "pinned")),
            competition: competition.map(str::to_owned),
            raw: raw.clone(),
        }
    }

    /// Link to the thread on the Zindi website.
    pub fn url(&self) -> String {
        let slug = text(get(&self.raw, "url_title")).filter(|slug| !slug.is_empty());
        match (&self.competition, slug) {
            (Some(comp), Some(slug)) => {
                format!(
                    "{WEB_URL}/competitions/{comp}/discussions/{}/{slug}",
                    self.id
                )
            }
            (Some(comp), None) => format!("{WEB_URL}/competitions/{comp}/discussions/{}", self.id),
            (None, _) => format!("{WEB_URL}/discussions/{}", self.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_fields_never_fail() {
        let empty = json!({});
        assert_eq!(User::from_json(&empty).username, "");
        assert_eq!(Competition::from_json(&json!(null)).id, "");
        assert_eq!(LeaderboardEntry::from_json(&empty).name, "?");
        assert_eq!(Submission::from_json(&empty).status, "unknown");
        assert!(Team::from_json(&json!([1, 2])).members.is_empty());
        assert_eq!(SubmissionLimits::from_json(&empty).remaining_today, None);
    }

    #[test]
    fn values_are_coerced_tolerantly() {
        let user = User::from_json(&json!({"username": 42, "rank": "812", "points": 3.9}));
        assert_eq!(user.username, "42");
        assert_eq!(user.rank, Some(812));
        assert_eq!(user.points, Some(3));
        let entry =
            LeaderboardEntry::from_json(&json!({"public_rank": -1, "best_public_score": "0.5"}));
        assert_eq!(entry.rank, None);
        assert_eq!(entry.score, Some(0.5));
    }

    #[test]
    fn timestamps() {
        let t = parse_time(&json!("2026-11-18T10:02:11.000Z")).unwrap();
        assert_eq!(t.to_rfc3339(), "2026-11-18T10:02:11+00:00");
        assert!(parse_time(&json!("2026-11-18T10:02:11")).is_some());
        assert!(parse_time(&json!("yesterday")).is_none());
        assert!(parse_time(&json!(12)).is_none());
    }

    #[test]
    fn html_conversion() {
        let html =
            "<ul><li>One &amp; two</li><li>Max&#32;3</li></ul><p>a<br/>b</p>\n\n\n\nend &bogus";
        assert_eq!(
            html_to_text(html),
            "• One & two\n• Max 3\na\nb\n\nend &bogus"
        );
        assert_eq!(html_to_text("1 < 2"), "1 < 2");
    }

    #[test]
    fn daily_limit() {
        assert_eq!(
            find_daily_limit("You may make a MAXIMUM OF 10 submissions per day."),
            Some(10)
        );
        assert_eq!(
            find_daily_limit("maximum of 1 submission  per\nday"),
            Some(1)
        );
        assert_eq!(find_daily_limit("maximum of ten submissions per day"), None);
        assert_eq!(
            find_daily_limit("maximum of 3 teams; maximum of 5 submissions per day"),
            Some(5)
        );
    }

    #[test]
    fn sizes() {
        let file = |size| DataFile {
            size,
            ..DataFile::default()
        };
        assert_eq!(file(None).size_human(), "?");
        assert_eq!(file(Some(512)).size_human(), "512 B");
        assert_eq!(file(Some(2048)).size_human(), "2.0 KB");
        assert_eq!(file(Some(1_048_576)).size_human(), "1.0 MB");
        assert_eq!(file(Some(5 * 1024 * 1024 * 1024)).size_human(), "5.0 GB");
    }
}
