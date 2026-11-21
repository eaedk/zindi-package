//! Joining, leaderboards, rank and teams.

mod common;

use common::{anon, builder, client, fixture, isolate, requests_to, COMP};
use serde_json::{json, Value};
use wiremock::matchers::{body_string, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zindi::Error;

async fn respond(server: &MockServer, verb: &str, route: &str, body: Value) {
    Mock::given(method(verb))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

/// Answer `first` once, then `then` for every later call.
async fn respond_once_then(
    server: &MockServer,
    verb: &str,
    route: &str,
    first: Value,
    then: Value,
) {
    Mock::given(method(verb))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_json(first))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(server)
        .await;
    respond(server, verb, route, then).await;
}

fn participations() -> String {
    format!("/competitions/{COMP}/participations")
}

// -- joining ---------------------------------------------------------------------------------

#[tokio::test]
async fn join_sends_token_and_reports_new_participation() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(&server, "POST", &participations(), fixture("join_ok")).await;

    assert!(client(&server).join(Some(COMP), None).await.unwrap());

    let request = &requests_to(&server, "POST", &participations()).await[0];
    assert_eq!(request.headers.get("auth-token").unwrap(), "tok_test");
    assert!(request.body.is_empty());
}

#[tokio::test]
async fn join_when_already_in_returns_false() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(&server, "POST", &participations(), fixture("join_already")).await;

    assert!(!client(&server).join(Some(COMP), None).await.unwrap());
}

#[tokio::test]
async fn join_private_competition() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond_once_then(
        &server,
        "POST",
        &participations(),
        fixture("join_secret_required"),
        fixture("join_ok"),
    )
    .await;
    let client = client(&server);

    let err = client.join(Some(COMP), None).await.unwrap_err();
    assert!(matches!(err, Error::SecretCodeRequired { .. }), "{err:?}");
    assert!(client.join(Some(COMP), Some("s3cret")).await.unwrap());

    let requests = requests_to(&server, "POST", &participations()).await;
    let body: Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(body, json!({"secret_code": "s3cret"}));
    assert_eq!(
        requests[1].headers.get("content-type").unwrap(),
        "application/json"
    );
}

// -- leaderboard -----------------------------------------------------------------------------

#[tokio::test]
async fn leaderboard_stops_at_unranked_rows() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(&server, "GET", &participations(), fixture("leaderboard")).await;
    let anon = anon(&server);

    let board = anon.leaderboard(Some(COMP), None, false).await.unwrap();

    let ranks: Vec<_> = board.iter().map(|e| e.rank).collect();
    assert_eq!(ranks, [Some(1), Some(2), Some(3)]);
    let team = &board[1];
    assert!(team.is_team);
    assert_eq!(team.name, "Lagos Learners");
    assert_eq!(team.display_name(), "Lagos Learners (team)");
    assert!(team.includes("AMINA_DEV"));
    assert_eq!(team.countrycode.as_deref(), Some("ng"));
    assert_eq!(board[0].score, Some(0.4123));
    assert_eq!(board[0].submission_count, 23);
    assert!(board[0].last_submission_at.is_some());

    assert_eq!(
        anon.leaderboard(Some(COMP), None, true)
            .await
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        anon.leaderboard(Some(COMP), Some(2), false)
            .await
            .unwrap()
            .len(),
        2
    );
    let request = &requests_to(&server, "GET", &participations()).await[0];
    assert!(request.url.query().unwrap().contains("per_page=100"));
}

#[tokio::test]
async fn rank_from_my_participation() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(
        &server,
        "GET",
        &format!("{}/my_participation", participations()),
        fixture("my_participation"),
    )
    .await;
    let client = client(&server);

    assert_eq!(client.rank(Some(COMP)).await.unwrap(), Some(2));
    let mine = client.my_participation(Some(COMP)).await.unwrap();
    assert_eq!(mine.team_title.as_deref(), Some("Lagos Learners"));
    assert_eq!(mine.score, Some(0.4377));
    assert_eq!(mine.submission_count, 41);
}

