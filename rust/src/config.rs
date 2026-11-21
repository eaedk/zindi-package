//! Where zindi keeps its files, and how it reads them.
//!
//! Two files matter:
//!
//! * **Credentials**: `<config dir>/credentials.json` holds your username and an auth
//!   token (never your password). It is shared by every zindi client (Python, R,
//!   JavaScript, Rust), so logging in once is enough.
//! * **Project file**: `zindi.toml` in your project folder (or any parent folder)
//!   remembers the competition you work on, so calls need no competition id.
//!
//! The config dir is, in order of priority: `$ZINDI_CONFIG_DIR`; `%APPDATA%\zindi` on
//! Windows; `$XDG_CONFIG_HOME/zindi`; `~/.config/zindi`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{SecondsFormat, Utc};
use serde_json::{json, Value};

use crate::error::{Error, Result};

/// Name of the project file that remembers the current competition.
pub const PROJECT_FILE: &str = "zindi.toml";

/// Name of the shared credentials file inside the config dir.
pub const CREDENTIALS_FILE: &str = "credentials.json";

fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

fn home_dir() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env_var(key).map(PathBuf::from)
}

/// Expand a leading `~` (like a shell would).
fn expand_user(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// The folder that holds zindi's user-level files.
///
/// Returns `None` only when no environment variable tells where the home folder is.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = env_var("ZINDI_CONFIG_DIR") {
        return Some(expand_user(&dir));
    }
    if cfg!(windows) {
        if let Some(appdata) = env_var("APPDATA") {
            return Some(PathBuf::from(appdata).join("zindi"));
        }
    }
    if let Some(xdg) = env_var("XDG_CONFIG_HOME") {
        return Some(expand_user(&xdg).join("zindi"));
    }
    home_dir().map(|home| home.join(".config").join("zindi"))
}

/// The path of the shared credentials file.
pub fn credentials_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(CREDENTIALS_FILE))
}

/// Credentials stored on disk by `zindi login` (or [`Client::login`](crate::Client::login)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedCredentials {
    /// The Zindi username.
    pub username: String,
    /// The auth token.
    pub auth_token: String,
}

/// Read saved credentials, or `None` if there are none (or they are unreadable).
pub fn load_credentials() -> Option<SavedCredentials> {
    let text = fs::read_to_string(credentials_path()?).ok()?;
    let data: Value = serde_json::from_str(&text).ok()?;
    let field = |key: &str| match data.get(key)? {
        Value::String(text) => Some(text.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    };
    Some(SavedCredentials {
        username: field("username")?,
        auth_token: field("auth_token")?,
    })
}

/// Save credentials atomically, readable by the current user only. Returns the path.
///
/// The file holds `{"username", "auth_token", "saved_at"}`, the format every zindi client
/// reads.
pub fn save_credentials(username: &str, auth_token: &str) -> Result<PathBuf> {
    let path = credentials_path().ok_or_else(|| {
        Error::Other("Cannot find your home folder: set ZINDI_CONFIG_DIR.".to_owned())
    })?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let payload = json!({
        "username": username,
        "auth_token": auth_token,
        "saved_at": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, false),
    });
    let mut text = serde_json::to_string_pretty(&payload)
        .map_err(|err| Error::Other(format!("Cannot encode credentials: {err}")))?;
    text.push('\n');

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let tmp = parent.join(format!(".credentials-{}-{nanos}.json", std::process::id()));
    let written = write_private(&tmp, text.as_bytes()).and_then(|()| fs::rename(&tmp, &path));
    if let Err(err) = written {
        let _ = fs::remove_file(&tmp);
        return Err(err.into());
    }
    Ok(path)
}

/// Create `path` with permissions `0600` (on Unix) and write `bytes` to it.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Delete saved credentials. Returns `true` if a file was removed.
pub fn delete_credentials() -> Result<bool> {
    let Some(path) = credentials_path() else {
        return Ok(false);
    };
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err.into()),
    }
}

