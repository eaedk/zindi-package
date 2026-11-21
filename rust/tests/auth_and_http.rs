//! Credentials, sign in, error mapping and retries.

mod common;

use std::time::Duration;

use common::{anon, builder, client, fixture, isolate, requests_to};
use serde_json::{json, Value};
use wiremock::matchers::{body_string, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zindi::{config, Client, Error};

async fn respond(server: &MockServer, verb: &str, route: &str, status: u16, body: Value) {
    Mock::given(method(verb))
        .and(path(route))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(server)
        .await;
}

// -- credentials -----------------------------------------------------------------------------

#[tokio::test]
async fn login_saves_the_token_but_never_the_password() {
    let env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/auth/signin"))
        .and(body_string("username=amina_dev&password=hunter2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("signin_ok")))
        .expect(1)
        .mount(&server)
        .await;
    let anon = anon(&server);

    let user = anon.login("amina_dev", "hunter2", true).await.unwrap();

    assert_eq!(user.username, "amina_dev");
    assert_eq!(anon.token().as_deref(), Some("tok_3f9a1c2e7b"));
    let path = env.credentials_file();
    assert_eq!(config::credentials_path().unwrap(), path);
    let text = std::fs::read_to_string(&path).unwrap();
    let saved: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(saved["username"], "amina_dev");
    assert_eq!(saved["auth_token"], "tok_3f9a1c2e7b");
    assert!(saved["saved_at"].as_str().unwrap().ends_with("+00:00"));
    assert!(!text.contains("hunter2"));
    let request = &requests_to(&server, "POST", "/auth/signin").await[0];
    assert!(request.headers.get("auth-token").is_none());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let leftovers = std::fs::read_dir(env.config()).unwrap().count();
    assert_eq!(leftovers, 1, "no temporary file is left behind");
}

#[tokio::test]
async fn wrong_password_points_to_the_reset_page() {
    let env = isolate().await;
    let server = MockServer::start().await;
    respond(
        &server,
        "POST",
        "/auth/signin",
        200,
        fixture("signin_wrong_password"),
    )
    .await;

    let err = anon(&server)
        .login("amina_dev", "nope", true)
        .await
        .unwrap_err();

    assert!(matches!(err, Error::Authentication { .. }), "{err:?}");
    assert!(
        err.to_string()
            .contains("https://zindi.world/passwords/request"),
        "{err}"
    );
    assert!(!env.credentials_file().exists());
}

#[tokio::test]
async fn saved_credentials_are_used() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(&server, "GET", "/me", 200, fixture("me")).await;
    config::save_credentials("amina_dev", "tok_saved").unwrap();
    let client = builder(&server).build().unwrap();

    assert_eq!(client.me().await.unwrap().username, "amina_dev");

    assert_eq!(
        client.credentials_source(),
        config::credentials_path().map(|p| p.display().to_string())
    );
    assert_eq!(client.username().as_deref(), Some("amina_dev"));
    let request = &requests_to(&server, "GET", "/me").await[0];
    assert_eq!(request.headers.get("auth-token").unwrap(), "tok_saved");
}

#[tokio::test]
async fn saved_credentials_can_be_ignored() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    config::save_credentials("amina_dev", "tok_saved").unwrap();

    let client = anon(&server);

    assert!(!client.is_authenticated());
    assert!(matches!(
        client.me().await,
        Err(Error::Authentication { .. })
    ));
}

#[tokio::test]
async fn env_token_beats_the_saved_file() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(&server, "GET", "/me", 200, fixture("me")).await;
    config::save_credentials("someone", "tok_saved").unwrap();
    std::env::set_var("ZINDI_TOKEN", "tok_env");
    let client = builder(&server).build().unwrap();

    client.me().await.unwrap();

    assert_eq!(
        client.credentials_source().as_deref(),
        Some("environment variables")
    );
    let request = &requests_to(&server, "GET", "/me").await[0];
    assert_eq!(request.headers.get("auth-token").unwrap(), "tok_env");
}

#[tokio::test]
async fn builder_token_beats_the_environment() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(&server, "GET", "/me", 200, fixture("me")).await;
    std::env::set_var("ZINDI_TOKEN", "tok_env");

    let client = client(&server);
    client.me().await.unwrap();

    assert_eq!(client.credentials_source().as_deref(), Some("argument"));
    let request = &requests_to(&server, "GET", "/me").await[0];
    assert_eq!(request.headers.get("auth-token").unwrap(), "tok_test");
}

