//! The [`Client`]: everything you can do on Zindi, from Rust.

use std::fmt;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use reqwest::Method;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use crate::config;
use crate::error::{Error, Result, PASSWORD_RESET_URL};
use crate::http::{segment, ApiRequest, Body, Transport};
use crate::models::{
    Competition, DataFile, Discussion, LeaderboardEntry, Participation, Submission,
    SubmissionLimits, Team, User,
};

// -- query types -----------------------------------------------------------------------------

/// Which competitions [`Client::competitions`] lists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Status {
    /// Competitions that currently accept submissions (the default).
    #[default]
    Open,
    /// Competitions that have ended.
    Closed,
    /// Both.
    All,
}

/// The kind of a competition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A regular competition.
    Competition,
    /// A hackathon.
    Hackathon,
}

impl Kind {
    /// The value used by the API (`"competition"` or `"hackathon"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Competition => "competition",
            Kind::Hackathon => "hackathon",
        }
    }
}

/// The reward type of a competition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reward {
    /// Cash prizes.
    Prize,
    /// Zindi points only.
    Points,
    /// Knowledge (no prize, no points).
    Knowledge,
}

impl Reward {
    /// The value used by the API (`"prize"`, `"points"` or `"knowledge"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Reward::Prize => "prize",
            Reward::Points => "points",
            Reward::Knowledge => "knowledge",
        }
    }
}

/// Filters for [`Client::competitions`].
///
/// ```
/// use zindi::{CompetitionQuery, Kind, Status};
///
/// let query = CompetitionQuery::new()
///     .status(Status::All)
///     .kind(Kind::Hackathon)
///     .search("speech")
///     .limit(None);
/// assert_eq!(query.limit, None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompetitionQuery {
    /// Open (default), closed or all competitions.
    pub status: Status,
    /// Only this kind; both when `None`.
    pub kind: Option<Kind>,
    /// Only this reward type; any when `None`.
    pub reward: Option<Reward>,
    /// Free-text search on titles.
    pub search: Option<String>,
    /// Maximum number of results (default 50); `None` for all of them.
    pub limit: Option<usize>,
}

impl Default for CompetitionQuery {
    fn default() -> Self {
        Self {
            status: Status::Open,
            kind: None,
            reward: None,
            search: None,
            limit: Some(50),
        }
    }
}

impl CompetitionQuery {
    /// Open competitions, at most 50.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the status filter.
    pub fn status(mut self, status: Status) -> Self {
        self.status = status;
        self
    }

    /// Only list this kind of competition.
    pub fn kind(mut self, kind: Kind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Only list competitions with this reward type.
    pub fn reward(mut self, reward: Reward) -> Self {
        self.reward = Some(reward);
        self
    }

    /// Search titles for this text.
    pub fn search(mut self, text: impl Into<String>) -> Self {
        self.search = Some(text.into());
        self
    }

    /// Set the maximum number of results (`None` for no limit).
    pub fn limit(mut self, limit: Option<usize>) -> Self {
        self.limit = limit;
        self
    }
}

/// Options for [`Client::download`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadOptions {
    /// Only these file names. By default, every file except the "everything in one zip"
    /// archive, unless that archive is the only file.
    pub files: Option<Vec<String>>,
    /// Download again even if the file is already there with the expected size.
    pub overwrite: bool,
}

impl DownloadOptions {
    /// Every file (except the all-in-one archive), skipping files already downloaded.
    pub fn new() -> Self {
        Self::default()
    }

    /// Only download these files.
    pub fn files<I, S>(mut self, files: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.files = Some(files.into_iter().map(Into::into).collect());
        self
    }

    /// Download again even if the file is already there.
    pub fn overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }
}

// -- builder ---------------------------------------------------------------------------------

/// Configures and creates a [`Client`]. Get one with [`Client::builder`].
#[derive(Clone)]
pub struct ClientBuilder {
    token: Option<String>,
    username: Option<String>,
    password: Option<String>,
    competition: Option<String>,
    base_url: Option<String>,
    timeout: Duration,
    max_retries: u32,
    retry_backoff: Duration,
    use_saved_credentials: bool,
    poll_interval: Duration,
    wait_timeout: Duration,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            token: None,
            username: None,
            password: None,
            competition: None,
            base_url: None,
            timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_backoff: Duration::from_millis(500),
            use_saved_credentials: true,
            poll_interval: Duration::from_secs(10),
            wait_timeout: Duration::from_secs(600),
        }
    }
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field("competition", &self.competition)
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .finish_non_exhaustive()
    }
}

