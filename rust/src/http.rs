//! Low-level HTTP layer: envelopes, errors, retries, auth headers, downloads.

use std::path::PathBuf;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, LOCATION, USER_AGENT as UA};
use reqwest::multipart::{Form, Part};
use reqwest::{Method, Response, StatusCode, Url};
use serde_json::Value;

use crate::error::{Error, Result};

/// The API root used when nothing else is configured.
pub const DEFAULT_BASE_URL: &str = "https://api.zindi.world/v1";

/// The `User-Agent` sent with every request.
pub const USER_AGENT: &str = concat!(
    "zindi-rust/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/eaedk/zindi-package)"
);

const CURRENT_URL: &str = "https://zindi.world/";
const RETRY_STATUSES: [u16; 5] = [429, 500, 502, 503, 504];
const MAX_BACKOFF: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;

/// What a request carries.
#[derive(Debug, Clone)]
pub(crate) enum Body {
    Empty,
    Json(Value),
    Form(Vec<(&'static str, String)>),
    /// A multipart upload with a `file` part (streamed from disk) and a `comment` part.
    Upload {
        path: PathBuf,
        comment: String,
    },
}

/// A request to the API, kept around so it can be sent again (retries, re-login).
#[derive(Debug, Clone)]
pub(crate) struct ApiRequest {
    pub method: Method,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Body,
}

impl ApiRequest {
    pub fn new(method: Method, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query: Vec::new(),
            body: Body::Empty,
        }
    }

    pub fn get(path: impl Into<String>) -> Self {
        Self::new(Method::GET, path)
    }

    pub fn post(path: impl Into<String>) -> Self {
        Self::new(Method::POST, path)
    }

    pub fn query(mut self, key: &str, value: impl ToString) -> Self {
        self.query.push((key.to_owned(), value.to_string()));
        self
    }

    pub fn body(mut self, body: Body) -> Self {
        self.body = body;
        self
    }
}

/// Percent-encode one path segment (everything but RFC 3986 unreserved characters).
pub(crate) fn segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Thin wrapper around [`reqwest::Client`] that speaks Zindi's conventions.
#[derive(Debug)]
pub(crate) struct Transport {
    base_url: String,
    http: reqwest::Client,
    /// Same settings, but redirects are handled by hand (to drop the token).
    downloads: reqwest::Client,
    max_retries: u32,
    backoff: Duration,
}

impl Transport {
    pub fn new(
        base_url: Option<String>,
        timeout: Duration,
        max_retries: u32,
        backoff: Duration,
    ) -> Result<Self> {
        let base_url = base_url
            .or_else(|| std::env::var("ZINDI_API_URL").ok())
            .filter(|url| !url.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned())
            .trim()
            .trim_end_matches('/')
            .to_owned();
        Url::parse(&base_url)
            .map_err(|err| Error::Other(format!("Invalid API URL {base_url:?}: {err}")))?;

        let mut defaults = HeaderMap::new();
        defaults.insert(UA, HeaderValue::from_static(USER_AGENT));
        defaults.insert(ACCEPT, HeaderValue::from_static("application/json"));
        // Like httpx in the Python client: a timeout per network operation, not a total
        // deadline, so large uploads and downloads are not cut short.
        let builder = || {
            reqwest::Client::builder()
                .default_headers(defaults.clone())
                .connect_timeout(timeout.min(Duration::from_secs(15)))
                .read_timeout(timeout)
        };
        Ok(Self {
            base_url,
            http: builder().redirect(api_redirect_policy()).build()?,
            downloads: builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            max_retries,
            backoff,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            return path.to_owned();
        }
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn headers(token: Option<&str>) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("current-url"),
            HeaderValue::from_static(CURRENT_URL),
        );
        if let Some(token) = token.filter(|token| !token.is_empty()) {
            let mut value = HeaderValue::from_str(token)
                .map_err(|_| Error::auth("The auth token contains invalid characters."))?;
            value.set_sensitive(true);
            // The web app sends `auth-token`; older endpoints read `auth_token`.
            headers.insert(HeaderName::from_static("auth-token"), value.clone());
            headers.insert(HeaderName::from_static("auth_token"), value);
        }
        Ok(headers)
    }

