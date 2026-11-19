//! Async `wait_code` on tokio's paused clock: sleeps advance virtual time only, nothing really waits.

mod common;

use std::time::Duration;

use common::{Reply, client, network_error, ok};
use onlinesim_rs_api::{Error, WaitCodeOptions};
use serde_json::json;
use tokio::time::Instant;

fn waiting() -> Reply {
    ok(json!([{ "tzid": 7, "response": "TZ_NUM_WAIT" }]))
}

fn answer(msg: &str) -> Reply {
    ok(json!([{ "tzid": 7, "response": "TZ_NUM_ANSWER", "msg": msg }]))
}

fn closed() -> Reply {
    ok(json!({ "response": 1, "tzid": 7 }))
}

fn paths(fake: &common::Fake) -> Vec<String> {
    fake.calls().iter().map(|c| c.path().to_string()).collect()
}

#[tokio::test(start_paused = true)]
async fn returns_the_code_once_it_arrives_and_closes_the_operation() {
    let (client, fake) = client(vec![waiting(), waiting(), answer("195895"), closed()]);
    let start = Instant::now();
    let code = client
        .numbers()
        .wait_code(7, WaitCodeOptions::default())
        .await
        .unwrap();
    assert_eq!(code, "195895");
    assert_eq!(start.elapsed(), Duration::from_secs(10));
    assert_eq!(
        paths(&fake),
        [
            "/api/getState.php",
            "/api/getState.php",
            "/api/getState.php",
            "/api/setOperationOk.php"
        ]
    );
    let calls = fake.calls();
    assert_eq!(calls[0].query()["tzid"], "7");
    assert_eq!(calls[0].query()["message_to_code"], "1");
    assert_eq!(calls[3].query()["tzid"], "7");
}

#[tokio::test(start_paused = true)]
async fn keep_open_calls_next_since_skips_the_previous_code_full_message_asks_for_text() {
    let (client, fake) = client(vec![
        answer("old"),
        answer("Your code is 2991"),
        ok(json!({ "response": "1" })),
    ]);
    let start = Instant::now();
    let options = WaitCodeOptions {
        keep_open: true,
        since: Some("old".into()),
        full_message: true,
        interval: Duration::from_secs(1),
        ..WaitCodeOptions::default()
    };
    let code = client.numbers().wait_code(7, options).await.unwrap();
    assert_eq!(code, "Your code is 2991");
    assert_eq!(start.elapsed(), Duration::from_secs(1));
    assert_eq!(
        paths(&fake),
        [
            "/api/getState.php",
            "/api/getState.php",
            "/api/setOperationRevise.php"
        ]
    );
    assert_eq!(fake.calls()[0].query()["message_to_code"], "0");
}

#[tokio::test(start_paused = true)]
async fn raises_the_finished_state_as_api_error_at_once() {
    for status in ["TZ_OVER_OK", "ERROR_NO_ITEMS"] {
        let (client, fake) = client(vec![ok(json!([{ "tzid": 7, "response": status }]))]);
        let start = Instant::now();
        let error = client
            .numbers()
            .wait_code(7, WaitCodeOptions::default())
            .await
            .unwrap_err();
        assert_eq!(error.code(), Some(status));
        assert_eq!(start.elapsed(), Duration::ZERO);
        assert_eq!(fake.calls().len(), 1);
    }
}

#[tokio::test(start_paused = true)]
async fn a_finished_operation_that_still_carries_a_new_code_returns_it() {
    let (client, _) = client(vec![
        ok(json!([{ "tzid": 7, "response": "TZ_OVER_OK", "msg": "1234" }])),
        closed(),
    ]);
    let code = client
        .numbers()
        .wait_code(7, WaitCodeOptions::default())
        .await;
    assert_eq!(code.unwrap(), "1234");
}

#[tokio::test(start_paused = true)]
async fn raises_timeout_when_no_code_arrives_and_cuts_the_last_sleep_to_the_deadline() {
    let (client, fake) = client(vec![waiting(), waiting(), waiting(), waiting()]);
    let start = Instant::now();
    let options = WaitCodeOptions {
        interval: Duration::from_secs(4),
        timeout: Duration::from_secs(10),
        ..WaitCodeOptions::default()
    };
    let error = client.numbers().wait_code(7, options).await.unwrap_err();
    assert!(matches!(error, Error::Timeout(_)), "{error:?}");
    assert_eq!(start.elapsed(), Duration::from_secs(10));
    assert_eq!(fake.calls().len(), 4);
}

#[tokio::test(start_paused = true)]
async fn retries_a_transient_transport_error() {
    let (client, fake) = client(vec![
        network_error("Network error: reset"),
        answer("42"),
        closed(),
    ]);
    let code = client
        .numbers()
        .wait_code(7, WaitCodeOptions::default())
        .await;
    assert_eq!(code.unwrap(), "42");
    assert_eq!(
        paths(&fake),
        [
            "/api/getState.php",
            "/api/getState.php",
            "/api/setOperationOk.php"
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn raises_any_api_error_at_once() {
    let (client, fake) = client(vec![ok(json!({ "response": "ERROR_NO_OPERATIONS" }))]);
    let error = client
        .numbers()
        .wait_code(7, WaitCodeOptions::default())
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("ERROR_NO_OPERATIONS"));
    assert_eq!(fake.calls().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_close_error_is_returned() {
    let (client, _) = client(vec![
        answer("1"),
        ok(json!({ "response": "NO_COMPLETE_TZID" })),
    ]);
    let error = client
        .numbers()
        .wait_code(7, WaitCodeOptions::default())
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("NO_COMPLETE_TZID"));
}
