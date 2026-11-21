//! Helpers shared by the integration tests.
//!
//! Every test starts with [`isolate`]: it takes a process-wide lock (tests in one binary
//! run on parallel threads, but the environment and the current folder are global), gives
//! the test its own config dir and working dir, and removes ambient `ZINDI_*` variables.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use tempfile::TempDir;
use tokio::sync::{Mutex, MutexGuard};
use wiremock::MockServer;
use zindi::Client;

pub const COMP: &str = "Crop-Yield-Forecast-Challenge";

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

const ZINDI_VARS: [&str; 6] = [
    "ZINDI_TOKEN",
    "ZINDI_USERNAME",
    "ZINDI_PASSWORD",
    "ZINDI_COMPETITION",
    "ZINDI_API_URL",
    "ZINDI_CONFIG_DIR",
];

/// An isolated environment; everything is restored when it is dropped.
pub struct Env {
    pub dir: TempDir,
    previous_cwd: PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl Env {
    /// The working directory of the test.
    pub fn work(&self) -> PathBuf {
        self.dir.path().join("work")
    }

    /// The config dir of the test (where `credentials.json` goes).
    pub fn config(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    pub fn credentials_file(&self) -> PathBuf {
        self.config().join("credentials.json")
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous_cwd);
        for key in ZINDI_VARS {
            std::env::remove_var(key);
        }
    }
}

pub async fn isolate() -> Env {
    let guard = ENV_LOCK.lock().await;
    let dir = tempfile::tempdir().expect("temp dir");
    for key in ZINDI_VARS {
        std::env::remove_var(key);
    }
    std::env::set_var("ZINDI_CONFIG_DIR", dir.path().join("config"));
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).expect("work dir");
    let previous_cwd = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(&work).expect("chdir");
    Env {
        dir,
        previous_cwd,
        _guard: guard,
    }
}

/// Load a JSON fixture shared by every client of this repository.
pub fn fixture(name: &str) -> Value {
    let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../spec/fixtures/"))
        .join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read fixture {}: {err}", path.display()));
    serde_json::from_str(&text).expect("valid fixture JSON")
}

/// A builder pointed at the mock server, with fast retries.
pub fn builder(server: &MockServer) -> zindi::ClientBuilder {
    Client::builder()
        .base_url(server.uri())
        .retry_backoff(Duration::from_millis(1))
        .poll_interval(Duration::from_millis(1))
        .timeout(Duration::from_secs(5))
}

/// A logged-in client (token `tok_test`).
pub fn client(server: &MockServer) -> Client {
    builder(server).token("tok_test").build().expect("client")
}

/// An anonymous client that ignores the credentials file.
pub fn anon(server: &MockServer) -> Client {
    builder(server)
        .use_saved_credentials(false)
        .build()
        .expect("client")
}

/// Requests received by the mock server for this method and path.
pub async fn requests_to(server: &MockServer, method: &str, path: &str) -> Vec<wiremock::Request> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.method.as_str() == method && r.url.path() == path)
        .collect()
}
