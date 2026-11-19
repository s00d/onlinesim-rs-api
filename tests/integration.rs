//! Opt-in integration test against a LOCAL backend. Skipped unless both are set:
//!   ONLINESIM_IT_BASE=https://on.test/api/  ONLINESIM_IT_KEY=<apikey>
//! With ONLINESIM_IT_MOCK=1 it also runs the number lifecycle on the provider-free `test` driver
//! (`spec/verify/backend_mock.php setup` first, a key of a user without webhook_url, teardown after).

use std::time::Duration;

use onlinesim_rs_api::{
    Client, Error, FreeListOptions, StateOneOptions, StateOptions, WaitCodeOptions,
};

struct Env {
    base: String,
    key: String,
}

fn env() -> Option<Env> {
    let base = std::env::var("ONLINESIM_IT_BASE")
        .ok()
        .filter(|v| !v.is_empty())?;
    let key = std::env::var("ONLINESIM_IT_KEY")
        .ok()
        .filter(|v| !v.is_empty())?;
    let host = reqwest::Url::parse(&base)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    assert!(
        host == "localhost"
            || host == "127.0.0.1"
            || host.ends_with(".test")
            || host.ends_with(".localhost"),
        "refusing non-local integration host {host}"
    );
    Some(Env { base, key })
}

/// Test-only transport: local backends use a self-signed certificate. Not part of the SDK API.
fn insecure() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap()
}

fn client(env: &Env, key: &str) -> Client {
    Client::builder()
        .api_key(key)
        .base_url(&env.base)
        .build_with(insecure())
        .unwrap()
}

#[tokio::test]
async fn read_only() {
    let Some(env) = env() else {
        eprintln!("skipped: ONLINESIM_IT_BASE / ONLINESIM_IT_KEY not set");
        return;
    };
    let client = client(&env, &env.key);

    let balance = client.user().balance().await.unwrap();
    assert!(balance.balance.is_finite() && balance.zbalance.is_finite());
    assert!(client.user().profile().await.unwrap().id > 0);

    let uk = client.numbers().tariffs_one(44).await.unwrap();
    assert_eq!(uk.code, 44);
    client
        .numbers()
        .state(StateOptions::default())
        .await
        .unwrap();

    assert_eq!(client.rent().tariffs_one(7).await.unwrap().code, 7);
    for rent in client.rent().state().await.unwrap() {
        assert!(rent.sum.is_finite());
    }

    client.user().payment_history(1).await.unwrap();
    client.user().webhook_logs(1).await.unwrap();
    client
        .free()
        .list(&FreeListOptions::default())
        .await
        .unwrap();

    let error = self::client(&env, "wrong-key")
        .user()
        .balance()
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("ERROR_WRONG_KEY"));
}

#[tokio::test]
async fn number_lifecycle_on_the_test_driver() {
    let Some(env) = env() else { return };
    if std::env::var("ONLINESIM_IT_MOCK").as_deref() != Ok("1") {
        eprintln!("skipped: ONLINESIM_IT_MOCK != 1");
        return;
    }
    let client = client(&env, &env.key);
    let numbers = client.numbers();
    const COUNTRY: u32 = 44;
    const SERVICE: &str = "google";

    assert!(numbers.price(SERVICE, COUNTRY).await.unwrap().is_some());
    let tzid = numbers.get(SERVICE, COUNTRY, &[]).await.unwrap();
    let order = numbers
        .get_with_number(SERVICE, COUNTRY, &[])
        .await
        .unwrap();
    assert!(order.number.trim_start_matches('+').starts_with("44"));
    assert_eq!(order.country, i64::from(COUNTRY));

    // The list itself: the backend caches getState for 5 s per user+params, and read_only (run in
    // parallel with the same key) may have just cached it without the new operations. So only its
    // shape is checked here; the new operations are looked up by tzid (a separate cache key).
    numbers.state(StateOptions::default()).await.unwrap();
    for id in [tzid, order.tzid] {
        let op = numbers
            .state_one(id, StateOneOptions::default())
            .await
            .unwrap();
        assert_eq!(op.tzid, id);
    }

    // The test driver answers NO_NUMBER to any reject list: proves `reject` is sent.
    assert!(
        numbers
            .get(SERVICE, COUNTRY, &["4470"])
            .await
            .unwrap_err()
            .is_no_number()
    );

    // No SMS yet: the guard interval keeps it open.
    let error = numbers.close(order.tzid).await.unwrap_err();
    assert_eq!(error.code(), Some("NO_COMPLETE_TZID"));

    // Inject an SMS through the test driver callback (local backends only, no auth).
    let url = reqwest::Url::parse(&env.base)
        .unwrap()
        .join("sms_in_getaways/test")
        .unwrap();
    let res = insecure()
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(
            serde_json::json!({ "id": tzid, "phoneFrom": "VK", "text": "VK: 195895 is your code" })
                .to_string(),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);

    let options = WaitCodeOptions {
        interval: Duration::from_secs(1),
        timeout: Duration::from_secs(30),
        ..WaitCodeOptions::default()
    };
    assert_eq!(numbers.wait_code(tzid, options).await.unwrap(), "195895");

    let user = client.user();
    user.set_webhook_url("https://example.com/hook")
        .await
        .unwrap();
    assert_eq!(
        user.profile().await.unwrap().webhook_url.as_deref(),
        Some("https://example.com/hook")
    );
    let error = user.set_webhook_url("not a url").await.unwrap_err();
    assert!(
        matches!(&error, Error::Api(e) if e.code == "VALIDATION_ERROR"),
        "{error:?}"
    );
    user.clear_webhook_url().await.unwrap();
    assert_eq!(user.profile().await.unwrap().webhook_url, None);
}