impl ClientBuilder {
    /// Use this auth token. It overrides every other source of credentials.
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Your Zindi username (used with [`password`](Self::password) to sign in).
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Your Zindi password. Prefer the `ZINDI_PASSWORD` environment variable or a saved
    /// token. The client signs in lazily, on the first call that needs it.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// The default competition id of the client.
    pub fn competition(mut self, competition: impl Into<String>) -> Self {
        self.competition = Some(competition.into());
        self
    }

    /// The API root. Defaults to `$ZINDI_API_URL`, else `https://api.zindi.world/v1`.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Network timeout for each operation (connecting, reading). Default: 30 seconds.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Retries for transient failures (connection errors, HTTP 429/5xx on reads).
    /// Default: 3.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// The first delay between two retries; it doubles after each retry, up to 30
    /// seconds. A `Retry-After` header sent by Zindi takes precedence. Default: 500 ms.
    pub fn retry_backoff(mut self, delay: Duration) -> Self {
        self.retry_backoff = delay;
        self
    }

    /// Read the shared credentials file written by `zindi login`. Default: `true`.
    pub fn use_saved_credentials(mut self, enabled: bool) -> Self {
        self.use_saved_credentials = enabled;
        self
    }

    /// Delay between two checks when waiting for a score. Default: 10 seconds.
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// How long [`Client::submit`] waits for a score when asked to. Default: 10 minutes.
    pub fn wait_timeout(mut self, timeout: Duration) -> Self {
        self.wait_timeout = timeout;
        self
    }

    /// Create the client.
    ///
    /// # Errors
    ///
    /// Fails if the base URL is invalid or the HTTP client cannot be initialised.
    pub fn build(self) -> Result<Client> {
        let transport = Transport::new(
            self.base_url,
            self.timeout,
            self.max_retries,
            self.retry_backoff,
        )?;
        let has_token = self.token.as_deref().is_some_and(|t| !t.is_empty());
        let state = State {
            token: self.token.filter(|t| !t.is_empty()),
            username: self.username,
            password: self.password,
            resolved: has_token,
            source: has_token.then(|| "argument".to_owned()),
            competition: self.competition,
        };
        Ok(Client {
            transport,
            state: Mutex::new(state),
            use_saved_credentials: self.use_saved_credentials,
            poll_interval: self.poll_interval,
            wait_timeout: self.wait_timeout,
        })
    }
}

// -- client ----------------------------------------------------------------------------------

#[derive(Default)]
struct State {
    token: Option<String>,
    username: Option<String>,
    password: Option<String>,
    /// `true` once credentials were looked up (they are looked up lazily, once).
    resolved: bool,
    source: Option<String>,
    competition: Option<String>,
}

/// A connection to Zindi.
///
/// Most methods take an optional competition id. When it is `None`, the client uses, in
/// order: the competition set with [`use_competition`](Self::use_competition) (or the
/// builder), the `ZINDI_COMPETITION` environment variable, then the `zindi.toml` file of
/// your project (written by `zindi use`).
///
/// You do not need to log in to browse competitions and leaderboards. For everything else
/// the client finds credentials by itself, in this order: the builder's token; the
/// `ZINDI_TOKEN` environment variable; `ZINDI_USERNAME` and `ZINDI_PASSWORD` (it signs in
/// lazily); the shared credentials file written by `zindi login` (see [`config`]). When a
/// token expires and a username and password are known, it signs in again and retries
/// once.
///
/// A `Client` is cheap to share: wrap it in an [`Arc`](std::sync::Arc) to use it from
/// several tasks.
pub struct Client {
    transport: Transport,
    state: Mutex<State>,
    use_saved_credentials: bool,
    poll_interval: Duration,
    wait_timeout: Duration,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state();
        f.debug_struct("Client")
            .field("base_url", &self.transport.base_url())
            .field("username", &state.username)
            .field("competition", &state.competition)
            .field("authenticated", &state.token.is_some())
            .finish_non_exhaustive()
    }
}

impl Client {
    /// A builder to configure a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// A client with the default settings (credentials are found automatically).
    ///
    /// # Errors
    ///
    /// Fails if `ZINDI_API_URL` is not a valid URL or the HTTP client cannot be created.
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    // -- authentication ----------------------------------------------------------------------

    /// The API root this client talks to.
    pub fn base_url(&self) -> &str {
        self.transport.base_url()
    }

