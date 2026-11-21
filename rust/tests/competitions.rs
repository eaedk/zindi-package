//! Browsing: competitions, details, users, discussions, competition selection.

mod common;

use common::{anon, client, fixture, isolate, requests_to, COMP};
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zindi::{CompetitionQuery, Error, Kind, Reward, Status};

async fn serve(server: &MockServer, route: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn lists_open_competitions_without_login() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    serve(&server, "/competitions", fixture("competitions")).await;

    let comps = anon(&server)
        .competitions(CompetitionQuery::new())
        .await
        .unwrap();

    let ids: Vec<_> = comps.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, [COMP, "Swahili-ASR-Hackathon"]);
    let request = &requests_to(&server, "GET", "/competitions").await[0];
    let params: Vec<(String, String)> = request.url.query_pairs().into_owned().collect();
    assert!(params.contains(&("active".into(), "true".into())));
    assert!(params.contains(&("page".into(), "0".into())));
    assert!(params.contains(&("per_page".into(), "50".into())));
    assert!(request.headers.get("auth-token").is_none());
    assert_eq!(
        request.headers.get("current-url").unwrap(),
        "https://zindi.world/"
    );
    let agent = request.headers.get("user-agent").unwrap().to_str().unwrap();
    assert!(agent.starts_with("zindi-rust/1.0.0 (+https://github.com/eaedk/zindi-package)"));
}

#[tokio::test]
async fn filters_are_sent_and_enforced() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    serve(&server, "/competitions", fixture("competitions")).await;

    let query = CompetitionQuery::new()
        .status(Status::All)
        .kind(Kind::Hackathon)
        .reward(Reward::Knowledge)
        .search("asr");
    let comps = anon(&server).competitions(query).await.unwrap();

    let request = &requests_to(&server, "GET", "/competitions").await[0];
    let params: Vec<(String, String)> = request.url.query_pairs().into_owned().collect();
    assert!(params.contains(&("kind[]".into(), "hackathon".into())));
    assert!(params.contains(&("prize".into(), "knowledge".into())));
    assert!(params.contains(&("query".into(), "asr".into())));
    assert!(!params.iter().any(|(k, _)| k == "active"));
    assert_eq!(comps.len(), 1);
    assert_eq!(comps[0].id, "Swahili-ASR-Hackathon");
    assert!(comps[0].secret_code_required && comps[0].is_participant);
}

#[tokio::test]
async fn closed_status_and_limit() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    serve(&server, "/competitions", fixture("competitions")).await;
    let anon = anon(&server);

    let closed = anon
        .competitions(CompetitionQuery::new().status(Status::Closed))
        .await
        .unwrap();
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].id, "Air-Quality-Lagos");
    assert_eq!(closed[0].status(), "closed");

    let one = anon
        .competitions(CompetitionQuery::new().status(Status::All).limit(Some(1)))
        .await
        .unwrap();
    assert_eq!(one.len(), 1);
}

#[tokio::test]
async fn competitions_paginate_until_total() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    let data = fixture("competitions")["data"].as_array().unwrap().clone();
    let first: Vec<_> = data[..2].iter().cycle().take(50).cloned().collect();
    let second = data[2..].to_vec();
    for (page, items) in [("0", first), ("1", second)] {
        Mock::given(method("GET"))
            .and(path("/competitions"))
            .and(query_param("page", page))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"data": items, "meta": {"total_count": 51}})),
            )
            .expect(1)
            .mount(&server)
            .await;
    }

    let comps = anon(&server)
        .competitions(CompetitionQuery::new().status(Status::All).limit(None))
        .await
        .unwrap();

    assert_eq!(comps.len(), 51);
}

#[tokio::test]
async fn competition_details() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    serve(
        &server,
        &format!("/competitions/{COMP}"),
        fixture("competition"),
    )
    .await;

    let comp = anon(&server).competition(Some(COMP)).await.unwrap();

    assert_eq!(comp.title, "Crop Yield Forecast Challenge");
    assert_eq!(comp.metric.as_deref(), Some("RMSE"));
    assert_eq!(comp.max_team_size, Some(4));
    assert_eq!(comp.daily_submission_limit(), Some(10));
    let names: Vec<_> = comp
        .data_files
        .iter()
        .map(|f| f.filename.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "Train.csv",
            "Test.csv",
            "SampleSubmission.csv",
            "all_data.zip"
        ]
    );
    assert_eq!(comp.data_files[0].size_human(), "1.0 MB");
    assert!(comp.data_files[3].is_archive_of_everything);
    assert!(comp.description().contains("maize"));
    assert!(comp.rules().contains("10 submissions per day"));
    assert_eq!(
        comp.url(),
        format!("https://zindi.world/competitions/{COMP}")
    );
    assert_eq!(comp.end_time.unwrap().format("%Y").to_string(), "2026");
    assert_eq!(comp.raw["difficulty"], "intermediate");
}

