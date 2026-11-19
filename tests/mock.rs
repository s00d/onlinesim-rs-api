//! `mock::MockOnlineSim` (in-process transport) through the real clients: every endpoint, every control
//! helper. No sockets, no clock: async tests run on tokio's paused clock, `wait_code` with a zero interval.

use std::collections::BTreeMap;
use std::time::Duration;

use onlinesim_rs_api::mock::{
    MOCK_BASE_URL, MockOnlineSim, MockOptions, MockRentTariff, SmsScript,
};
use onlinesim_rs_api::types::{OperationMsg, OperationStatus};
use onlinesim_rs_api::{
    Client, Error, FreeListOptions, Method, OrderBy, StateOneOptions, StateOptions, WaitCodeOptions,
};

fn mock_with(balance: f64) -> (MockOnlineSim, Client) {
    let mock = MockOnlineSim::new(MockOptions {
        balance,
        ..MockOptions::default()
    });
    let client = mock.client().unwrap();
    (mock, client)
}

fn now() -> WaitCodeOptions {
    WaitCodeOptions {
        interval: Duration::ZERO,
        ..WaitCodeOptions::default()
    }
}

fn code_of(error: Error) -> String {
    error.code().unwrap_or("<no code>").to_string()
}

// ----- auth and transport -----

#[tokio::test(start_paused = true)]
async fn wrong_or_missing_key_is_456_error_wrong_key() {
    let mock = MockOnlineSim::default();
    for builder in [Client::builder().api_key("wrong"), Client::builder()] {
        let client = builder.build_with(mock.transport()).unwrap();
        let error = client.user().balance().await.unwrap_err();
        assert_eq!(code_of(error), "ERROR_WRONG_KEY");
    }
}

#[tokio::test(start_paused = true)]
async fn tariffs_and_free_numbers_need_no_key() {
    let mock = MockOnlineSim::default();
    let keyless = Client::builder().build_with(mock.transport()).unwrap();
    assert_eq!(keyless.numbers().tariffs_one(44).await.unwrap().code, 44);
    assert_eq!(keyless.rent().tariffs_one(7).await.unwrap().code, 7);
    let free = keyless
        .free()
        .list(&FreeListOptions::default())
        .await
        .unwrap();
    assert_eq!(free.countries.len(), 2);
}

#[tokio::test(start_paused = true)]
async fn transport_of_a_hand_built_client_with_any_base_url() {
    let mock = MockOnlineSim::default();
    let client = Client::builder()
        .api_key("mock-key")
        .base_url("https://example.invalid/some/api")
        .build_with(mock.transport())
        .unwrap();
    assert_eq!(client.user().balance().await.unwrap().balance, 100.0);
    assert_eq!(mock.requests()[0].path, "getBalance.php");
    assert_eq!(mock.base_url(), MOCK_BASE_URL);
}

#[tokio::test(start_paused = true)]
async fn unknown_endpoint_is_a_non_json_404() {
    let mock = MockOnlineSim::default();
    let client = mock
        .builder()
        .base_url("https://mock.onlinesim.invalid/nope/")
        .build_with(mock.transport())
        .unwrap();
    match client.user().balance().await.unwrap_err() {
        Error::Transport { status, .. } => assert_eq!(status, Some(404)),
        other => panic!("expected a transport error, got {other:?}"),
    }
}

// ----- numbers -----

#[tokio::test(start_paused = true)]
async fn price() {
    let (_mock, client) = mock_with(100.0);
    let numbers = client.numbers();
    assert_eq!(numbers.price("google", 44).await.unwrap(), Some(0.2));
    assert_eq!(numbers.price("nope", 44).await.unwrap(), None);
    let error = numbers.price("google", 999).await.unwrap_err();
    assert_eq!(code_of(error), "UNDEFINED_COUNTRY");
}