    fn backoff(&self, attempt: u32, response: Option<&Response>) -> Duration {
        let retry_after = response
            .and_then(|r| r.headers().get("retry-after"))
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|secs| secs.is_finite() && *secs >= 0.0);
        if let Some(secs) = retry_after {
            return Duration::from_secs_f64(secs).min(MAX_BACKOFF);
        }
        self.backoff
            .saturating_mul(2u32.saturating_pow(attempt))
            .min(MAX_BACKOFF)
    }

    fn build(&self, request: &ApiRequest, token: Option<&str>) -> Result<reqwest::RequestBuilder> {
        let mut builder = self
            .http
            .request(request.method.clone(), self.url(&request.path))
            .headers(Self::headers(token)?);
        if !request.query.is_empty() {
            builder = builder.query(&request.query);
        }
        builder = match &request.body {
            Body::Empty => builder,
            Body::Json(value) => builder.json(value),
            Body::Form(fields) => builder.form(fields),
            Body::Upload { path, comment } => builder.multipart(upload_form(path, comment)?),
        };
        Ok(builder)
    }

    /// Send a request, retrying transient failures.
    ///
    /// Reads (GET, DELETE) are retried on connection problems and on HTTP 429/5xx. Writes
    /// are only retried when the connection could not be opened, so a POST that may have
    /// reached the server is never replayed.
    async fn send(&self, request: &ApiRequest, token: Option<&str>) -> Result<Response> {
        let idempotent = matches!(
            request.method,
            Method::GET | Method::HEAD | Method::OPTIONS | Method::DELETE
        );
        let mut attempt = 0;
        loop {
            match self.build(request, token)?.send().await {
                Err(err) => {
                    let retryable = if idempotent {
                        err.is_connect() || err.is_timeout() || err.is_request()
                    } else {
                        err.is_connect()
                    };
                    if !retryable || attempt >= self.max_retries {
                        return Err(Error::Http(err));
                    }
                    tokio::time::sleep(self.backoff(attempt, None)).await;
                }
                Ok(response) => {
                    let status = response.status().as_u16();
                    if idempotent && RETRY_STATUSES.contains(&status) && attempt < self.max_retries
                    {
                        tokio::time::sleep(self.backoff(attempt, Some(&response))).await;
                    } else {
                        return Ok(response);
                    }
                }
            }
            attempt += 1;
        }
    }

    /// Send a request and return the `(data, meta)` members of the response envelope.
    pub async fn execute(
        &self,
        request: &ApiRequest,
        token: Option<&str>,
    ) -> Result<(Value, Value)> {
        let response = self.send(request, token).await?;
        unwrap_envelope(response).await
    }

    /// Start downloading a file and return the response, ready to be streamed.
    ///
    /// Redirects (to object storage) are followed by hand: the auth token is only sent to
    /// the API's own origin, never to another host.
    pub async fn download(&self, path: &str, token: Option<&str>) -> Result<Response> {
        let mut url = Url::parse(&self.url(path))
            .map_err(|err| Error::Other(format!("Invalid download URL: {err}")))?;
        let mut headers = Self::headers(token)?;
        for _ in 0..=MAX_REDIRECTS {
            let response = self
                .downloads
                .get(url.clone())
                .headers(headers.clone())
                .send()
                .await?;
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok());
            if response.status().is_redirection() {
                if let Some(location) = location {
                    let next = url.join(location).map_err(|err| {
                        Error::Other(format!("Invalid redirect location {location:?}: {err}"))
                    })?;
                    if next.origin() != url.origin() {
                        headers = HeaderMap::new();
                    }
                    url = next;
                    continue;
                }
            }
            if response.status().is_client_error() || response.status().is_server_error() {
                unwrap_envelope(response).await?;
                return Err(Error::Other("The download failed.".to_owned()));
            }
            return Ok(response);
        }
        Err(Error::Other(
            "Too many redirects while downloading the file.".to_owned(),
        ))
    }
}

/// `true` for Zindi's own hosts (`zindi.world`, `zindi.africa` and their subdomains).
fn is_zindi_host(url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        ["zindi.world", "zindi.africa"]
            .iter()
            .any(|root| host == *root || host.ends_with(&format!(".{root}")))
    })
}

