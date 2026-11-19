//! `mock::MockOnlineSim` end to end over real local HTTP (reqwest transports). No test sleeps: the
//! scripted SMS is already there on the first poll.

use onlinesim_rs_api::mock::{MockOnlineSim, SmsScript};
use onlinesim_rs_api::{Client, FreeListOptions, StateOptions, WaitCodeOptions};

#[tokio::test(flavor = "multi_thread")]
async fn number_lifecycle() {
    let mock = MockOnlineSim::start().await;
    mock.script_sms(SmsScript {
        service: "google".into(),
        code: "999111".into(),
        ..SmsScript::default()
    });
    let client = mock.client().unwrap();
    let numbers = client.numbers();

    assert_eq!(numbers.price("google", 1).await.unwrap(), Some(0.5));
    let order = numbers.get_with_number("google", 1, &[]).await.unwrap();
    assert_eq!(order.number, "+19001234567");
    assert_eq!(
        numbers.state(StateOptions::default()).await.unwrap().len(),
        1
    );

    let code = numbers
        .wait_code(
            order.tzid,
            WaitCodeOptions {
                keep_open: true,
                ..WaitCodeOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(code, "999111");
    numbers.close(order.tzid).await.unwrap();
    let error = numbers
        .state_one(order.tzid, Default::default())
        .await
        .unwrap_err();
    assert_eq!(error.code(), Some("ERROR_NO_OPERATIONS"));

    mock.fail_no_number("telegram");
    assert!(
        numbers
            .get("telegram", 1, &[])
            .await
            .unwrap_err()
            .is_no_number()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn account_rent_free_and_errors() {
    let mock = MockOnlineSim::start().await;
    mock.set_balance(10.0, 2.5);
    let client = mock.client().unwrap();

    let balance = client.user().balance().await.unwrap();
    assert_eq!((balance.balance, balance.zbalance), (10.0, 2.5));

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
    client.user().clear_webhook_url().await.unwrap();
    assert_eq!(client.user().profile().await.unwrap().webhook_url, None);

    let rent = client.rent().get(7, 1, false).await.unwrap();
    assert!(rent.extend.is_empty());
    assert_eq!(client.rent().state().await.unwrap().len(), 1);
    client.rent().close(rent.tzid).await.unwrap();
    assert_eq!(
        client.rent().tariffs_one(999).await.unwrap_err().code(),
        Some("NO_COUNTRY")
    );

    let free = client
        .free()
        .list(&FreeListOptions::default())
        .await
        .unwrap();
    assert!(free.numbers.is_empty());

    mock.fail_next("TRY_AGAIN_LATER");
    assert_eq!(
        client.user().balance().await.unwrap_err().code(),
        Some("TRY_AGAIN_LATER")
    );

    let keyless = Client::builder().base_url(mock.base_url()).build().unwrap();
    assert_eq!(
        keyless.user().balance().await.unwrap_err().code(),
        Some("ERROR_WRONG_KEY")
    );
    assert_eq!(keyless.numbers().tariffs_one(44).await.unwrap().code, 44);
}

#[tokio::test(flavor = "multi_thread")]
async fn blocking_client_on_the_mock() {
    let mock = MockOnlineSim::start().await;
    let base = mock.base_url();
    let code = tokio::task::spawn_blocking(move || {
        let client = onlinesim_rs_api::ClientBuilder::new()
            .api_key("k")
            .base_url(base)
            .build_blocking()
            .unwrap();
        let tzid = client.numbers().get("telegram", 1, &[]).unwrap();
        client
            .numbers()
            .wait_code(tzid, WaitCodeOptions::default())
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(code, "123456");
}