#[tokio::test]
async fn env_username_and_password_sign_in_lazily() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(&server, "POST", "/auth/signin", 200, fixture("signin_ok")).await;
    respond(&server, "GET", "/me", 200, fixture("me")).await;
    std::env::set_var("ZINDI_USERNAME", "amina_dev");
    std::env::set_var("ZINDI_PASSWORD", "hunter2");
    let client = builder(&server).build().unwrap();

    assert!(client.is_authenticated());
    assert!(requests_to(&server, "POST", "/auth/signin")
        .await
        .is_empty());
    client.me().await.unwrap();
    client.me().await.unwrap();

    assert_eq!(requests_to(&server, "POST", "/auth/signin").await.len(), 1);
    let me = requests_to(&server, "GET", "/me").await;
    assert_eq!(me[1].headers.get("auth-token").unwrap(), "tok_3f9a1c2e7b");
}

#[tokio::test]
async fn expired_token_is_refreshed_once() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(&server, "POST", "/auth/signin", 200, fixture("signin_ok")).await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(401).set_body_json(fixture("error_not_authorized")))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    respond(&server, "GET", "/me", 200, fixture("me")).await;
    let client = builder(&server)
        .token("old")
        .username("amina_dev")
        .password("hunter2")
        .build()
        .unwrap();

    assert_eq!(client.me().await.unwrap().username, "amina_dev");

    assert_eq!(client.token().as_deref(), Some("tok_3f9a1c2e7b"));
    assert_eq!(requests_to(&server, "POST", "/auth/signin").await.len(), 1);
    let me = requests_to(&server, "GET", "/me").await;
    assert_eq!(me.len(), 2);
    assert_eq!(me[0].headers.get("auth-token").unwrap(), "old");
    assert_eq!(me[1].headers.get("auth-token").unwrap(), "tok_3f9a1c2e7b");
}

#[tokio::test]
async fn not_authorized_inside_a_200_envelope() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(&server, "GET", "/me", 200, fixture("error_not_authorized")).await;

    let err = client(&server).me().await.unwrap_err();

    assert!(
        matches!(
            err,
            Error::Authentication {
                status: Some(200),
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "not authorized");
}

#[tokio::test]
async fn logout_forgets_everything() {
    let env = isolate().await;
    let server = MockServer::start().await;
    respond(&server, "POST", "/auth/signin", 200, fixture("signin_ok")).await;
    let anon = anon(&server);
    anon.login("amina_dev", "hunter2", true).await.unwrap();
    assert!(env.credentials_file().exists());

    anon.logout(true).unwrap();

    assert_eq!(anon.token(), None);
    assert!(!anon.is_authenticated());
    assert!(!env.credentials_file().exists());
    assert!(!config::delete_credentials().unwrap());
}

#[tokio::test]
async fn config_dir_follows_the_environment() {
    let env = isolate().await;
    assert_eq!(config::config_dir().unwrap(), env.config());
    std::env::remove_var("ZINDI_CONFIG_DIR");
    if cfg!(windows) {
        return; // %APPDATA% takes over on Windows
    }
    std::env::set_var("XDG_CONFIG_HOME", env.dir.path());
    let xdg = config::config_dir();
    std::env::remove_var("XDG_CONFIG_HOME");
    assert_eq!(xdg.unwrap(), env.dir.path().join("zindi"));
    let home = std::env::var("HOME").unwrap();
    assert_eq!(
        config::config_dir().unwrap(),
        std::path::Path::new(&home).join(".config").join("zindi")
    );
}

#[tokio::test]
async fn unreadable_credentials_are_ignored() {
    let env = isolate().await;
    std::fs::create_dir_all(env.config()).unwrap();
    std::fs::write(env.credentials_file(), "{not json").unwrap();
    assert_eq!(config::load_credentials(), None);
    std::fs::write(env.credentials_file(), r#"{"username": "a"}"#).unwrap();
    assert_eq!(config::load_credentials(), None);
}

// -- transport -------------------------------------------------------------------------------

#[tokio::test]
async fn base_url_comes_from_the_environment() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(
        &server,
        "GET",
        "/v1/users/amina_dev",
        200,
        fixture("user_profile"),
    )
    .await;
    std::env::set_var("ZINDI_API_URL", format!("{}/v1/", server.uri()));

    let client = Client::builder()
        .use_saved_credentials(false)
        .build()
        .unwrap();

    assert_eq!(client.base_url(), format!("{}/v1", server.uri()));
    assert_eq!(
        client.user("amina_dev").await.unwrap().username,
        "amina_dev"
    );
}

#[tokio::test]
async fn default_base_url() {
    let _env = isolate().await;
    let client = Client::new().unwrap();
    assert_eq!(client.base_url(), zindi::DEFAULT_BASE_URL);
    assert_eq!(client.base_url(), "https://api.zindi.world/v1");
}

#[tokio::test]
async fn retries_transient_errors_on_reads() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    for status in [503, 502] {
        Mock::given(method("GET"))
            .and(path("/users/amina_dev"))
            .respond_with(ResponseTemplate::new(status))
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&server)
            .await;
    }
    respond(
        &server,
        "GET",
        "/users/amina_dev",
        200,
        fixture("user_profile"),
    )
    .await;

    assert_eq!(
        anon(&server).user("amina_dev").await.unwrap().username,
        "amina_dev"
    );
    assert_eq!(
        requests_to(&server, "GET", "/users/amina_dev").await.len(),
        3
    );
}

