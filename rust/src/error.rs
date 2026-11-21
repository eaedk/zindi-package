//! The [`Error`] type returned by every fallible operation of this crate.

use serde_json::Value;

/// Web page where a forgotten password can be reset.
pub const PASSWORD_RESET_URL: &str = "https://zindi.world/passwords/request";

/// A `Result` whose error type is [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong when talking to Zindi.
///
/// The variants that come from an API answer (`AlreadyJoined`, `SecretCodeRequired`,
/// `Authentication`, `NotFound`, `RateLimited` and `Api`) all carry the HTTP status (when
/// there was a response), a readable message and the raw `errors` object returned by
/// Zindi (`Value::Null` when there was none).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// You are already a participant. [`Client::join`](crate::Client::join) turns this
    /// one into `Ok(false)`.
    #[error("{message}")]
    AlreadyJoined {
        /// HTTP status of the response.
        status: Option<u16>,
        /// Readable message.
        message: String,
        /// The `errors` object returned by Zindi.
        errors: Value,
    },

    /// The competition is private: joining it needs a secret code.
    #[error("{message}")]
    SecretCodeRequired {
        /// HTTP status of the response.
        status: Option<u16>,
        /// Readable message.
        message: String,
        /// The `errors` object returned by Zindi.
        errors: Value,
    },

    /// Missing, wrong or expired credentials, or an action you are not allowed to do.
    #[error("{message}")]
    Authentication {
        /// HTTP status of the response, `None` when no request was made.
        status: Option<u16>,
        /// Readable message.
        message: String,
        /// The `errors` object returned by Zindi.
        errors: Value,
    },

    /// The competition, user or file does not exist (or is not visible to you).
    #[error("{message}")]
    NotFound {
        /// HTTP status of the response.
        status: Option<u16>,
        /// Readable message.
        message: String,
        /// The `errors` object returned by Zindi.
        errors: Value,
    },

    /// Zindi asked us to slow down (HTTP 429) and retries did not help.
    #[error("{message}")]
    RateLimited {
        /// HTTP status of the response.
        status: Option<u16>,
        /// Readable message.
        message: String,
        /// The `errors` object returned by Zindi.
        errors: Value,
    },

    /// Any other error answer from the Zindi API.
    #[error("{message}")]
    Api {
        /// HTTP status of the response.
        status: Option<u16>,
        /// Readable message.
        message: String,
        /// The `errors` object returned by Zindi.
        errors: Value,
    },

    /// An operation needs a competition, but none was given or configured.
    #[error(
        "No competition selected. Pass a competition id, call \
         `client.use_competition(\"<competition-id>\")`, set ZINDI_COMPETITION, \
         or run `zindi use <competition-id>`."
    )]
    CompetitionNotSet,

    /// The request could not be sent or the response could not be read.
    #[error("could not reach Zindi: {0}")]
    Http(#[from] reqwest::Error),

    /// A local file could not be read or written.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Any other problem (unknown file name, scoring timeout, ...).
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// The HTTP status of the response that caused this error, when there was one.
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Http(err) => err.status().map(|s| s.as_u16()),
            _ => self.api_parts().and_then(|(status, _, _)| status),
        }
    }

    /// The raw `errors` object returned by Zindi, for errors that come from the API.
    pub fn errors(&self) -> Option<&Value> {
        self.api_parts().map(|(_, _, errors)| errors)
    }

    /// `(status, message, errors)` for the variants built from an API answer.
    pub(crate) fn api_parts(&self) -> Option<(Option<u16>, &str, &Value)> {
        match self {
            Error::AlreadyJoined {
                status,
                message,
                errors,
            }
            | Error::SecretCodeRequired {
                status,
                message,
                errors,
            }
            | Error::Authentication {
                status,
                message,
                errors,
            }
            | Error::NotFound {
                status,
                message,
                errors,
            }
            | Error::RateLimited {
                status,
                message,
                errors,
            }
            | Error::Api {
                status,
                message,
                errors,
            } => Some((*status, message.as_str(), errors)),
            _ => None,
        }
    }

    /// An authentication error raised before any request was made.
    pub(crate) fn auth(message: impl Into<String>) -> Self {
        Error::Authentication {
            status: None,
            message: message.into(),
            errors: Value::Null,
        }
    }

    /// The most specific error for an error answer of the API.
    pub(crate) fn from_api(status: u16, errors: Value) -> Self {
        let message = error_message(&errors);
        let lowered = message.to_lowercase();
        let status_opt = Some(status);
        if lowered.contains("already in") || lowered.contains("already joined") {
            return Error::AlreadyJoined {
                status: status_opt,
                message,
                errors,
            };
        }
        if lowered.contains("secret code") {
            return Error::SecretCodeRequired {
                status: status_opt,
                message,
                errors,
            };
        }
        if status == 401
            || status == 403
            || lowered.contains("not authorized")
            || lowered.contains("not authenticated")
        {
            return Error::Authentication {
                status: status_opt,
                message,
                errors,
            };
        }
        if status == 404 || lowered == "not found" {
            return Error::NotFound {
                status: status_opt,
                message,
                errors,
            };
        }
        if status == 429 {
            return Error::RateLimited {
                status: status_opt,
                message,
                errors,
            };
        }
        Error::Api {
            status: status_opt,
            message,
            errors,
        }
    }

    /// The same error with another message (status and `errors` are kept).
    pub(crate) fn with_message(self, new_message: String) -> Self {
        match self {
            Error::AlreadyJoined { status, errors, .. } => Error::AlreadyJoined {
                status,
                message: new_message,
                errors,
            },
            Error::SecretCodeRequired { status, errors, .. } => Error::SecretCodeRequired {
                status,
                message: new_message,
                errors,
            },
            Error::Authentication { status, errors, .. } => Error::Authentication {
                status,
                message: new_message,
                errors,
            },
            Error::NotFound { status, errors, .. } => Error::NotFound {
                status,
                message: new_message,
                errors,
            },
            Error::RateLimited { status, errors, .. } => Error::RateLimited {
                status,
                message: new_message,
                errors,
            },
            Error::Api { status, errors, .. } => Error::Api {
                status,
                message: new_message,
                errors,
            },
            other => other,
        }
    }
}