/// Find `zindi.toml` in `start` (default: the current folder) or one of its parents.
pub fn find_project_file(start: Option<&Path>) -> Option<PathBuf> {
    let start = match start {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir().ok()?,
    };
    let start = start.canonicalize().unwrap_or(start);
    start
        .ancestors()
        .map(|folder| folder.join(PROJECT_FILE))
        .find(|candidate| candidate.is_file())
}

/// The competition id stored in the nearest `zindi.toml`, if any.
pub fn read_project_competition(start: Option<&Path>) -> Option<String> {
    let text = fs::read_to_string(find_project_file(start)?).ok()?;
    parse_competition(&text)
}

/// Write (or update) `zindi.toml` in `folder` (default: the current folder) with the
/// given competition id. Other lines of the file are kept. Returns the path.
pub fn write_project_competition(competition: &str, folder: Option<&Path>) -> Result<PathBuf> {
    let folder = match folder {
        Some(folder) => folder.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let path = folder.join(PROJECT_FILE);
    let mut lines: Vec<String> = match fs::read_to_string(&path) {
        Ok(text) => text
            .lines()
            .filter(|line| !is_competition_line(line))
            .map(str::to_owned)
            .collect(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => return Err(err.into()),
    };
    // A JSON string literal is also a valid TOML basic string.
    let literal = serde_json::to_string(competition)
        .map_err(|err| Error::Other(format!("Cannot encode competition id: {err}")))?;
    lines.insert(0, format!("competition = {literal}"));
    fs::write(&path, lines.join("\n") + "\n")?;
    Ok(path)
}

fn is_competition_line(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("competition")
        .is_some_and(|rest| rest.trim_start().starts_with('='))
}

/// Read the top-level `competition = "..."` key of a TOML document.
///
/// This is a deliberately small parser: it understands comments, basic strings (with
/// escapes) and literal strings, which is all `zindi.toml` needs.
fn parse_competition(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            break; // keys after a table header do not belong to the top level
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches(|c| c == '"' || c == '\'');
        if key != "competition" {
            continue;
        }
        return parse_toml_string(value.trim()).filter(|value| !value.is_empty());
    }
    None
}

fn parse_toml_string(value: &str) -> Option<String> {
    if let Some(rest) = value.strip_prefix('\'') {
        return rest.split_once('\'').map(|(inner, _)| inner.to_owned());
    }
    let mut chars = value.strip_prefix('"')?.chars();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'b' => out.push('\u{8}'),
                'f' => out.push('\u{c}'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'u' => out.push(hex_char(&mut chars, 4)?),
                'U' => out.push(hex_char(&mut chars, 8)?),
                _ => return None,
            },
            _ => out.push(c),
        }
    }
    None
}

fn hex_char(chars: &mut std::str::Chars<'_>, len: usize) -> Option<char> {
    let digits: String = chars.by_ref().take(len).collect();
    if digits.len() != len {
        return None;
    }
    char::from_u32(u32::from_str_radix(&digits, 16).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_competition_key() {
        assert_eq!(
            parse_competition("# project\ncompetition = \"Crop-Yield\" # note\nother = 1\n"),
            Some("Crop-Yield".into())
        );
        assert_eq!(parse_competition("competition='Lit'"), Some("Lit".into()));
        assert_eq!(
            parse_competition("competition = \"a\\\"b\\u00e9\""),
            Some("a\"b\u{e9}".into())
        );
        assert_eq!(parse_competition("[tool]\ncompetition = \"nested\""), None);
        assert_eq!(parse_competition("competition = \"\""), None);
        assert_eq!(parse_competition("competitions = \"x\""), None);
    }

    #[test]
    fn recognises_competition_lines() {
        assert!(is_competition_line("  competition = \"x\""));
        assert!(is_competition_line("competition=\"x\""));
        assert!(!is_competition_line("competitions = 1"));
        assert!(!is_competition_line("# competition = \"x\""));
    }
}