    /// The current auth token, if any (does not sign in or read any file).
    pub fn token(&self) -> Option<String> {
        self.state().token.clone()
    }

    /// The username of the signed-in user, if known.
    pub fn username(&self) -> Option<String> {
        self.resolve_credentials();
        self.state().username.clone()
    }

    /// Where the credentials came from: `"argument"`, `"environment variables"`,
    /// `"password"` or the path of the credentials file.
    pub fn credentials_source(&self) -> Option<String> {
        self.resolve_credentials();
        self.state().source.clone()
    }

    /// `true` if credentials are available (a token, or a username and a password).
    pub fn is_authenticated(&self) -> bool {
        self.resolve_credentials();
        let state = self.state();
        state.token.is_some() || (state.username.is_some() && state.password.is_some())
    }

    /// Sign in with a username and password, and return your profile.
    ///
    /// With `save` the auth token (never the password) is stored in the shared credentials
    /// file, so later sessions, the CLI and the other zindi clients are logged in too.
    ///
    /// # Errors
    ///
    /// [`Error::Authentication`] when the username or password is wrong (the message
    /// points to the password reset page).
    pub async fn login(&self, username: &str, password: &str, save: bool) -> Result<User> {
        let data = self.sign_in(username, password).await?;
        if save {
            let (user, token) = {
                let state = self.state();
                (state.username.clone(), state.token.clone())
            };
            config::save_credentials(
                user.as_deref().unwrap_or(username),
                token.as_deref().unwrap_or_default(),
            )?;
        }
        let user = data
            .get("user")
            .filter(|user| user.is_object())
            .cloned()
            .unwrap_or_else(|| json!({ "username": self.state().username.clone() }));
        Ok(User::from_json(&user))
    }

    /// Forget the token and password in memory and, with `forget`, delete the saved
    /// credentials file too.
    ///
    /// # Errors
    ///
    /// Fails only if the credentials file exists but cannot be deleted.
    pub fn logout(&self, forget: bool) -> Result<()> {
        {
            let mut state = self.state();
            state.token = None;
            state.password = None;
            state.resolved = true;
        }
        if forget {
            config::delete_credentials()?;
        }
        Ok(())
    }

    async fn sign_in(&self, username: &str, password: &str) -> Result<Value> {
        let request = ApiRequest::post("auth/signin").body(Body::Form(vec![
            ("username", username.to_owned()),
            ("password", password.to_owned()),
        ]));
        let data = match self.transport.execute(&request, None).await {
            Ok((data, _)) => data,
            Err(err) => {
                if let Some((status, message, errors)) = err.api_parts() {
                    let bad_field = errors
                        .as_object()
                        .is_some_and(|f| f.contains_key("password") || f.contains_key("username"));
                    if bad_field || message.to_lowercase().contains("login") {
                        return Err(Error::Authentication {
                            status,
                            message: format!(
                                "Login failed: {message}. Check your username and password. \
                                 Forgot your password? Reset it at {PASSWORD_RESET_URL}"
                            ),
                            errors: errors.clone(),
                        });
                    }
                }
                return Err(err);
            }
        };
        let token = match data.get("auth_token") {
            Some(Value::String(token)) if !token.is_empty() => token.clone(),
            Some(Value::Number(token)) => token.to_string(),
            _ => return Err(Error::auth("Zindi did not return an auth token.")),
        };
        let signed_in_as = data
            .get("user")
            .and_then(|user| user.get("username"))
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or(username)
            .to_owned();
        let mut state = self.state();
        state.token = Some(token);
        state.username = Some(signed_in_as);
        state.password = Some(password.to_owned());
        state.resolved = true;
        state.source.get_or_insert_with(|| "password".to_owned());
        Ok(data)
    }

    /// Look credentials up once: environment variables, then the credentials file.
    fn resolve_credentials(&self) {
        let mut state = self.state();
        if state.resolved {
            return;
        }
        state.resolved = true;
        if state.username.is_some() && state.password.is_some() {
            state.source = Some("argument".to_owned());
            return;
        }
        let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
        let (token, user, password) = (
            env("ZINDI_TOKEN"),
            env("ZINDI_USERNAME"),
            env("ZINDI_PASSWORD"),
        );
        let can_sign_in = user.is_some() && password.is_some();
        if token.is_some() || can_sign_in {
            state.source = Some("environment variables".to_owned());
            if state.username.is_none() {
                state.username = user;
            }
            if token.is_some() {
                state.token = token;
            }
            if can_sign_in {
                state.password = password;
            }
            return;
        }
        if self.use_saved_credentials {
            if let Some(saved) = config::load_credentials() {
                state.source = config::credentials_path().map(|p| p.display().to_string());
                state.username.get_or_insert(saved.username);
                state.token = Some(saved.auth_token);
            }
        }
    }