#[tokio::test(start_paused = true)]
async fn get_freezes_the_price_with_deterministic_numbers() {
    let (mock, client) = mock_with(10.0);
    let numbers = client.numbers();
    assert_eq!(numbers.get("google", 44, &[]).await.unwrap(), 1);
    let order = numbers.get_with_number("vkcom", 7, &[]).await.unwrap();
    assert_eq!(order.tzid, 2);
    assert_eq!(order.number, "+7900000002");
    assert_eq!(order.country, 7);
    assert_eq!(order.service, "vkcom");
    assert_eq!(order.time, 1200);

    let balance = client.user().balance().await.unwrap();
    assert_eq!((balance.balance, balance.zbalance), (10.0, 0.7));
    assert_eq!(mock.operations()[0].number, "+44900000001");
    assert_eq!(mock.operations()[0].status, OperationStatus::TzNumWait);
}

#[tokio::test(start_paused = true)]
async fn get_errors() {
    let (_mock, client) = mock_with(0.25);
    let numbers = client.numbers();
    let unknown_country = numbers.get("google", 999, &[]).await.unwrap_err();
    assert_eq!(code_of(unknown_country), "UNDEFINED_COUNTRY");
    let unknown_service = numbers.get("nope", 44, &[]).await.unwrap_err();
    assert_eq!(code_of(unknown_service), "ERROR_NO_SERVICE");
    let low = numbers.get("vkcom", 44, &[]).await.unwrap_err();
    assert_eq!(code_of(low), "WARNING_LOW_BALANCE");
    // The next number is +44900000001: a matching prefix rejects it.
    let rejected = numbers.get("google", 44, &["4490"]).await.unwrap_err();
    assert!(rejected.is_no_number());
    assert_eq!(numbers.get("google", 44, &["4470"]).await.unwrap(), 1);
}

#[tokio::test(start_paused = true)]
async fn state_lifecycle() {
    let (mock, client) = mock_with(100.0);
    let numbers = client.numbers();
    assert!(
        numbers
            .state(StateOptions::default())
            .await
            .unwrap()
            .is_empty()
    );
    let missing = numbers.state_one(1, StateOneOptions::default()).await;
    assert_eq!(code_of(missing.unwrap_err()), "ERROR_NO_OPERATIONS");

    let first = numbers.get("google", 44, &[]).await.unwrap();
    let second = numbers.get("vkcom", 44, &[]).await.unwrap();
    let op = numbers
        .state_one(first, StateOneOptions::default())
        .await
        .unwrap();
    assert_eq!(op.response, OperationStatus::TzNumWait);
    assert_eq!(op.msg, None);
    assert_eq!(op.guard_interval_remaining_seconds, Some(0));

    mock.add_sms(first, "Your code: 123-456");
    let op = numbers
        .state_one(first, StateOneOptions::default())
        .await
        .unwrap();
    assert_eq!(op.response, OperationStatus::TzNumAnswer);
    assert_eq!(op.msg, Some(OperationMsg::Text("123456".into())));
    let full = StateOneOptions {
        message_to_code: false,
        ..StateOneOptions::default()
    };
    let op = numbers.state_one(first, full).await.unwrap();
    assert_eq!(
        op.msg,
        Some(OperationMsg::Text("Your code: 123-456".into()))
    );

    let desc = StateOptions {
        order_by: OrderBy::Desc,
        ..StateOptions::default()
    };
    let list = numbers.state(desc).await.unwrap();
    assert_eq!(
        list.iter().map(|o| o.tzid).collect::<Vec<_>>(),
        [second, first]
    );

    numbers.close(first).await.unwrap();
    let op = numbers
        .state_one(first, StateOneOptions::default())
        .await
        .unwrap();
    assert_eq!(op.response, OperationStatus::TzOverOk);
    assert_eq!(op.time, Some(0));
}

