//! Tests for the built-in [`onlinesim_rs_api::mock`] server.

#![cfg(feature = "mock")]

use onlinesim_rs_api::mock::{MockOnlineSim, SmsScript};
use onlinesim_rs_api::WaitCodeOptions;

#[tokio::test]
async fn mock_balance_and_profile() {
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
async fn mock_sms_flow_wait_code() {
    let mock = MockOnlineSim::start().await;
    mock.script_sms(SmsScript {
        service: "telegram".into(),
        number: "+79001234567".into(),
        country: 7,
        code: "654321".into(),
        polls_before_code: 1,
        price: "12".into(),
    });

    let client = mock.client().unwrap();
    let price = client.numbers().price("telegram").await.unwrap();
    assert_eq!(price, "12");

    let ordered = client.numbers().get_with_number("telegram").await.unwrap();
    assert_eq!(ordered.number, "+79001234567");

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
async fn mock_no_number() {
    let mock = MockOnlineSim::start().await;
    mock.fail_no_number("whatsapp");
    let client = mock.client().unwrap();
    let err = client.numbers().get("whatsapp").await.unwrap_err();
    assert!(matches!(err, onlinesim_rs_api::Error::NoNumber { .. }));
}

#[tokio::test]
async fn mock_free_and_rent() {
    let mock = MockOnlineSim::start().await;
    let client = mock.client().unwrap();

    let countries = client.free().countries().await.unwrap();
    assert!(!countries.is_empty());

    let numbers = client.free().numbers(7).await.unwrap();
    assert!(!numbers.is_empty());

    let messages = client.free().messages(9001234567).await.unwrap();
    assert!(messages[0].text.contains("111222"));

    let rent = client.rent().get().await.unwrap();
    assert!(rent.tzid > 0);
    let list = client.rent().state().await.unwrap();
    assert!(!list.is_empty());
    client.rent().close(rent.tzid).await.unwrap();
}

#[tokio::test]
async fn mock_tariffs() {
    let mock = MockOnlineSim::start().await;
    let client = mock.client().unwrap();
    let all = client.numbers().tariffs().await.unwrap();
    assert!(all.contains_key("7"));
    let one = client.numbers().tariffs_one().await.unwrap();
    assert_eq!(one.code, 7);
}