#[tokio::test]
async fn rank_falls_back_to_leaderboard_search() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(
        &server,
        "GET",
        &format!("{}/my_participation", participations()),
        json!({"data": {"public_rank": null}}),
    )
    .await;
    respond(&server, "GET", &participations(), fixture("leaderboard")).await;
    let client = builder(&server)
        .token("t")
        .username("zawadi_k")
        .build()
        .unwrap();

    assert_eq!(client.rank(Some(COMP)).await.unwrap(), Some(3));
}

#[tokio::test]
async fn rank_is_none_without_a_score() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("{}/my_participation", participations())))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({"data": {"errors": {"message": "not found"}}})),
        )
        .mount(&server)
        .await;
    respond(&server, "GET", &participations(), fixture("leaderboard")).await;
    let client = builder(&server)
        .token("t")
        .username("newcomer")
        .build()
        .unwrap();

    assert_eq!(client.rank(Some(COMP)).await.unwrap(), None);
}

// -- teams -----------------------------------------------------------------------------------

#[tokio::test]
async fn team_lifecycle() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    let team_path = format!("/competitions/{COMP}/my_team");
    let invite_path = format!("{team_path}/invite");
    Mock::given(method("POST"))
        .and(path(&team_path))
        .and(body_string("title=Lagos+Learners"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("team_created")))
        .expect(1)
        .mount(&server)
        .await;
    respond_once_then(
        &server,
        "POST",
        &invite_path,
        fixture("team_invite_ok"),
        fixture("team_invite_already"),
    )
    .await;
    respond(&server, "GET", &team_path, fixture("my_team")).await;
    Mock::given(method("DELETE"))
        .and(path(&team_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("team_disbanded")))
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server);

    let team = client
        .create_team("Lagos Learners", Some(COMP), &["zawadi_k", "tolu_a"])
        .await
        .unwrap();

    assert_eq!(team.title, "Lagos Learners");
    assert_eq!(team.leader.as_deref(), Some("amina_dev"));
    let invites = requests_to(&server, "POST", &invite_path).await;
    assert_eq!(invites.len(), 2);
    assert_eq!(invites[0].body, b"username=zawadi_k");
    assert_eq!(
        invites[0].headers.get("content-type").unwrap(),
        "application/x-www-form-urlencoded"
    );

    let shown = client.team(Some(COMP)).await.unwrap().expect("a team");
    assert_eq!(shown.members, ["amina_dev", "tolu_a"]);
    assert_eq!(shown.pending_invitations, ["zawadi_k"]);
    assert_eq!(shown.id.as_deref(), Some("team_lagos"));

    client.disband_team(Some(COMP)).await.unwrap();
}

#[tokio::test]
async fn team_is_none_when_solo() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/competitions/{COMP}/my_team")))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({"data": {"errors": {"message": "not found"}}})),
        )
        .mount(&server)
        .await;

    assert_eq!(client(&server).team(Some(COMP)).await.unwrap(), None);
}

#[tokio::test]
async fn invite_other_errors_are_raised() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(
        &server,
        "POST",
        &format!("/competitions/{COMP}/my_team/invite"),
        json!({"data": {"errors": {"base": "User does not exist"}}}),
    )
    .await;

    let err = client(&server)
        .invite(&["ghost"], Some(COMP))
        .await
        .unwrap_err();

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
    assert!(err.to_string().contains("does not exist"));
}

#[tokio::test]
async fn creating_a_second_team_is_an_api_error() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    respond(
        &server,
        "POST",
        &format!("/competitions/{COMP}/my_team"),
        fixture("team_already_leader"),
    )
    .await;

    let err = client(&server)
        .create_team("Another", Some(COMP), &[])
        .await
        .unwrap_err();

    assert_eq!(err.to_string(), "Leader can only be the leader of one team");
    assert_eq!(
        err.errors(),
        Some(&json!({"base": "Leader can only be the leader of one team"}))
    );
}
