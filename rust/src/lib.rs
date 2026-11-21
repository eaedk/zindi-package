//! An async client for [Zindi](https://zindi.world), the data science competition
//! platform.
//!
//! Browse competitions, download their data, submit predictions, follow leaderboards and
//! manage teams, from Rust. The crate speaks the same API and reads the same credentials
//! file as the Python, JavaScript and R zindi clients: log in once with any of them and
//! every client is logged in.
//!
//! # Example
//!
//! ```no_run
//! use zindi::{Client, CompetitionQuery, DownloadOptions};
//!
//! #[tokio::main]
//! async fn main() -> zindi::Result<()> {
//!     // Credentials come from ZINDI_TOKEN, ZINDI_USERNAME + ZINDI_PASSWORD, or the
//!     // file written by `zindi login`. Browsing needs no credentials at all.
//!     let client = Client::new()?;
//!
//!     for comp in client.competitions(CompetitionQuery::new().limit(Some(5))).await? {
//!         println!("{:<40} {}", comp.id, comp.title);
//!     }
//!
//!     let id = "Crop-Yield-Forecast-Challenge";
//!     client.join(Some(id), None).await?;
//!     let files = client.download(Some(id), "data", DownloadOptions::new()).await?;
//!     println!("downloaded {} files", files.len());
//!
//!     let submission = client
//!         .submit("submission.csv", "baseline", Some(id), true)
//!         .await?;
//!     println!("score: {:?}", submission.score());
//!     println!("rank: {:?}", client.rank(Some(id)).await?);
//!     Ok(())
//! }
//! ```
//!
//! # Authentication
//!
//! The [`Client`] looks for credentials lazily, the first time a call needs them, in this
//! order:
//!
//! 1. a token given to [`ClientBuilder::token`];
//! 2. the `ZINDI_TOKEN` environment variable;
//! 3. `ZINDI_USERNAME` and `ZINDI_PASSWORD` (or [`ClientBuilder::username`] and
//!    [`ClientBuilder::password`]): the client signs in on first use;
//! 4. the shared credentials file `<config dir>/credentials.json` (see [`config`]).
//!
//! [`Client::login`] signs in explicitly and can save the token (never the password) to
//! the shared file.
//!
//! # Choosing a competition
//!
//! Methods take an `Option<&str>` competition id. With `None` the client uses the id set
//! with [`Client::use_competition`], then `ZINDI_COMPETITION`, then the nearest
//! `zindi.toml` file with a `competition = "..."` line.
//!
//! # Configuration
//!
//! | Environment variable | Meaning |
//! |---|---|
//! | `ZINDI_API_URL` | API root (default `https://api.zindi.world/v1`) |
//! | `ZINDI_TOKEN` | Auth token |
//! | `ZINDI_USERNAME`, `ZINDI_PASSWORD` | Credentials to sign in with |
//! | `ZINDI_COMPETITION` | Default competition id |
//! | `ZINDI_CONFIG_DIR` | Folder of the shared credentials file |

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod client;
pub mod config;
mod error;
mod http;
mod models;

pub use client::{Client, ClientBuilder, CompetitionQuery, DownloadOptions, Kind, Reward, Status};
pub use error::{Error, Result, PASSWORD_RESET_URL};
pub use http::{DEFAULT_BASE_URL, USER_AGENT};
pub use models::{
    html_to_text, parse_time, Competition, DataFile, Discussion, LeaderboardEntry, Participation,
    Submission, SubmissionLimits, Team, User, WEB_URL,
};

/// The version of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