    fn sign_in_pair(&self) -> Option<(String, String)> {
        let state = self.state();
        state.username.clone().zip(state.password.clone())
    }

    async fn ensure_token(&self) -> Result<()> {
        self.resolve_credentials();
        if self.state().token.is_some() {
            return Ok(());
        }
        match self.sign_in_pair() {
            Some((username, password)) => self.sign_in(&username, &password).await.map(drop),
            None => Err(Error::auth(
                "You are not logged in. Run `zindi login`, call `Client::login`, or set \
                 ZINDI_TOKEN (or ZINDI_USERNAME and ZINDI_PASSWORD).",
            )),
        }
    }

    /// Make an API call, signing in first if needed and once more if the token expired.
    async fn call(&self, request: ApiRequest, auth_required: bool) -> Result<Value> {
        if auth_required {
            self.ensure_token().await?;
        } else {
            self.resolve_credentials();
        }
        let token = self.token();
        match self.transport.execute(&request, token.as_deref()).await {
            Err(err @ Error::Authentication { .. }) if auth_required => {
                let Some((username, password)) = self.sign_in_pair() else {
                    return Err(err);
                };
                self.sign_in(&username, &password).await?;
                let token = self.token();
                Ok(self.transport.execute(&request, token.as_deref()).await?.0)
            }
            other => other.map(|(data, _)| data),
        }
    }

    /// Fetch the pages of a list endpoint and hand each item to `visit`, until it breaks
    /// or the list ends.
    async fn paginate<F>(
        &self,
        path: &str,
        params: &[(&str, String)],
        per_page: usize,
        mut visit: F,
    ) -> Result<()>
    where
        F: FnMut(&Value) -> ControlFlow<()>,
    {
        self.resolve_credentials();
        let token = self.token();
        let mut seen = 0_u64;
        for page in 0_u64.. {
            let mut request = ApiRequest::get(path);
            for (key, value) in params {
                request = request.query(key, value);
            }
            let request = request.query("page", page).query("per_page", per_page);
            let (data, meta) = self.transport.execute(&request, token.as_deref()).await?;
            let items = data.as_array().map(Vec::as_slice).unwrap_or_default();
            let total = meta.get("total_count").and_then(|total| match total {
                Value::Number(n) => n.as_u64(),
                Value::String(s) => s.parse().ok(),
                _ => None,
            });
            for item in items {
                seen += 1;
                if visit(item).is_break() {
                    return Ok(());
                }
            }
            if items.len() < per_page || total.is_some_and(|total| seen >= total) {
                break;
            }
        }
        Ok(())
    }

    // -- competition selection ---------------------------------------------------------------

    /// Make `competition` the default for this client.
    ///
    /// To remember it for the project (and the CLI), also write it to `zindi.toml` with
    /// [`config::write_project_competition`].
    pub fn use_competition(&self, competition: impl Into<String>) {
        self.state().competition = Some(competition.into());
    }

    /// The default competition set on this client, if any.
    pub fn competition_id(&self) -> Option<String> {
        self.state().competition.clone()
    }

    /// The competition id that a method would use for `competition`.
    ///
    /// # Errors
    ///
    /// [`Error::CompetitionNotSet`] when none was given and none is configured.
    pub fn resolve_competition(&self, competition: Option<&str>) -> Result<String> {
        let explicit = competition.filter(|c| !c.is_empty()).map(str::to_owned);
        explicit
            .or_else(|| self.competition_id().filter(|c| !c.is_empty()))
            .or_else(|| {
                std::env::var("ZINDI_COMPETITION")
                    .ok()
                    .filter(|c| !c.is_empty())
            })
            .or_else(|| config::read_project_competition(None))
            .ok_or(Error::CompetitionNotSet)
    }

    // -- users -------------------------------------------------------------------------------

    /// Your own profile.
    pub async fn me(&self) -> Result<User> {
        let data = self.call(ApiRequest::get("me"), true).await?;
        // `/me` answers `{"auth_token": ..., "user": {...}}`; older versions sent the user itself.
        match data.get("user") {
            Some(user) if user.is_object() => Ok(User::from_json(user)),
            _ => Ok(User::from_json(&data)),
        }
    }

