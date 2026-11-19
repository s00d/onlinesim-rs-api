//! `MockOnlineSim::start()` (feature `mock`): the same fake served over real local HTTP, reached through the
//! default reqwest transports. No test sleeps: `wait_code` runs with a zero interval.

use std::time::Duration;

use onlinesim_rs_api::mock::{MockOnlineSim, MockOptions, SmsScript};
use onlinesim_rs_api::{Client, FreeListOptions, StateOptions, WaitCodeOptions};

fn now() -> WaitCodeOptions {
    WaitCodeOptions {
        interval: Duration::ZERO,
        ..WaitCodeOptions::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn number_lifecycle_over_http() {
    let mock = MockOnlineSim::start().await;
    assert!(mock.base_url().starts_with("http://127.0.0.1:"));
    let client = mock.client().unwrap();
    let numbers = client.numbers();

    assert_eq!(numbers.price("google", 44).await.unwrap(), Some(0.2));
    let order = numbers.get_with_number("google", 44, &[]).await.unwrap();
    assert_eq!(order.number, "+44900000001");
    assert_eq!(
        numbers.state(StateOptions::default()).await.unwrap().len(),
        1
    );
    mock.add_sms_after(order.tzid, "Your code: 999-111", 1);
    assert_eq!(
        numbers.wait_code(order.tzid, now()).await.unwrap(),
        "999111"
    );
    assert_eq!(mock.balance(), 99.8);

    mock.fail_next_transport(502);
    assert!(matches!(
        client.user().balance().await.unwrap_err(),
        onlinesim_rs_api::Error::Transport {
            status: Some(502),
            ..
        }
    ));
    mock.fail_next_on("getNum.php", "NO_NUMBER");
    assert!(
        numbers
            .get("google", 44, &[])
            .await
            .unwrap_err()
            .is_no_number()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn account_rent_free_and_auth_over_http() {
    let mock = MockOnlineSim::start_with(MockOptions {
        api_key: "secret".into(),
        ..MockOptions::default()
    })
    .await;
    let client = mock.client().unwrap();

    client
        .user()
        .set_webhook_url("https://example.com/hook")
        .await
        .unwrap();
    assert_eq!(
        mock.webhook_url().as_deref(),
        Some("https://example.com/hook")
    );
    let error = client.user().set_webhook_url("nope").await.unwrap_err();
    assert_eq!(error.code(), Some("VALIDATION_ERROR"));

    let rent = client.rent().get(7, 1, false).await.unwrap();
    assert!(rent.extend.is_empty());
    client.rent().close(rent.tzid).await.unwrap();
    let free = client
        .free()
        .list(&FreeListOptions::default())
        .await
        .unwrap();
    assert!(free.numbers.contains_key("7459608320"));

    let keyless = Client::builder().base_url(mock.base_url()).build().unwrap();
    let error = keyless.user().balance().await.unwrap_err();
    assert_eq!(error.code(), Some("ERROR_WRONG_KEY"));
    assert_eq!(keyless.numbers().tariffs_one(44).await.unwrap().code, 44);
    assert_eq!(mock.requests().last().unwrap().path, "getNumbersStats.php");
}

#[tokio::test(flavor = "multi_thread")]
async fn blocking_client_and_legacy_script_over_http() {
    let mock = MockOnlineSim::start().await;
    mock.script_sms(SmsScript {
        service: "google".into(),
        code: "123456".into(),
        ..SmsScript::default()
    });
    let builder = mock.builder();
    let code = tokio::task::spawn_blocking(move || {
        let client = builder.build_blocking().unwrap();
        let tzid = client.numbers().get("google", 44, &[]).unwrap();
        client.numbers().wait_code(tzid, now()).unwrap()
    })
    .await
    .unwrap();
    assert_eq!(code, "123456");
}