/// Turn Zindi's `errors` object (whatever its shape) into one readable sentence.
pub(crate) fn error_message(errors: &Value) -> String {
    match errors {
        Value::Null => "Unknown error".to_owned(),
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(error_message)
            .collect::<Vec<_>>()
            .join("; "),
        Value::Object(map) => {
            for key in ["message", "base", "error"] {
                if let Some(value) = map.get(key) {
                    return error_message(value);
                }
            }
            map.iter()
                .map(|(key, value)| format!("{key}: {}", error_message(value)))
                .collect::<Vec<_>>()
                .join("; ")
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn error_message_shapes() {
        assert_eq!(
            error_message(&json!({"message": "already in"})),
            "already in"
        );
        assert_eq!(error_message(&json!({"base": ["a", "b"]})), "a; b");
        assert_eq!(
            error_message(&json!({"password": "wrong password"})),
            "password: wrong password"
        );
        assert_eq!(
            error_message(&json!({"title": "taken", "size": ["too big"]})),
            "title: taken; size: too big"
        );
        assert_eq!(error_message(&json!("plain")), "plain");
        assert_eq!(error_message(&Value::Null), "Unknown error");
    }

    #[test]
    fn mapping_picks_the_most_specific_variant() {
        let cases = [
            (200, json!({"message": "already in"}), "AlreadyJoined"),
            (
                200,
                json!({"message": "Needs a secret code"}),
                "SecretCodeRequired",
            ),
            (200, json!({"message": "not authorized"}), "Authentication"),
            (403, json!("Forbidden"), "Authentication"),
            (404, json!("Nope"), "NotFound"),
            (200, json!({"message": "not found"}), "NotFound"),
            (429, json!({"message": "slow down"}), "RateLimited"),
            (500, json!({"base": "boom"}), "Api"),
        ];
        for (status, errors, expected) in cases {
            let err = Error::from_api(status, errors);
            let name = format!("{err:?}");
            assert!(name.starts_with(expected), "{status}: {name}");
            assert_eq!(err.status(), Some(status));
        }
    }

    #[test]
    fn with_message_keeps_the_details() {
        let err = Error::from_api(401, json!({"message": "not authorized"}))
            .with_message("try again".into());
        assert_eq!(err.to_string(), "try again");
        assert_eq!(err.status(), Some(401));
        assert_eq!(err.errors(), Some(&json!({"message": "not authorized"})));
    }
}
