//! Blocking client: the same calls as the async one, and `wait_code_with` on a fake clock.

mod common;

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use common::{Reply, blocking, fixture, network_error, ok, q};
use onlinesim_rs_api::{Clock, Error, FreeListOptions, StateOptions, WaitCodeOptions};
use serde_json::json;

/// Fake clock: `sleep` only moves `now` and records the duration.
struct FakeClock {
    start: Instant,
    offset: Cell<Duration>,
    sleeps: RefCell<Vec<Duration>>,
}

impl FakeClock {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            offset: Cell::new(Duration::ZERO),
            sleeps: RefCell::default(),
        }
    }

    fn sleeps(&self) -> Vec<u64> {
        self.sleeps.borrow().iter().map(Duration::as_secs).collect()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.start + self.offset.get()
    }

    fn sleep(&self, duration: Duration) {
        self.sleeps.borrow_mut().push(duration);
        self.offset.set(self.offset.get() + duration);
    }
}

fn waiting() -> Reply {
    ok(json!([{ "tzid": 7, "response": "TZ_NUM_WAIT" }]))
}

fn answer(msg: &str) -> Reply {
    ok(json!([{ "tzid": 7, "response": "TZ_NUM_ANSWER", "msg": msg }]))
}

fn paths(fake: &common::Fake) -> Vec<String> {
    fake.calls().iter().map(|c| c.path().to_string()).collect()
}

#[test]
fn sections_send_the_same_requests_as_async() {
    let (client, fake) = blocking(vec![
        ok(json!({ "response": "1", "price": 0.36 })),
        ok(json!({ "response": 1, "tzid": 1520 })),
        ok(fixture("getState")),
        ok(fixture("rent_getRentState")),
        ok(fixture("getBalance")),
        ok(json!({ "response": "1" })),
        ok(fixture("getFreeList")),
    ]);
    assert_eq!(client.numbers().price("vkcom", 44).unwrap(), Some(0.36));
    assert_eq!(client.numbers().get("google", 44, &["4470"]).unwrap(), 1520);
    assert_eq!(
        client
            .numbers()
            .state(StateOptions::default())
            .unwrap()
            .len(),
        2
    );
    assert_eq!(client.rent().state().unwrap()[0].sum, 5.14);
    assert_eq!(client.user().balance().unwrap().balance, 1442.33);
    client.user().clear_webhook_url().unwrap();
    assert_eq!(
        client
            .free()
            .list(&FreeListOptions::default())
            .unwrap()
            .countries
            .len(),
        2
    );

    let calls = fake.calls();
    assert_eq!(
        calls[1].query(),
        q(&[("service", "google"), ("country", "44"), ("reject", "4470")])
    );
    assert_eq!(calls[1].header("Authorization"), Some("Bearer test-key"));
    assert_eq!(calls[3].query(), q(&[("pagination", "0"), ("number", "1")]));
    assert_eq!(
        calls[5].body,
        Some(json!({ "profile": { "webhook_url": null } }))
    );
    assert_eq!(calls[6].path(), "/api/getFreeList");
}

#[test]
fn maps_errors_like_async() {
    let (client, _) = blocking(vec![
        Reply::Json(456, json!({ "response": "ERROR_WRONG_KEY" })),
        Reply::Raw(502, "<html>"),
    ]);
    assert_eq!(
        client.user().balance().unwrap_err().code(),
        Some("ERROR_WRONG_KEY")
    );
    assert!(matches!(
        client.user().balance().unwrap_err(),
        Error::Transport {
            status: Some(502),
            ..
        }
    ));
}

#[test]
fn wait_code_returns_the_code_and_closes() {
    let (client, fake) = blocking(vec![
        waiting(),
        waiting(),
        answer("195895"),
        ok(json!({ "response": 1 })),
    ]);
    let clock = FakeClock::new();
    let code = client
        .numbers()
        .wait_code_with(7, WaitCodeOptions::default(), &clock)
        .unwrap();
    assert_eq!(code, "195895");
    assert_eq!(clock.sleeps(), [5, 5]);
    assert_eq!(
        paths(&fake),
        [
            "/api/getState.php",
            "/api/getState.php",
            "/api/getState.php",
            "/api/setOperationOk.php"
        ]
    );
}

#[test]
fn wait_code_keep_open_since_full_message() {
    let (client, fake) = blocking(vec![
        answer("old"),
        answer("Your code is 2991"),
        ok(json!({ "response": "1" })),
    ]);
    let clock = FakeClock::new();
    let options = WaitCodeOptions {
        keep_open: true,
        since: Some("old".into()),
        full_message: true,
        interval: Duration::from_secs(1),
        ..WaitCodeOptions::default()
    };
    let code = client.numbers().wait_code_with(7, options, &clock).unwrap();
    assert_eq!(code, "Your code is 2991");
    assert_eq!(clock.sleeps(), [1]);
    assert_eq!(
        paths(&fake),
        [
            "/api/getState.php",
            "/api/getState.php",
            "/api/setOperationRevise.php"
        ]
    );
    assert_eq!(fake.calls()[1].query()["message_to_code"], "0");
}

#[test]
fn wait_code_raises_the_finished_state_at_once() {
    for status in ["TZ_OVER_OK", "ERROR_NO_ITEMS"] {
        let (client, fake) = blocking(vec![ok(json!([{ "tzid": 7, "response": status }]))]);
        let clock = FakeClock::new();
        let error = client
            .numbers()
            .wait_code_with(7, WaitCodeOptions::default(), &clock)
            .unwrap_err();
        assert_eq!(error.code(), Some(status));
        assert!(clock.sleeps().is_empty());
        assert_eq!(fake.calls().len(), 1);
    }
}

#[test]
fn wait_code_times_out_and_cuts_the_last_sleep_to_the_deadline() {
    let (client, fake) = blocking(vec![waiting(), waiting(), waiting(), waiting()]);
    let clock = FakeClock::new();
    let options = WaitCodeOptions {
        interval: Duration::from_secs(4),
        timeout: Duration::from_secs(10),
        ..WaitCodeOptions::default()
    };
    let error = client
        .numbers()
        .wait_code_with(7, options, &clock)
        .unwrap_err();
    assert!(matches!(error, Error::Timeout(_)), "{error:?}");
    assert_eq!(clock.sleeps(), [4, 4, 2]);
    assert_eq!(fake.calls().len(), 4);
}

#[test]
fn wait_code_retries_transport_errors_and_raises_api_errors() {
    let (client, _) = blocking(vec![
        network_error("reset"),
        answer("42"),
        ok(json!({ "response": 1 })),
    ]);
    let clock = FakeClock::new();
    let code = client
        .numbers()
        .wait_code_with(7, WaitCodeOptions::default(), &clock);
    assert_eq!(code.unwrap(), "42");
    assert_eq!(clock.sleeps(), [5]);

    let (client, fake) = blocking(vec![ok(json!({ "response": "ERROR_NO_OPERATIONS" }))]);
    let error = client
        .numbers()
        .wait_code_with(7, WaitCodeOptions::default(), &FakeClock::new())
        .unwrap_err();
    assert_eq!(error.code(), Some("ERROR_NO_OPERATIONS"));
    assert_eq!(fake.calls().len(), 1);
}

#[test]
fn wait_code_on_the_system_clock_returns_without_sleeping_when_the_code_is_there() {
    let (client, _) = blocking(vec![answer("1"), ok(json!({ "response": 1 }))]);
    assert_eq!(
        client
            .numbers()
            .wait_code(7, WaitCodeOptions::default())
            .unwrap(),
        "1"
    );
}