#[tokio::test]
async fn missing_competition_is_not_found() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/competitions/nope"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({"data": {"errors": {"message": "not found"}}})),
        )
        .mount(&server)
        .await;

    let err = anon(&server).competition(Some("nope")).await.unwrap_err();

    assert!(
        matches!(
            err,
            Error::NotFound {
                status: Some(404),
                ..
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn competition_id_resolution_order() {
    let env = isolate().await;
    let server = MockServer::start().await;
    let anon = anon(&server);

    assert!(matches!(
        anon.resolve_competition(None),
        Err(Error::CompetitionNotSet)
    ));
    std::fs::write(
        env.work().join("zindi.toml"),
        "competition = \"From-File\"\n",
    )
    .unwrap();
    assert_eq!(anon.resolve_competition(None).unwrap(), "From-File");
    let nested = env.work().join("a").join("b");
    std::fs::create_dir_all(&nested).unwrap();
    std::env::set_current_dir(&nested).unwrap();
    assert_eq!(anon.resolve_competition(None).unwrap(), "From-File");
    std::env::set_var("ZINDI_COMPETITION", "From-Env");
    assert_eq!(anon.resolve_competition(None).unwrap(), "From-Env");
    anon.use_competition("From-Use");
    assert_eq!(anon.resolve_competition(None).unwrap(), "From-Use");
    assert_eq!(
        anon.resolve_competition(Some("Explicit")).unwrap(),
        "Explicit"
    );
}

#[tokio::test]
async fn builder_competition_is_the_default() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    serve(
        &server,
        &format!("/competitions/{COMP}"),
        fixture("competition"),
    )
    .await;

    let client = common::builder(&server).competition(COMP).build().unwrap();

    assert_eq!(client.competition(None).await.unwrap().id, COMP);
}

#[tokio::test]
async fn project_file_round_trip() {
    let env = isolate().await;
    let root = env.work();
    std::fs::write(
        root.join("zindi.toml"),
        "# my project\ncompetition = \"Old\"\nother = 1\n",
    )
    .unwrap();

    zindi::config::write_project_competition("New-One", Some(&root)).unwrap();

    let nested = root.join("x").join("y");
    std::fs::create_dir_all(&nested).unwrap();
    assert_eq!(
        zindi::config::read_project_competition(Some(&nested)).as_deref(),
        Some("New-One")
    );
    let text = std::fs::read_to_string(root.join("zindi.toml")).unwrap();
    assert_eq!(text, "competition = \"New-One\"\n# my project\nother = 1\n");
}

#[tokio::test]
async fn user_profile() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    serve(&server, "/users/amina_dev", fixture("user_profile")).await;

    let user = anon(&server).user("amina_dev").await.unwrap();

    assert_eq!(user.full_name(), "Amina Diallo");
    assert_eq!(
        (user.gold_medals, user.silver_medals, user.bronze_medals),
        (1, 2, 4)
    );
    assert_eq!(user.competitions_count, Some(14));
    assert_eq!(user.url(), "https://zindi.world/users/amina_dev");
    assert!(user.joined_at.is_some());
}

#[tokio::test]
async fn me_requires_login() {
    let _env = isolate().await;
    let server = MockServer::start().await;

    let err = anon(&server).me().await.unwrap_err();

    assert!(matches!(err, Error::Authentication { status: None, .. }));
    assert!(err.to_string().contains("zindi login"));
}

#[tokio::test]
async fn me_with_token() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    serve(&server, "/me", fixture("me")).await;

    let me = client(&server).me().await.unwrap();

    assert_eq!(me.username, "amina_dev");
    assert_eq!(me.points, Some(1250));
    let request = &requests_to(&server, "GET", "/me").await[0];
    assert_eq!(request.headers.get("auth-token").unwrap(), "tok_test");
    assert_eq!(request.headers.get("auth_token").unwrap(), "tok_test");
}

#[tokio::test]
async fn discussions() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    serve(
        &server,
        &format!("/competitions/{COMP}/discussions"),
        fixture("discussions"),
    )
    .await;

    let threads = anon(&server)
        .discussions(Some(COMP), Some(20))
        .await
        .unwrap();

    assert_eq!(threads.len(), 2);
    assert_eq!(threads[0].id, "7001");
    assert_eq!(threads[0].author.as_deref(), Some("zawadi_k"));
    assert_eq!(
        threads[0]
            .created_at
            .unwrap()
            .format("%Y-%m-%d")
            .to_string(),
        "2026-11-15"
    );
    assert_eq!(threads[0].comments_count, 4);
    assert_eq!(
        threads[0].url(),
        format!(
            "https://zindi.world/competitions/{COMP}/discussions/7001/is-external-weather-data-allowed"
        )
    );
    let request =
        &requests_to(&server, "GET", &format!("/competitions/{COMP}/discussions")).await[0];
    assert!(request.url.query().unwrap().contains("per_page=20"));
}

#[test]
fn client_and_futures_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_send<T: Send>(_: &T) {}
    assert_send_sync::<zindi::Client>();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = runtime.enter();
    let client = zindi::Client::builder()
        .base_url("http://127.0.0.1:9")
        .token("t")
        .build()
        .unwrap();
    assert_send(&client.me());
    assert_send(&client.download(Some(COMP), "data", zindi::DownloadOptions::new()));
    assert_send(&client.submit("x.csv", "", Some(COMP), true));
    assert_send(&client.rank(Some(COMP)));
}
