//! Integration tests against the **published** [`onlinesim_rs_api::mock`] API.
//!
//! These are the same mocks consumers use after crates.io publish:
//!
//! ```toml
//! [dev-dependencies]
//! onlinesim-rs-api = { version = "0.1", features = ["mock"] }
//! ```
//!
//! Run: `cargo test --features mock` (add `blocking` for sync client coverage).

#![cfg(feature = "mock")]

use onlinesim_rs_api::mock::{MockOnlineSim, SmsScript};
use onlinesim_rs_api::{WaitCodeOptions, DEFAULT_COUNTRY};

#[tokio::test]
async fn balance_and_profile() {
    let mock = MockOnlineSim::start().await;
    mock.set_balance(42.0, 1.5, 9.0);
    let client = mock.client().unwrap();

    let balance = client.user().balance().await.unwrap();
    assert_eq!(balance.balance, 42.0);
    assert_eq!(balance.zbalance, 1.5);
    assert_eq!(balance.income, Some(9.0));

    let profile = client.user().profile().await.unwrap();
    assert_eq!(profile.id, 1);
    assert_eq!(profile.username.as_deref(), Some("mock"));
}

#[tokio::test]
async fn balance_error_maps_to_request() {
    let mock = MockOnlineSim::start().await;
    mock.fail_balance("ERROR_WRONG_KEY");
    let client = mock.client().unwrap();

    let err = client.user().balance().await.unwrap_err();
    match err {
        onlinesim_rs_api::Error::Request { code, message } => {
            assert_eq!(code, "ERROR_WRONG_KEY");
            assert_eq!(message, "wrong apikey");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn sms_flow_wait_code() {
    let mock = MockOnlineSim::start().await;
    mock.script_sms(SmsScript {
        service: "telegram".into(),
        number: "+19001234567".into(),
        country: DEFAULT_COUNTRY,
        code: "654321".into(),
        polls_before_code: 1,
        price: "12".into(),
    });

    let client = mock.client().unwrap();
    assert_eq!(client.numbers().price("telegram").await.unwrap(), "12");

    let ordered = client.numbers().get_with_number("telegram").await.unwrap();
    assert_eq!(ordered.number, "+19001234567");
    assert_eq!(ordered.country, DEFAULT_COUNTRY);
    assert_eq!(ordered.service.as_deref(), Some("telegram"));

    // First poll is still waiting (polls_before_code = 1).
    let waiting = client.numbers().state_one(ordered.tzid).await.unwrap();
    assert_eq!(waiting.response.as_deref(), Some("TZ_NUM_WAIT"));
    assert!(waiting.msg.is_none());
    assert_eq!(waiting.sum, Some(10.0));

    let code = client
        .numbers()
        .wait_code(
            ordered.tzid,
            WaitCodeOptions {
                interval_secs: 0,
                max_attempts: 5,
                ..WaitCodeOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(code, "654321");
}

#[tokio::test]
async fn wait_code_batches_multiple_tzids() {
    let mock = MockOnlineSim::start().await;
    mock.script_sms(SmsScript {
        service: "telegram".into(),
        number: "+19001111111".into(),
        code: "111111".into(),
        polls_before_code: 1,
        ..SmsScript::default()
    });
    mock.script_sms(SmsScript {
        service: "whatsapp".into(),
        number: "+19002222222".into(),
        code: "222222".into(),
        polls_before_code: 1,
        ..SmsScript::default()
    });

    let client = mock.client().unwrap();
    let a = client.numbers().get("telegram").await.unwrap();
    let b = client.numbers().get("whatsapp").await.unwrap();

    let opts = WaitCodeOptions {
        interval_secs: 0,
        max_attempts: 5,
        ..WaitCodeOptions::default()
    };
    let numbers = client.numbers();
    let (code_a, code_b) = tokio::join!(
        numbers.wait_code(a, opts.clone()),
        numbers.wait_code(b, opts),
    );
    assert_eq!(code_a.unwrap(), "111111");
    assert_eq!(code_b.unwrap(), "222222");
}

#[tokio::test]
async fn no_number() {
    let mock = MockOnlineSim::start().await;
    mock.fail_no_number("whatsapp");
    let client = mock.client().unwrap();
    let err = client.numbers().get("whatsapp").await.unwrap_err();
    assert!(matches!(
        err,
        onlinesim_rs_api::Error::NoNumber { code } if code == "NO_NUMBER"
    ));
}

#[tokio::test]
async fn free_and_rent() {
    let mock = MockOnlineSim::start().await;
    let client = mock.client().unwrap();

    let countries = client.free().countries().await.unwrap();
    assert_eq!(countries[0].country, DEFAULT_COUNTRY);

    let numbers = client.free().numbers(DEFAULT_COUNTRY).await.unwrap();
    assert!(!numbers.is_empty());

    let messages = client.free().messages(9001234567).await.unwrap();
    assert!(messages[0].text.contains("111222"));

    let rent = client.rent().get().await.unwrap();
    assert!(rent.tzid > 0);
    assert!(!client.rent().state().await.unwrap().is_empty());
    client.rent().close(rent.tzid).await.unwrap();
}

#[tokio::test]
async fn webhook_url_and_logs() {
    let mock = MockOnlineSim::start().await;
    let client = mock.client().unwrap();

    client
        .user()
        .set_webhook_url(Some("https://example.test/hook"))
        .await
        .unwrap();
    let profile = client.user().profile().await.unwrap();
    assert_eq!(
        profile.webhook_url.as_deref(),
        Some("https://example.test/hook")
    );

    let logs = client.user().webhook_logs(1).await.unwrap();
    assert_eq!(logs.data.len(), 1);
    assert_eq!(logs.data[0].status.as_deref(), Some("success"));

    client.user().clear_webhook_url().await.unwrap();
    let profile = client.user().profile().await.unwrap();
    assert!(profile.webhook_url.is_none());
}

#[tokio::test]
async fn tariffs() {
    let mock = MockOnlineSim::start().await;
    let client = mock.client().unwrap();
    assert!(client
        .numbers()
        .tariffs()
        .await
        .unwrap()
        .contains_key(&DEFAULT_COUNTRY.to_string()));
    assert_eq!(
        client.numbers().tariffs_one().await.unwrap().code,
        DEFAULT_COUNTRY
    );
}

#[test]
#[cfg(feature = "blocking")]
fn blocking_client_uses_same_mock() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let mock = rt.block_on(MockOnlineSim::start());
    mock.set_balance(10.0, 0.0, 1.0);

    let client = mock.client_blocking().unwrap();
    let balance = client.user().balance().unwrap();
    assert_eq!(balance.balance, 10.0);
    assert_eq!(balance.income, Some(1.0));

    mock.script_sms(SmsScript {
        code: "111222".into(),
        ..SmsScript::default()
    });
    let tzid = client.numbers().get("telegram").unwrap();
    let code = client
        .numbers()
        .wait_code(
            tzid,
            WaitCodeOptions {
                interval_secs: 0,
                ..WaitCodeOptions::default()
            },
        )
        .unwrap();
    assert_eq!(code, "111222");
}