#[tokio::test]
async fn retry_after_is_honoured() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/x"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    respond(
        &server,
        "GET",
        "/users/x",
        200,
        json!({"data": {"username": "x"}}),
    )
    .await;
    // A long backoff proves that the Retry-After value was used instead.
    let client = builder(&server)
        .retry_backoff(Duration::from_secs(30))
        .build()
        .unwrap();

    let started = std::time::Instant::now();
    assert_eq!(client.user("x").await.unwrap().username, "x");
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn gives_up_after_max_retries() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(
        &server,
        "GET",
        "/users/x",
        429,
        json!({"data": {"errors": {"message": "slow down"}}}),
    )
    .await;

    let err = builder(&server)
        .max_retries(2)
        .build()
        .unwrap()
        .user("x")
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            Error::RateLimited {
                status: Some(429),
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "slow down");
    assert_eq!(requests_to(&server, "GET", "/users/x").await.len(), 3);
}

#[tokio::test]
async fn posts_are_not_replayed_on_server_errors() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(
        &server,
        "POST",
        "/competitions/C/participations",
        503,
        json!({}),
    )
    .await;

    let err = client(&server).join(Some("C"), None).await.unwrap_err();

    assert!(
        matches!(
            err,
            Error::Api {
                status: Some(503),
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "Service Unavailable");
    assert_eq!(
        requests_to(&server, "POST", "/competitions/C/participations")
            .await
            .len(),
        1
    );
}

#[tokio::test]
async fn posts_are_not_replayed_after_a_timeout() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/competitions/C/participations"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture("join_ok"))
                .set_delay(Duration::from_secs(3)),
        )
        .mount(&server)
        .await;
    let client = builder(&server)
        .token("t")
        .timeout(Duration::from_millis(300))
        .build()
        .unwrap();

    let err = client.join(Some("C"), None).await.unwrap_err();

    assert!(
        matches!(err, Error::Http(ref e) if e.is_timeout()),
        "{err:?}"
    );
    assert!(err.to_string().starts_with("could not reach Zindi"));
    assert_eq!(
        requests_to(&server, "POST", "/competitions/C/participations")
            .await
            .len(),
        1
    );
}

#[tokio::test]
async fn reads_are_retried_after_a_timeout() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/slow"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"data": {"username": "slow"}}))
                .set_delay(Duration::from_secs(3)),
        )
        .mount(&server)
        .await;
    let client = builder(&server)
        .timeout(Duration::from_millis(300))
        .max_retries(1)
        .build()
        .unwrap();

    let err = client.user("slow").await.unwrap_err();

    assert!(
        matches!(err, Error::Http(ref e) if e.is_timeout()),
        "{err:?}"
    );
    assert_eq!(requests_to(&server, "GET", "/users/slow").await.len(), 2);
}

#[tokio::test]
async fn connection_errors_are_reported() {
    let _env = isolate().await;
    let client = Client::builder()
        .base_url("http://127.0.0.1:1")
        .retry_backoff(Duration::from_millis(1))
        .max_retries(2)
        .use_saved_credentials(false)
        .build()
        .unwrap();

    let err = client.user("x").await.unwrap_err();

    assert!(
        matches!(err, Error::Http(ref e) if e.is_connect()),
        "{err:?}"
    );
}

#[tokio::test]
async fn non_json_success_is_reported() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>maintenance</html>"))
        .mount(&server)
        .await;

    let err = anon(&server).user("x").await.unwrap_err();

    assert!(
        matches!(
            err,
            Error::Api {
                status: Some(200),
                ..
            }
        ),
        "{err:?}"
    );
    assert!(err.to_string().contains("non-JSON"), "{err}");
    assert!(err.to_string().contains("maintenance"), "{err}");
}

#[tokio::test]
async fn non_json_error_uses_the_reason_phrase() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/x"))
        .respond_with(ResponseTemplate::new(403).set_body_string("<html>denied</html>"))
        .mount(&server)
        .await;

    let err = anon(&server).user("x").await.unwrap_err();

    assert!(
        matches!(
            err,
            Error::Authentication {
                status: Some(403),
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(err.to_string(), "Forbidden");
}

#[tokio::test]
async fn top_level_errors_are_detected() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(
        &server,
        "GET",
        "/users/x",
        200,
        json!({"errors": {"message": "not found"}}),
    )
    .await;

    let err = anon(&server).user("x").await.unwrap_err();

    assert!(matches!(err, Error::NotFound { .. }), "{err:?}");
}
