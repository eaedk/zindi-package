//! Downloads and submissions.

mod common;

use std::time::Duration;

use common::{client, fixture, isolate, requests_to, COMP};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zindi::{DownloadOptions, Error};

fn file_path(name: &str) -> String {
    format!("/competitions/{COMP}/files/{name}")
}

async fn serve_details(server: &MockServer, details: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/competitions/{COMP}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(details))
        .mount(server)
        .await;
}

async fn serve_file(server: &MockServer, name: &str, bytes: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(file_path(name)))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(server)
        .await;
}

fn details_with(files: Value) -> Value {
    let mut details = fixture("competition");
    details["data"]["datafiles"] = files;
    details
}

// -- downloads -------------------------------------------------------------------------------

#[tokio::test]
async fn download_skips_the_archive_and_existing_files() {
    let env = isolate().await;
    let server = MockServer::start().await;
    serve_details(&server, fixture("competition")).await;
    serve_file(&server, "Train.csv", vec![b'a'; 1_048_576]).await;
    serve_file(&server, "Test.csv", vec![b'b'; 262_144]).await;
    serve_file(&server, "SampleSubmission.csv", vec![b'c'; 2048]).await;
    serve_file(&server, "all_data.zip", b"zip".to_vec()).await;
    let client = client(&server);
    let target = env.dir.path().join("data");

    let paths = client
        .download(Some(COMP), &target, DownloadOptions::new())
        .await
        .unwrap();

    let names: Vec<_> = paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["Train.csv", "Test.csv", "SampleSubmission.csv"]);
    assert_eq!(
        std::fs::metadata(target.join("Train.csv")).unwrap().len(),
        1_048_576
    );
    assert!(requests_to(&server, "GET", &file_path("all_data.zip"))
        .await
        .is_empty());
    let leftovers: Vec<_> = std::fs::read_dir(&target)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
        .collect();
    assert!(leftovers.is_empty());
    let request = &requests_to(&server, "GET", &file_path("Train.csv")).await[0];
    assert_eq!(request.headers.get("auth-token").unwrap(), "tok_test");

    // Second run: everything is already there with the right size.
    client
        .download(Some(COMP), &target, DownloadOptions::new())
        .await
        .unwrap();
    assert_eq!(
        requests_to(&server, "GET", &file_path("Train.csv"))
            .await
            .len(),
        1
    );
}

#[tokio::test]
async fn download_selected_files_and_overwrite() {
    let env = isolate().await;
    let server = MockServer::start().await;
    serve_details(&server, fixture("competition")).await;
    serve_file(&server, "SampleSubmission.csv", vec![b'c'; 2048]).await;
    let client = client(&server);
    let target = env.dir.path();

    let only = DownloadOptions::new().files(["SampleSubmission.csv"]);
    client
        .download(Some(COMP), target, only.clone())
        .await
        .unwrap();
    client
        .download(Some(COMP), target, only.clone())
        .await
        .unwrap();
    client
        .download(Some(COMP), target, only.overwrite(true))
        .await
        .unwrap();
    assert_eq!(
        requests_to(&server, "GET", &file_path("SampleSubmission.csv"))
            .await
            .len(),
        2
    );

    let err = client
        .download(
            Some(COMP),
            target,
            DownloadOptions::new().files(["nope.csv"]),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Other(ref m) if m.contains("Unknown file(s): nope.csv")),
        "{err}"
    );
}

#[tokio::test]
async fn download_follows_redirect_without_leaking_the_token() {
    let env = isolate().await;
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    serve_details(
        &api,
        details_with(json!([{"filename": "Train.csv", "filesize": 4}])),
    )
    .await;
    let signed = format!("{}/signed/Train.csv?sig=abc", storage.uri());
    Mock::given(method("GET"))
        .and(path(file_path("Train.csv")))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", signed.as_str()))
        .expect(1)
        .mount(&api)
        .await;
    Mock::given(method("GET"))
        .and(path("/signed/Train.csv"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"data".to_vec()))
        .expect(1)
        .mount(&storage)
        .await;

    client(&api)
        .download(Some(COMP), env.dir.path(), DownloadOptions::new())
        .await
        .unwrap();

    assert_eq!(
        std::fs::read(env.dir.path().join("Train.csv")).unwrap(),
        b"data"
    );
    let request = &requests_to(&storage, "GET", "/signed/Train.csv").await[0];
    assert!(request.headers.get("auth-token").is_none());
    assert!(request.headers.get("auth_token").is_none());
}

#[tokio::test]
async fn download_refused_suggests_joining() {
    let env = isolate().await;
    let server = MockServer::start().await;
    serve_details(
        &server,
        details_with(json!([{"filename": "Train.csv", "filesize": 4}])),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(file_path("Train.csv")))
        .respond_with(ResponseTemplate::new(401).set_body_json(fixture("error_not_authorized")))
        .mount(&server)
        .await;

    let err = client(&server)
        .download(Some(COMP), env.dir.path(), DownloadOptions::new())
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            Error::Authentication {
                status: Some(401),
                ..
            }
        ),
        "{err:?}"
    );
    assert!(
        err.to_string().contains(&format!("zindi join {COMP}")),
        "{err}"
    );
    assert!(!env.dir.path().join("Train.csv").exists());
    assert!(!env.dir.path().join("Train.csv.part").exists());
}