#[tokio::test(start_paused = true)]
async fn full_text_without_code_next_and_msg_list() {
    let (mock, client) = mock_with(100.0);
    let numbers = client.numbers();
    let tzid = numbers.get("google", 44, &[]).await.unwrap();
    mock.add_sms(tzid, "Welcome aboard");
    let op = numbers
        .state_one(tzid, StateOneOptions::default())
        .await
        .unwrap();
    assert_eq!(op.msg, Some(OperationMsg::Text("Welcome aboard".into())));

    numbers.next(tzid).await.unwrap();
    let op = numbers
        .state_one(tzid, StateOneOptions::default())
        .await
        .unwrap();
    assert_eq!((op.response, op.msg), (OperationStatus::TzNumWait, None));
    mock.add_sms_from(tzid, "code 4321", Some("Other"));
    let op = numbers
        .state_one(tzid, StateOneOptions::default())
        .await
        .unwrap();
    assert_eq!(op.msg, Some(OperationMsg::Text("4321".into())));

    let list = StateOneOptions {
        msg_list: true,
        ..StateOneOptions::default()
    };
    match numbers.state_one(tzid, list).await.unwrap().msg {
        Some(OperationMsg::List(all)) => {
            let msgs: Vec<_> = all
                .iter()
                .map(|s| (s.service.as_str(), s.msg.as_deref().unwrap_or_default()))
                .collect();
            assert_eq!(msgs, [("google", "Welcome aboard"), ("Other", "4321")]);
        }
        other => panic!("expected a list, got {other:?}"),
    }
    // setOperationRevise always succeeds, even for unknown tzids.
    numbers.next(999).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn close_spends_or_refunds_and_ban_after_sms_answers_zero() {
    let (mock, client) = mock_with(10.0);
    let numbers = client.numbers();
    let refunded = numbers.get("google", 44, &[]).await.unwrap();
    numbers.close(refunded).await.unwrap();
    assert_eq!(mock.balance(), 10.0);
    let closed_again = numbers.close(refunded).await.unwrap_err();
    assert_eq!(code_of(closed_again), "ERROR_WRONG_TZID");
    assert_eq!(
        code_of(numbers.close(999).await.unwrap_err()),
        "ERROR_WRONG_TZID"
    );

    let spent = numbers.get("google", 44, &[]).await.unwrap();
    mock.add_sms(spent, "1234");
    let ban = numbers.ban(spent).await.unwrap_err();
    assert_eq!(code_of(ban), "0");
    numbers.close(spent).await.unwrap();
    assert_eq!(mock.balance(), 9.8);
    let balance = client.user().balance().await.unwrap();
    assert_eq!((balance.balance, balance.zbalance), (9.8, 0.0));

    let banned = numbers.get("google", 44, &[]).await.unwrap();
    numbers.ban(banned).await.unwrap();
    assert_eq!(mock.balance(), 9.8);
}

#[tokio::test(start_paused = true)]
async fn guard_interval_blocks_closing_without_sms() {
    let mock = MockOnlineSim::new(MockOptions {
        guard_interval: 120,
        ..MockOptions::default()
    });
    let client = mock.client().unwrap();
    let numbers = client.numbers();
    let tzid = numbers.get("google", 44, &[]).await.unwrap();
    let op = numbers
        .state_one(tzid, StateOneOptions::default())
        .await
        .unwrap();
    assert_eq!(op.guard_interval_remaining_seconds, Some(120));
    assert_eq!(
        code_of(numbers.close(tzid).await.unwrap_err()),
        "NO_COMPLETE_TZID"
    );
    mock.add_sms(tzid, "Code 5555");
    numbers.close(tzid).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn wait_code_with_add_sms_after_and_zero_interval() {
    let (mock, client) = mock_with(10.0);
    let start = tokio::time::Instant::now();
    let tzid = client.numbers().get("google", 44, &[]).await.unwrap();
    mock.add_sms_after(tzid, "Your Google code: 123-456", 2);
    let code = client.numbers().wait_code(tzid, now()).await.unwrap();
    assert_eq!(code, "123456");
    assert_eq!(start.elapsed(), Duration::ZERO);
    let polls = mock
        .requests()
        .iter()
        .filter(|r| r.path == "getState.php")
        .count();
    assert_eq!(polls, 3);
    assert_eq!(mock.operations()[0].status, OperationStatus::TzOverOk);
    assert_eq!(mock.balance(), 9.8);
}

#[tokio::test(start_paused = true)]
async fn wait_code_keep_open_since_and_polls_of_the_whole_list() {
    let (mock, client) = mock_with(10.0);
    let numbers = client.numbers();
    let tzid = numbers.get("google", 44, &[]).await.unwrap();
    mock.add_sms(tzid, "1111");
    let keep = WaitCodeOptions {
        keep_open: true,
        ..now()
    };
    assert_eq!(numbers.wait_code(tzid, keep.clone()).await.unwrap(), "1111");

    mock.add_sms_after(tzid, "2222", 1);
    numbers.state(StateOptions::default()).await.unwrap(); // a listing poll counts
    let since = WaitCodeOptions {
        since: Some("1111".into()),
        ..keep
    };
    assert_eq!(numbers.wait_code(tzid, since).await.unwrap(), "2222");
    assert_eq!(mock.operations()[0].messages.len(), 2);
}

#[tokio::test(start_paused = true)]
async fn numbers_tariffs_from_prices() {
    let (_mock, client) = mock_with(100.0);
    let all = client.numbers().tariffs().await.unwrap();
    let mut codes: Vec<_> = all.keys().cloned().collect();
    codes.sort();
    assert_eq!(codes, ["44", "7"]);
    let uk = client.numbers().tariffs_one(44).await.unwrap();
    let google = &uk.services["service_google"];
    assert_eq!(
        (google.price, google.count, google.slug.as_str()),
        (0.2, Some(100), "google")
    );
    let error = client.numbers().tariffs_one(999).await.unwrap_err();
    assert_eq!(code_of(error), "NO_COUNTRY");
}

// ----- rent -----

#[tokio::test(start_paused = true)]
async fn rent_lifecycle() {
    let (mock, client) = mock_with(10.0);
    let rent = client.rent();
    let item = rent.get(7, 1, true).await.unwrap();
    assert_eq!((item.tzid, item.number.as_str()), (1, "900000001"));
    assert_eq!((item.sum, item.days, item.time), (1.0, 1, 1440));
    assert_eq!(item.time_seconds, Some(86_400));
    assert_eq!(item.extend.get("3"), Some(&2.5));
    assert_eq!(mock.balance(), 9.0);

    let extended = rent.extend(item.tzid, 3).await.unwrap();
    assert_eq!((extended.days, extended.sum), (4, 3.5));
    assert_eq!(mock.balance(), 6.5);
    assert_eq!(rent.state().await.unwrap().len(), 1);
    assert_eq!(rent.state_one(item.tzid).await.unwrap().days, 4);

    let fixed = rent.get(7, 1, false).await.unwrap();
    assert!(fixed.extend.is_empty());
    let refused = rent.extend(fixed.tzid, 1).await.unwrap_err();
    assert_eq!(code_of(refused), "TRY_AGAIN_LATER");

    rent.close(item.tzid).await.unwrap();
    assert_eq!(mock.balance(), 5.5); // no refund
    assert_eq!(
        code_of(rent.close(item.tzid).await.unwrap_err()),
        "ERROR_WRONG_TZID"
    );
    assert_eq!(mock.rents().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn rent_errors() {
    let (_mock, client) = mock_with(2.0);
    let rent = client.rent();
    assert_eq!(
        code_of(rent.get(44, 1, true).await.unwrap_err()),
        "NO_COUNTRY"
    );
    assert_eq!(
        code_of(rent.get(7, 2, true).await.unwrap_err()),
        "UNDEFINED_DAYS"
    );
    assert_eq!(
        code_of(rent.get(7, 3, true).await.unwrap_err()),
        "WARNING_LOW_BALANCE"
    );
}

#[tokio::test(start_paused = true)]
async fn rent_sms_and_tariffs() {
    let mock = MockOnlineSim::new(MockOptions {
        rent_tariffs: BTreeMap::from([(
            44,
            MockRentTariff {
                days: BTreeMap::from([(1, 2.0)]),
                extend: false,
            },
        )]),
        ..MockOptions::default()
    });
    let client = mock.client().unwrap();
    let item = client.rent().get(44, 1, true).await.unwrap();
    assert!(item.extend.is_empty()); // the tariff is not extendable
    mock.add_sms(item.tzid, "first 1111");
    mock.add_sms_after(item.tzid, "second 2222", 1);
    assert_eq!(
        client
            .rent()
            .state_one(item.tzid)
            .await
            .unwrap()
            .messages
            .len(),
        1
    );
    let messages = client.rent().state_one(item.tzid).await.unwrap().messages;
    assert_eq!(messages[0].text, "second 2222"); // newest first
    assert_eq!(messages[0].code, "2222");
    assert_eq!(messages[1].service, "other");

    let tariffs = client.rent().tariffs().await.unwrap();
    assert_eq!(tariffs.len(), 1);
    assert_eq!(tariffs["44"].days.get("1"), Some(&2.0));
    let error = client.rent().tariffs_one(7).await.unwrap_err();
    assert_eq!(code_of(error), "NO_COUNTRY"); // `[]` → the client's NO_COUNTRY
}

// ----- user and free -----

#[tokio::test(start_paused = true)]
async fn profile_webhook_and_logs() {
    let (mock, client) = mock_with(10.0);
    let user = client.user();
    let profile = user.profile().await.unwrap();
    assert_eq!(profile.webhook_url, None);

    user.set_webhook_url("https://example.com/hook")
        .await
        .unwrap();
    assert_eq!(
        mock.webhook_url().as_deref(),
        Some("https://example.com/hook")
    );
    let invalid = user.set_webhook_url("ftp://example.com").await.unwrap_err();
    assert_eq!(code_of(invalid), "VALIDATION_ERROR");

    let tzid = client.numbers().get("google", 44, &[]).await.unwrap();
    mock.add_sms(tzid, "Code 777777");
    let logs = user.webhook_logs(1).await.unwrap();
    assert_eq!(logs.total, 1);
    assert_eq!(logs.data[0].kind, "receiving_sms");
    assert_eq!(logs.data[0].params["code"], "777777");

    user.clear_webhook_url().await.unwrap();
    assert_eq!(user.profile().await.unwrap().webhook_url, None);
    mock.add_sms(tzid, "Code 888888");
    assert_eq!(user.webhook_logs(1).await.unwrap().total, 1);
}

#[tokio::test(start_paused = true)]
async fn balance_and_payment_history() {
    let (_mock, client) = mock_with(5.0);
    client.numbers().get("telegram", 7, &[]).await.unwrap();
    let balance = client.user().balance().await.unwrap();
    assert_eq!(
        (balance.balance, balance.zbalance, balance.income),
        (5.0, 1.2, Some(0.0))
    );
    let history = client.user().payment_history(1).await.unwrap();
    assert!(history.orders.data.is_empty());
}

#[tokio::test(start_paused = true)]
async fn free_list() {
    let (_mock, client) = mock_with(100.0);
    let uk = client
        .free()
        .list(&FreeListOptions::default())
        .await
        .unwrap();
    assert!(uk.numbers.contains_key("7459608320"));
    assert_eq!(uk.messages.page.data.len(), 2);
    assert_eq!(uk.messages.page.data[0].code, "195895");
    let ru = FreeListOptions {
        country: Some(7),
        ..FreeListOptions::default()
    };
    let ru = client.free().list(&ru).await.unwrap();
    assert_eq!(ru.messages.page.data[0].text, "Telegram code: 51423");
}

// ----- control API -----

#[tokio::test(start_paused = true)]
async fn fail_next_once_and_per_endpoint() {
    let (mock, client) = mock_with(100.0);
    mock.fail_next("TRY_AGAIN_LATER");
    assert_eq!(
        code_of(client.user().balance().await.unwrap_err()),
        "TRY_AGAIN_LATER"
    );
    assert!(client.user().balance().await.is_ok());

    mock.fail_next_on("getNum.php", "NO_NUMBER");
    assert!(client.user().balance().await.is_ok());
    assert!(
        client
            .numbers()
            .get("google", 44, &[])
            .await
            .unwrap_err()
            .is_no_number()
    );
    assert!(client.numbers().get("google", 44, &[]).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn fail_next_transport_once_and_wait_code_retries_it() {
    let (mock, client) = mock_with(100.0);
    mock.fail_next_transport(503);
    match client.user().balance().await.unwrap_err() {
        Error::Transport { status, .. } => assert_eq!(status, Some(503)),
        other => panic!("expected a transport error, got {other:?}"),
    }

    let tzid = client.numbers().get("google", 44, &[]).await.unwrap();
    mock.add_sms(tzid, "Code 4242");
    mock.fail_next_transport_on("getState.php", 502);
    assert_eq!(
        client.numbers().wait_code(tzid, now()).await.unwrap(),
        "4242"
    );
}

#[tokio::test(start_paused = true)]
async fn set_balance_requests_reset_and_copies() {
    let (mock, client) = mock_with(100.0);
    mock.set_balance(3.0, 0.5);
    let balance = client.user().balance().await.unwrap();
    assert_eq!((balance.balance, balance.zbalance), (3.0, 0.5));

    client
        .numbers()
        .get("google", 44, &["4470", "4471"])
        .await
        .unwrap();
    let requests = mock.requests();
    let last = requests.last().unwrap();
    assert_eq!(
        (last.method, last.path.as_str(), last.body.as_ref()),
        (Method::Get, "getNum.php", None)
    );
    let query: Vec<_> = last
        .query
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        query,
        [
            ("country", "44"),
            ("lang", "en"),
            ("reject", "4470,4471"),
            ("service", "google")
        ]
    );

    client
        .user()
        .set_webhook_url("https://example.com/h")
        .await
        .unwrap();
    let post = mock.requests().pop().unwrap();
    assert_eq!(post.method, Method::Post);
    assert_eq!(
        post.body.unwrap()["profile"]["webhook_url"],
        "https://example.com/h"
    );

    let mut copy = mock.operations();
    copy[0].messages.clear();
    copy.clear();
    assert_eq!(mock.operations().len(), 1);

    mock.reset();
    assert_eq!(mock.balance(), 100.0);
    assert!(mock.operations().is_empty() && mock.requests().is_empty());
    assert_eq!(mock.webhook_url(), None);
    assert_eq!(client.numbers().get("google", 44, &[]).await.unwrap(), 1);
}

#[test]
#[should_panic(expected = "no open operation or rent 7")]
fn add_sms_to_an_unknown_id_panics() {
    MockOnlineSim::default().add_sms(7, "x");
}

#[test]
#[should_panic(expected = "no operation or rent 7")]
fn add_sms_after_to_an_unknown_id_panics() {
    MockOnlineSim::default().add_sms_after(7, "x", 1);
}

// ----- blocking client and 0.2.0 helpers -----

#[test]
fn blocking_client_without_a_runtime() {
    let mock = MockOnlineSim::default();
    let client = mock.client_blocking().unwrap();
    let order = client.numbers().get_with_number("google", 44, &[]).unwrap();
    mock.add_sms_after(order.tzid, "Your code: 98 76 54", 1);
    assert_eq!(
        client.numbers().wait_code(order.tzid, now()).unwrap(),
        "987654"
    );
    assert_eq!(client.user().balance().unwrap().balance, 99.8);
    assert_eq!(client.rent().get(7, 1, true).unwrap().sum, 1.0);
}

#[test]
fn legacy_script_sms_and_fail_no_number() {
    let mock = MockOnlineSim::default();
    let client = mock.client_blocking().unwrap();
    mock.script_sms(SmsScript {
        service: "google".into(),
        number: "+447000000000".into(),
        country: 44,
        code: "999111".into(),
        polls_before_code: 1,
    });
    let order = client.numbers().get_with_number("google", 44, &[]).unwrap();
    assert_eq!(order.number, "+447000000000");
    assert_eq!(
        client.numbers().wait_code(order.tzid, now()).unwrap(),
        "999111"
    );

    mock.fail_no_number("vkcom");
    assert!(
        client
            .numbers()
            .get("vkcom", 44, &[])
            .unwrap_err()
            .is_no_number()
    );
}