/// API calls follow redirects within the same origin or to Zindi's own hosts (the legacy
/// `api.zindi.africa` redirects to `api.zindi.world`), never elsewhere: the auth token is
/// sent in custom headers that a redirect would otherwise carry to another host.
fn api_redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() > 10 {
            return attempt.error("too many redirects");
        }
        let same_origin = attempt
            .previous()
            .first()
            .is_none_or(|first| first.origin() == attempt.url().origin());
        if same_origin || is_zindi_host(attempt.url()) {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

fn upload_form(path: &std::path::Path, comment: &str) -> Result<Form> {
    let file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "submission".to_owned());
    let body = reqwest::Body::from(tokio::fs::File::from_std(file));
    let part = Part::stream_with_length(body, length)
        .file_name(name)
        .mime_str(guess_mime(path))?;
    Ok(Form::new()
        .part("file", part)
        .text("comment", comment.to_owned()))
}

fn guess_mime(path: &std::path::Path) -> &'static str {
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "txt" => "text/plain",
        "json" => "application/json",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "parquet" => "application/vnd.apache.parquet",
        _ => "application/octet-stream",
    }
}

fn reason(status: StatusCode) -> Value {
    Value::String(status.canonical_reason().unwrap_or("HTTP error").to_owned())
}

/// Truthiness of an `errors` member, the way the platform means it.
fn has_errors(errors: &Value) -> bool {
    match errors {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
        Value::Number(_) => true,
    }
}

/// Read a response and split the `{data, meta}` envelope, raising on errors.
async fn unwrap_envelope(response: Response) -> Result<(Value, Value)> {
    let status = response.status();
    let bytes = response.bytes().await?;
    let is_error = status.is_client_error() || status.is_server_error();
    let payload: Value = match serde_json::from_slice(&bytes) {
        Ok(payload) => payload,
        Err(_) if is_error => return Err(Error::from_api(status.as_u16(), reason(status))),
        Err(_) => {
            let text = String::from_utf8_lossy(&bytes);
            let preview: String = text.trim().replace('\n', " ").chars().take(200).collect();
            return Err(Error::Api {
                status: Some(status.as_u16()),
                message: format!(
                    "Zindi returned a non-JSON response (HTTP {}): {preview}",
                    status.as_u16()
                ),
                errors: Value::Null,
            });
        }
    };
    let (data, meta) = match payload {
        Value::Object(mut map) if map.contains_key("data") => {
            let data = map.remove("data").unwrap_or(Value::Null);
            let meta = map.remove("meta").unwrap_or(Value::Null);
            let errors = data
                .get("errors")
                .filter(|errors| !errors.is_null())
                .or_else(|| map.get("errors"))
                .cloned()
                .unwrap_or(Value::Null);
            check(status, is_error, errors)?;
            (data, meta)
        }
        other => {
            let errors = other.get("errors").cloned().unwrap_or(Value::Null);
            check(status, is_error, errors)?;
            (other, Value::Null)
        }
    };
    Ok((data, meta))
}

fn check(status: StatusCode, is_error: bool, errors: Value) -> Result<()> {
    if has_errors(&errors) {
        return Err(Error::from_api(status.as_u16(), errors));
    }
    if is_error {
        return Err(Error::from_api(status.as_u16(), reason(status)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_are_percent_encoded() {
        assert_eq!(segment("Crop-Yield_2.0~x"), "Crop-Yield_2.0~x");
        assert_eq!(segment("a b/c"), "a%20b%2Fc");
        assert_eq!(segment("é"), "%C3%A9");
    }

    #[test]
    fn backoff_doubles_and_is_capped() {
        let transport = Transport::new(
            Some("https://api.test/v1/".into()),
            Duration::from_secs(5),
            3,
            Duration::from_millis(500),
        )
        .unwrap();
        assert_eq!(transport.base_url(), "https://api.test/v1");
        assert_eq!(transport.backoff(0, None), Duration::from_millis(500));
        assert_eq!(transport.backoff(2, None), Duration::from_secs(2));
        assert_eq!(transport.backoff(10, None), MAX_BACKOFF);
    }

    #[test]
    fn user_agent_names_the_crate() {
        assert!(USER_AGENT.starts_with("zindi-rust/1."));
        assert!(USER_AGENT.ends_with("(+https://github.com/eaedk/zindi-package)"));
    }

    #[test]
    fn mime_types() {
        assert_eq!(guess_mime(std::path::Path::new("a/sub.CSV")), "text/csv");
        assert_eq!(
            guess_mime(std::path::Path::new("model.bin")),
            "application/octet-stream"
        );
    }
}