#[tokio::test]
async fn download_only_archive_when_alone() {
    let env = isolate().await;
    let server = MockServer::start().await;
    serve_details(
        &server,
        details_with(json!([{"filename": "all.zip", "filesize": 3, "zip_of_everything": true}])),
    )
    .await;
    serve_file(&server, "all.zip", b"zip".to_vec()).await;

    let paths = client(&server)
        .download(Some(COMP), env.dir.path(), DownloadOptions::new())
        .await
        .unwrap();

    assert_eq!(paths, [env.dir.path().join("all.zip")]);
}

// -- submissions -----------------------------------------------------------------------------

fn submissions_path() -> String {
    format!("/competitions/{COMP}/submissions")
}

#[tokio::test]
async fn submit_uploads_file_and_comment() {
    let env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(submissions_path()))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("submission_created")))
        .expect(1)
        .mount(&server)
        .await;
    let file = env.dir.path().join("submission.csv");
    std::fs::write(&file, "ID,Yield\n1,2.5\n").unwrap();

    let sub = client(&server)
        .submit(&file, "first try", Some(COMP), false)
        .await
        .unwrap();

    assert_eq!(sub.id, "sub_d4");
    assert!(sub.is_pending());
    assert_eq!(sub.status_label(), "scoring…");
    let request = &requests_to(&server, "POST", &submissions_path()).await[0];
    let content_type = request
        .headers
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.starts_with("multipart/form-data; boundary="));
    let body = String::from_utf8_lossy(&request.body);
    assert!(body.contains("name=\"comment\"") && body.contains("first try"));
    assert!(body.contains("name=\"file\"; filename=\"submission.csv\""));
    assert!(body.contains("Content-Type: text/csv"));
    assert!(body.contains("ID,Yield\n1,2.5\n"));
    assert_eq!(request.headers.get("auth-token").unwrap(), "tok_test");
}

#[tokio::test]
async fn submit_and_wait_polls_until_scored() {
    let env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(submissions_path()))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("submission_created")))
        .mount(&server)
        .await;
    let pending = json!({"data": [fixture("submission_created")["data"]]});
    Mock::given(method("GET"))
        .and(path(submissions_path()))
        .respond_with(ResponseTemplate::new(200).set_body_json(pending))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(submissions_path()))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("submission_scored")))
        .mount(&server)
        .await;
    let file = env.dir.path().join("submission.csv");
    std::fs::write(&file, "x").unwrap();

    let sub = client(&server)
        .submit(&file, "", Some(COMP), true)
        .await
        .unwrap();

    assert!(sub.is_scored());
    assert_eq!(sub.public_score, Some(0.4211));
    assert_eq!(sub.score(), Some(0.4211));
    assert_eq!(
        requests_to(&server, "GET", &submissions_path()).await.len(),
        2
    );
}

#[tokio::test]
async fn wait_for_score_times_out() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(submissions_path()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"data": [fixture("submission_created")["data"]]})),
        )
        .mount(&server)
        .await;

    let err = client(&server)
        .wait_for_score("sub_d4", Some(COMP), Duration::ZERO, Duration::ZERO)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("not scored"), "{err}");
}

#[tokio::test]
async fn submit_missing_file() {
    let env = isolate().await;
    let server = MockServer::start().await;

    let err = client(&server)
        .submit(env.dir.path().join("nope.csv"), "", Some(COMP), false)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("not found"), "{err}");
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn submissions_and_statuses() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(submissions_path()))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("submissions")))
        .mount(&server)
        .await;
    let client = client(&server);

    let subs = client.submissions(Some(COMP), None).await.unwrap();

    let labels: Vec<_> = subs.iter().map(|s| s.status_label()).collect();
    assert_eq!(labels, ["scoring…", "scored", "failed"]);
    assert_eq!(subs[2].note(), "Missing column: Yield");
    assert_eq!(subs[1].note(), "lgbm baseline");
    assert_eq!(subs[1].score(), Some(0.4377));
    assert!(subs[1].selected);
    assert_eq!(
        client.submissions(Some(COMP), Some(1)).await.unwrap().len(),
        1
    );
    let requests = requests_to(&server, "GET", &submissions_path()).await;
    assert!(requests[0].url.query().unwrap().contains("per_page=1000"));
    assert!(requests[1].url.query().unwrap().contains("per_page=1"));
}

#[tokio::test]
async fn limits() {
    let _env = isolate().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("{}/limits", submissions_path())))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("submission_limits")))
        .mount(&server)
        .await;

    let limits = client(&server).limits(Some(COMP)).await.unwrap();

    assert_eq!(limits.remaining_today, Some(7));
}