    /// The public profile of any user (no login needed).
    pub async fn user(&self, username: &str) -> Result<User> {
        let path = format!("users/{}", segment(username));
        let data = self.call(ApiRequest::get(path), false).await?;
        Ok(User::from_json(&data))
    }

    // -- competitions ------------------------------------------------------------------------

    /// List competitions (no login needed).
    ///
    /// Filters are sent to the API and enforced again on the results.
    pub async fn competitions(&self, query: CompetitionQuery) -> Result<Vec<Competition>> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(kind) = query.kind {
            params.push(("kind[]", kind.as_str().to_owned()));
        }
        if let Some(reward) = query.reward {
            params.push(("prize", reward.as_str().to_owned()));
        }
        if let Some(search) = query.search.as_ref().filter(|s| !s.is_empty()) {
            params.push(("query", search.clone()));
        }
        if query.status == Status::Open {
            params.push(("active", "true".to_owned()));
        }
        let mut items = Vec::new();
        if query.limit == Some(0) {
            return Ok(items);
        }
        self.paginate("competitions", &params, 50, |raw| {
            let comp = Competition::from_json(raw);
            let wanted = match query.status {
                Status::Open => comp.is_open,
                Status::Closed => !comp.is_open,
                Status::All => true,
            } && differs_not(
                comp.reward_type.as_deref(),
                query.reward.map(Reward::as_str),
            ) && differs_not(comp.kind.as_deref(), query.kind.map(Kind::as_str));
            if wanted {
                items.push(comp);
                if query.limit.is_some_and(|limit| items.len() >= limit) {
                    return ControlFlow::Break(());
                }
            }
            ControlFlow::Continue(())
        })
        .await?;
        Ok(items)
    }

    /// Full details of a competition: data files, rules, metric, team size, ... (no login
    /// needed).
    pub async fn competition(&self, competition: Option<&str>) -> Result<Competition> {
        let cid = self.resolve_competition(competition)?;
        let path = format!("competitions/{}", segment(&cid));
        let data = self.call(ApiRequest::get(path), false).await?;
        Ok(Competition::from_json(&data))
    }

    /// Join a competition (accepting its rules).
    ///
    /// Returns `true` if you just joined and `false` if you were already in.
    ///
    /// # Errors
    ///
    /// [`Error::SecretCodeRequired`] when the competition is private and `secret_code` is
    /// missing or wrong.
    pub async fn join(&self, competition: Option<&str>, secret_code: Option<&str>) -> Result<bool> {
        let cid = self.resolve_competition(competition)?;
        let mut request =
            ApiRequest::post(format!("competitions/{}/participations", segment(&cid)));
        if let Some(code) = secret_code.filter(|code| !code.is_empty()) {
            request = request.body(Body::Json(json!({ "secret_code": code })));
        }
        match self.call(request, true).await {
            Ok(_) => Ok(true),
            Err(Error::AlreadyJoined { .. }) => Ok(false),
            Err(err) => Err(err),
        }
    }

    // -- data --------------------------------------------------------------------------------

    /// Download a competition's data files into `destination` (created if needed).
    ///
    /// Files that are already there with the expected size are skipped, so you can call
    /// this at the start of every run. Downloads go to a temporary `.part` file first, so
    /// an interrupted download never leaves a truncated file behind.
    ///
    /// Returns the paths of the files on disk.
    ///
    /// # Errors
    ///
    /// [`Error::Authentication`] if Zindi refuses the download (the message suggests
    /// `zindi join <id>`), [`Error::Other`] for an unknown file name.
    pub async fn download(
        &self,
        competition: Option<&str>,
        destination: impl AsRef<Path>,
        options: DownloadOptions,
    ) -> Result<Vec<PathBuf>> {
        let cid = self.resolve_competition(competition)?;
        let details = self.competition(Some(&cid)).await?;
        let chosen = choose_files(&details.data_files, options.files.as_deref())?;
        let target = destination.as_ref();
        tokio::fs::create_dir_all(target).await?;
        self.ensure_token().await?;

        let mut paths = Vec::with_capacity(chosen.len());
        for data_file in chosen {
            check_file_name(&data_file.filename)?;
            let path = target.join(&data_file.filename);
            let up_to_date = match tokio::fs::metadata(&path).await {
                Ok(meta) => meta.is_file() && data_file.size.is_none_or(|size| meta.len() == size),
                Err(_) => false,
            };
            if options.overwrite || !up_to_date {
                self.download_file(&cid, &data_file.filename, &path).await?;
            }
            paths.push(path);
        }
        Ok(paths)
    }

    async fn download_file(&self, cid: &str, filename: &str, path: &Path) -> Result<()> {
        let part = path.with_file_name(format!("{filename}.part"));
        let url_path = format!("competitions/{}/files/{}", segment(cid), segment(filename));
        let token = self.token();
        let result = async {
            let mut response = self.transport.download(&url_path, token.as_deref()).await?;
            let mut file = tokio::fs::File::create(&part).await?;
            while let Some(chunk) = response.chunk().await? {
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            drop(file);
            tokio::fs::rename(&part, path).await?;
            Ok::<_, Error>(())
        }
        .await;
        match result {
            Ok(()) => Ok(()),
            Err(err) => {
                let _ = tokio::fs::remove_file(&part).await;
                Err(match err {
                    Error::Authentication { .. } => {
                        let message = format!(
                            "Zindi refused the download of {filename}: {err}. Have you joined \
                             the competition? Try `zindi join {cid}`."
                        );
                        err.with_message(message)
                    }
                    other => other,
                })
            }
        }
    }

    // -- submissions -------------------------------------------------------------------------

    /// Upload a submission file, with a comment to remember what it was.
    ///
    /// With `wait`, poll until Zindi has scored the submission and return it with its
    /// score (see [`ClientBuilder::poll_interval`] and [`ClientBuilder::wait_timeout`]).
    pub async fn submit(
        &self,
        path: impl AsRef<Path>,
        comment: &str,
        competition: Option<&str>,
        wait: bool,
    ) -> Result<Submission> {
        let cid = self.resolve_competition(competition)?;
        let path = path.as_ref();
        let is_file = tokio::fs::metadata(path)
            .await
            .map(|meta| meta.is_file())
            .unwrap_or(false);
        if !is_file {
            return Err(Error::Other(format!(
                "Submission file not found: {}",
                path.display()
            )));
        }
        let request = ApiRequest::post(format!("competitions/{}/submissions", segment(&cid))).body(
            Body::Upload {
                path: path.to_path_buf(),
                comment: comment.to_owned(),
            },
        );
        let data = self.call(request, true).await?;
        let submission = Submission::from_json(&data);
        if wait && !submission.id.is_empty() {
            return self
                .wait_for_score(
                    &submission.id,
                    Some(&cid),
                    self.wait_timeout,
                    self.poll_interval,
                )
                .await;
        }
        Ok(submission)
    }

    /// Your submissions to a competition, newest first (at most `limit`).
    pub async fn submissions(
        &self,
        competition: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<Submission>> {
        let cid = self.resolve_competition(competition)?;
        let request = ApiRequest::get(format!("competitions/{}/submissions", segment(&cid)))
            .query("per_page", limit.filter(|&l| l > 0).unwrap_or(1000));
        let data = self.call(request, true).await?;
        let mut items: Vec<Submission> = data
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter(|item| item.is_object())
            .map(Submission::from_json)
            .collect();
        if let Some(limit) = limit {
            items.truncate(limit);
        }
        Ok(items)
    }

    /// Poll until a submission is scored (or failed) and return it.
    ///
    /// # Errors
    ///
    /// [`Error::Other`] if the submission is still being scored after `timeout`.
    pub async fn wait_for_score(
        &self,
        submission_id: &str,
        competition: Option<&str>,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<Submission> {
        let cid = self.resolve_competition(competition)?;
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let found = self
                .submissions(Some(&cid), None)
                .await?
                .into_iter()
                .find(|sub| sub.id == submission_id && !sub.is_pending());
            if let Some(submission) = found {
                return Ok(submission);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Other(format!(
                    "Submission {submission_id} was not scored within {}s. Check later with \
                     `zindi submissions {cid}`.",
                    timeout.as_secs()
                )));
            }
            tokio::time::sleep(poll_interval).await;
        }
    }

    /// How many submissions you can still make today.
    pub async fn limits(&self, competition: Option<&str>) -> Result<SubmissionLimits> {
        let cid = self.resolve_competition(competition)?;
        let request = ApiRequest::get(format!("competitions/{}/submissions/limits", segment(&cid)));
        let data = self.call(request, true).await?;
        Ok(SubmissionLimits::from_json(&data))
    }

    // -- leaderboard -------------------------------------------------------------------------

    /// The leaderboard of a competition (private ranks once they are revealed); no login
    /// needed.
    ///
    /// Rows without a valid score come last; they are left out unless `include_unranked`.
    pub async fn leaderboard(
        &self,
        competition: Option<&str>,
        limit: Option<usize>,
        include_unranked: bool,
    ) -> Result<Vec<LeaderboardEntry>> {
        let cid = self.resolve_competition(competition)?;
        let mut rows = Vec::new();
        if limit == Some(0) {
            return Ok(rows);
        }
        let path = format!("competitions/{}/participations", segment(&cid));
        self.paginate(&path, &[], 100, |raw| {
            let entry = LeaderboardEntry::from_json(raw);
            if entry.rank.is_none() && !include_unranked {
                // Unranked rows come last: nobody after this one has a valid score.
                return ControlFlow::Break(());
            }
            rows.push(entry);
            if limit.is_some_and(|limit| rows.len() >= limit) {
                return ControlFlow::Break(());
            }
            ControlFlow::Continue(())
        })
        .await?;
        Ok(rows)
    }

    /// Your rank, best score and submission count in a competition.
    pub async fn my_participation(&self, competition: Option<&str>) -> Result<Participation> {
        let cid = self.resolve_competition(competition)?;
        let request = ApiRequest::get(format!(
            "competitions/{}/participations/my_participation",
            segment(&cid)
        ));
        let data = self.call(request, true).await?;
        Ok(Participation::from_json(&data))
    }

    /// Your rank on the leaderboard, or `None` if you have no valid score yet.
    ///
    /// Asks for your participation first and, if it has no rank, looks for your username
    /// (alone or in a team) on the leaderboard.
    pub async fn rank(&self, competition: Option<&str>) -> Result<Option<u32>> {
        let cid = self.resolve_competition(competition)?;
        let rank = match self.my_participation(Some(&cid)).await {
            Ok(participation) => participation.rank,
            Err(err) if err.api_parts().is_some() => None,
            Err(err) => return Err(err),
        };
        if rank.is_some() {
            return Ok(rank);
        }
        let Some(username) = self.username() else {
            return Ok(None);
        };
        let board = self.leaderboard(Some(&cid), None, false).await?;
        Ok(board
            .iter()
            .find(|entry| entry.includes(&username))
            .and_then(|entry| entry.rank))
    }

    // -- teams -------------------------------------------------------------------------------

    /// Your team in a competition, or `None` if you compete alone.
    pub async fn team(&self, competition: Option<&str>) -> Result<Option<Team>> {
        let cid = self.resolve_competition(competition)?;
        let request = ApiRequest::get(format!("competitions/{}/my_team", segment(&cid)));
        match self.call(request, true).await {
            Ok(data) => Ok(data
                .as_object()
                .filter(|team| !team.is_empty())
                .map(|_| Team::from_json(&data))),
            Err(err) => match err.api_parts() {
                Some((status, message, _))
                    if status == Some(404) || message.to_lowercase().contains("not found") =>
                {
                    Ok(None)
                }
                _ => Err(err),
            },
        }
    }

    /// Create a team (you become its leader) and invite people to it.
    pub async fn create_team(
        &self,
        name: &str,
        competition: Option<&str>,
        invite: &[&str],
    ) -> Result<Team> {
        let cid = self.resolve_competition(competition)?;
        let request = ApiRequest::post(format!("competitions/{}/my_team", segment(&cid)))
            .body(Body::Form(vec![("title", name.to_owned())]));
        let data = self.call(request, true).await?;
        let team = if data.is_object() {
            Team::from_json(&data)
        } else {
            Team::from_json(&json!({ "title": name }))
        };
        if !invite.is_empty() {
            self.invite(invite, Some(&cid)).await?;
        }
        Ok(team)
    }

    /// Invite users to your team. Returns who was invited.
    ///
    /// People who already have a pending invitation count as invited.
    pub async fn invite(
        &self,
        usernames: &[&str],
        competition: Option<&str>,
    ) -> Result<Vec<String>> {
        let cid = self.resolve_competition(competition)?;
        let path = format!("competitions/{}/my_team/invite", segment(&cid));
        let mut invited = Vec::with_capacity(usernames.len());
        for name in usernames {
            let request = ApiRequest::post(path.clone())
                .body(Body::Form(vec![("username", (*name).to_owned())]));
            if let Err(err) = self.call(request, true).await {
                let already = err.api_parts().is_some_and(|(_, message, _)| {
                    message.to_lowercase().contains("already invited")
                });
                if !already {
                    return Err(err);
                }
            }
            invited.push((*name).to_owned());
        }
        Ok(invited)
    }

    /// Disband your team (you must be its leader).
    pub async fn disband_team(&self, competition: Option<&str>) -> Result<()> {
        let cid = self.resolve_competition(competition)?;
        let request = ApiRequest::new(
            Method::DELETE,
            format!("competitions/{}/my_team", segment(&cid)),
        );
        self.call(request, true).await.map(drop)
    }

    // -- discussions -------------------------------------------------------------------------

    /// Discussion threads of a competition, newest first (at most `limit`); no login
    /// needed.
    pub async fn discussions(
        &self,
        competition: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<Discussion>> {
        let cid = self.resolve_competition(competition)?;
        let mut items = Vec::new();
        if limit == Some(0) {
            return Ok(items);
        }
        let path = format!("competitions/{}/discussions", segment(&cid));
        self.paginate(&path, &[], 20, |raw| {
            items.push(Discussion::from_json(raw, Some(&cid)));
            if limit.is_some_and(|limit| items.len() >= limit) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .await?;
        Ok(items)
    }
}

/// `false` only when both values are known and different.
fn differs_not(actual: Option<&str>, wanted: Option<&str>) -> bool {
    match (actual, wanted) {
        (Some(actual), Some(wanted)) if !actual.is_empty() => actual == wanted,
        _ => true,
    }
}

fn choose_files(available: &[DataFile], wanted: Option<&[String]>) -> Result<Vec<DataFile>> {
    if let Some(names) = wanted {
        let missing: Vec<&str> = names
            .iter()
            .filter(|name| !available.iter().any(|f| &f.filename == *name))
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            let known: Vec<&str> = available.iter().map(|f| f.filename.as_str()).collect();
            let known = if known.is_empty() {
                "none".to_owned()
            } else {
                known.join(", ")
            };
            return Err(Error::Other(format!(
                "Unknown file(s): {}. Available: {known}",
                missing.join(", ")
            )));
        }
        return Ok(names
            .iter()
            .filter_map(|name| available.iter().find(|f| &f.filename == name).cloned())
            .collect());
    }
    let individual: Vec<DataFile> = available
        .iter()
        .filter(|f| !f.is_archive_of_everything)
        .cloned()
        .collect();
    Ok(if individual.is_empty() {
        available.to_vec()
    } else {
        individual
    })
}

/// Refuse file names that would be written outside the destination folder.
fn check_file_name(name: &str) -> Result<()> {
    let unsafe_name = name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\'])
        || Path::new(name).is_absolute();
    if unsafe_name {
        return Err(Error::Other(format!(
            "Refusing to download {name:?}: unsafe file name."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, archive: bool) -> DataFile {
        DataFile {
            filename: name.to_owned(),
            is_archive_of_everything: archive,
            ..DataFile::default()
        }
    }

    #[test]
    fn archive_is_skipped_unless_alone() {
        let files = [file("a.csv", false), file("all.zip", true)];
        let names =
            |files: Vec<DataFile>| files.into_iter().map(|f| f.filename).collect::<Vec<_>>();
        assert_eq!(names(choose_files(&files, None).unwrap()), ["a.csv"]);
        assert_eq!(names(choose_files(&files[1..], None).unwrap()), ["all.zip"]);
        let wanted = vec!["all.zip".to_owned()];
        assert_eq!(
            names(choose_files(&files, Some(&wanted)).unwrap()),
            ["all.zip"]
        );
        let unknown = vec!["nope.csv".to_owned()];
        let err = choose_files(&files, Some(&unknown)).unwrap_err();
        assert!(err.to_string().starts_with("Unknown file(s): nope.csv"));
    }

    #[test]
    fn unsafe_file_names_are_refused() {
        assert!(check_file_name("Train.csv").is_ok());
        for name in ["", "..", "../evil", "a/b", "a\\b", "/etc/passwd"] {
            assert!(check_file_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn filters_tolerate_unknown_values() {
        assert!(differs_not(None, Some("prize")));
        assert!(differs_not(Some("prize"), None));
        assert!(differs_not(Some("prize"), Some("prize")));
        assert!(!differs_not(Some("points"), Some("prize")));
    }
}
