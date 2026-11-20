//! `mock::MockOnlineSim` (in-process transport) through the real clients: every endpoint, every control
//! helper. No sockets, no clock: async tests run on tokio's paused clock, `wait_code` with a zero interval.

use std::collections::BTreeMap;
use std::time::Duration;

use onlinesim_rs_api::mock::{
    MOCK_BASE_URL, MockOnlineSim, MockOptions, MockRentTariff, SmsScript,
};
use onlinesim_rs_api::types::{OperationMsg, OperationStatus};
use onlinesim_rs_api::{
    BlockingTransport, Client, Error, FreeListOptions, HttpRequest, HttpResponse, Method, OrderBy,
    StateOneOptions, StateOptions, WaitCodeOptions,
};
use serde_json::{Value, json};

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

/// A raw GET through the mock's transport: status and parsed body, as the server would send them.
fn raw(mock: &MockOnlineSim, path_and_query: &str, key: Option<&str>) -> (u16, Value) {
    let request = HttpRequest {
        method: Method::Get,
        url: format!("{MOCK_BASE_URL}{path_and_query}"),
        headers: key
            .map(|k| ("Authorization".to_string(), format!("Bearer {k}")))
            .into_iter()
            .collect(),
        body: None,
        timeout: Duration::from_secs(1),
    };
    let HttpResponse { status, body } =
        BlockingTransport::send(&mock.transport(), request).unwrap();
    (status, serde_json::from_str(&body).unwrap())
}

// ----- auth and transport -----

#[tokio::test(start_paused = true)]
async fn wrong_key_is_456_missing_key_200_both_error_wrong_key() {
    let mock = MockOnlineSim::default();
    for builder in [Client::builder().api_key("wrong"), Client::builder()] {
        let client = builder.build_with(mock.transport()).unwrap();
        let error = client.user().balance().await.unwrap_err();
        assert_eq!(code_of(error), "ERROR_WRONG_KEY");
    }
    let wrong_key = json!({ "response": "ERROR_WRONG_KEY" });
    assert_eq!(raw(&mock, "getBalance.php", None), (200, wrong_key.clone()));
    assert_eq!(
        raw(&mock, "getBalance.php", Some("nope")),
        (456, wrong_key.clone())
    );
    // Public endpoints check a sent key too.
    assert_eq!(raw(&mock, "getFreeList", Some("nope")), (456, wrong_key));
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
    assert_eq!(order.title.as_deref(), Some("vkcom"));
    assert_eq!(order.guard_interval_remaining_seconds, Some(0));
    let (_, body) = raw(
        &mock,
        "getNum.php?service=google&country=44&number=1",
        Some("mock-key"),
    );
    assert_eq!(body["response_text"], "Waiting for SMS");
    mock.reset();
    numbers.get("google", 44, &[]).await.unwrap();
    numbers.get_with_number("vkcom", 7, &[]).await.unwrap();

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
    assert_eq!(op.response, Some(OperationStatus::TzNumWait));
    assert_eq!(op.msg, None);
    assert_eq!(op.guard_interval_remaining_seconds, Some(0));

    mock.add_sms(first, "Your code: 123-456");
    let op = numbers
        .state_one(first, StateOneOptions::default())
        .await
        .unwrap();
    assert_eq!(op.response, Some(OperationStatus::TzNumAnswer));
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
    assert_eq!(op.response, Some(OperationStatus::TzOverOk));
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
    assert_eq!(
        (op.response, op.msg),
        (Some(OperationStatus::TzNumWait), None)
    );
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
    let revised = mock
        .requests()
        .iter()
        .filter(|r| r.path == "setOperationRevise.php")
        .count();
    assert_eq!(revised, 1);
}

#[tokio::test(start_paused = true)]
async fn next_answers_the_tzid_as_a_string_and_rejects_unknown_or_zero_tzids() {
    let (mock, client) = mock_with(100.0);
    let numbers = client.numbers();
    let tzid = numbers.get("google", 44, &[]).await.unwrap();
    let path = format!("setOperationRevise.php?tzid={tzid}");
    assert_eq!(
        raw(&mock, &path, Some("mock-key")),
        (200, json!({ "tzid": tzid.to_string(), "response": "1" }))
    );
    assert_eq!(
        code_of(numbers.next(424_242).await.unwrap_err()),
        "ERROR_WRONG_TZID"
    );
    assert_eq!(code_of(numbers.next(0).await.unwrap_err()), "ERROR_NO_TZID");
    assert_eq!(
        code_of(numbers.close(0).await.unwrap_err()),
        "ERROR_NO_TZID"
    );
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
async fn rent_get_reserves_first_sms_confirms_and_charges_extend_close() {
    let (mock, client) = mock_with(10.0);
    let rent = client.rent();
    let item = rent.get(7, 3, true).await.unwrap();
    assert_eq!((item.tzid, item.number.as_str()), (1, "900000001"));
    assert_eq!((item.status, item.checked), (0, Some(false)));
    assert_eq!((item.sum, item.days, item.time), (2.5, 0, 20));
    assert_eq!((item.rent, item.days_autoextend), (Some(3), Some(3)));
    assert_eq!(item.time_seconds, Some(1200));
    assert_eq!(item.guard_interval_remaining_seconds, Some(0));
    assert_eq!(item.extend.get("3"), Some(&2.5));
    // Reserved: the total balance stays, the price is frozen.
    assert_eq!(mock.balance(), 10.0);
    let balance = client.user().balance().await.unwrap();
    assert_eq!((balance.balance, balance.zbalance), (10.0, 2.5));
    let unconfirmed = rent.extend(item.tzid, 1).await.unwrap_err();
    assert_eq!(code_of(unconfirmed), "TRY_AGAIN_LATER");

    mock.add_sms_from(item.tzid, "Bank: 777 888", Some("bank"));
    mock.add_sms(item.tzid, "no code here");
    let state = rent.state_one(item.tzid).await.unwrap();
    assert_eq!((state.status, state.checked), (1, Some(true)));
    assert_eq!((state.days, state.time), (3, 3 * 1440));
    assert_eq!(state.time_seconds, Some(3 * 86_400));
    let messages: Vec<_> = state
        .messages
        .iter()
        .map(|m| (m.service.as_str(), m.code.as_str()))
        .collect();
    assert_eq!(messages, [("other", ""), ("bank", "777888")]); // newest first
    assert_eq!(mock.balance(), 7.5);
    let balance = client.user().balance().await.unwrap();
    assert_eq!((balance.balance, balance.zbalance), (7.5, 0.0));

    let extended = rent.extend(item.tzid, 1).await.unwrap();
    assert_eq!((extended.days, extended.sum), (4, 3.5));
    assert_eq!(
        (extended.rent, extended.days_autoextend),
        (Some(4), Some(3))
    );
    assert_eq!(mock.balance(), 6.5);
    let period = rent.extend(item.tzid, 2).await.unwrap_err();
    assert_eq!(code_of(period), "TRY_AGAIN_LATER"); // unknown period
    let unknown = rent.extend(999, 1).await.unwrap_err();
    assert_eq!(code_of(unknown), "TRY_AGAIN_LATER"); // unknown tzid

    let fixed = rent.get(7, 1, false).await.unwrap();
    mock.add_sms(fixed.tzid, "1234");
    assert!(rent.state_one(fixed.tzid).await.unwrap().extend.is_empty());
    let refused = rent.extend(fixed.tzid, 1).await.unwrap_err();
    assert_eq!(code_of(refused), "TRY_AGAIN_LATER");
    let ids: Vec<_> = rent.state().await.unwrap().iter().map(|r| r.tzid).collect();
    assert_eq!(ids, [item.tzid, fixed.tzid]);

    rent.close(fixed.tzid).await.unwrap(); // confirmed: no refund
    assert_eq!(mock.balance(), 5.5);
    assert_eq!(mock.rents().len(), 1);
    let gone = rent.state_one(fixed.tzid).await.unwrap_err();
    assert_eq!(code_of(gone), "ERROR_NO_OPERATIONS");
    let again = rent.close(fixed.tzid).await.unwrap_err();
    assert_eq!(code_of(again), "ERROR_NO_OPERATIONS");
}

#[tokio::test(start_paused = true)]
async fn rent_sum_is_a_number_in_get_and_a_decimal_string_in_state_on_the_wire() {
    let mock = MockOnlineSim::default();
    let (_, got) = raw(&mock, "rent/getRentNum.php?days=3", Some("mock-key"));
    assert_eq!(got["item"]["sum"], json!(2.5));
    let (_, listed) = raw(&mock, "rent/getRentState.php", Some("mock-key"));
    assert_eq!(listed["list"][0]["sum"], json!("2.50"));
}

#[tokio::test(start_paused = true)]
async fn unconfirmed_rent_close_releases_notify_sms_has_code_0_and_guard_interval() {
    let mock = MockOnlineSim::new(MockOptions {
        balance: 10.0,
        guard_interval: 120,
        ..MockOptions::default()
    });
    let client = mock.client().unwrap();
    let rent = client.rent();
    let item = rent.get(7, 3, true).await.unwrap();
    assert_eq!(item.guard_interval_remaining_seconds, Some(120));
    let early = rent.close(item.tzid).await.unwrap_err();
    assert_eq!(code_of(early), "NO_COMPLETE_TZID");

    mock.add_sms_from(
        item.tzid,
        "Messages from this sender are not accepted",
        Some("notify"),
    );
    let state = rent.state_one(item.tzid).await.unwrap();
    assert_eq!((state.status, state.checked), (0, Some(false)));
    assert_eq!(state.guard_interval_remaining_seconds, Some(0));
    assert_eq!(
        (
            state.messages[0].service.as_str(),
            state.messages[0].code.as_str()
        ),
        ("notify", "0")
    );
    let (_, wire) = raw(&mock, "rent/getRentState.php", Some("mock-key"));
    assert_eq!(wire["list"][0]["messages"][0]["code"], json!(0));

    rent.close(item.tzid).await.unwrap();
    assert_eq!(mock.balance(), 10.0); // released, nothing charged
    let balance = client.user().balance().await.unwrap();
    assert_eq!((balance.balance, balance.zbalance), (10.0, 0.0));
}

#[tokio::test(start_paused = true)]
async fn rent_errors() {
    let (_mock, client) = mock_with(2.0);
    let rent = client.rent();
    assert_eq!(
        code_of(rent.get(44, 1, true).await.unwrap_err()),
        "UNDEFINED_COUNTRY"
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
    let invalid = user.set_webhook_url("not a url").await.unwrap_err();
    assert_eq!(code_of(invalid), "VALIDATION_ERROR");
    user.set_webhook_url("ftp://example.com").await.unwrap(); // any scheme://… URL
    user.set_webhook_url("https://example.com/hook")
        .await
        .unwrap();

    let tzid = client.numbers().get("google", 44, &[]).await.unwrap();
    mock.add_sms(tzid, "Code 777777");
    let item = client.rent().get(7, 1, true).await.unwrap();
    mock.add_sms(item.tzid, "code 5678");
    mock.add_sms(item.tzid, "no code");
    let logs = user.webhook_logs(1).await.unwrap();
    assert_eq!(logs.total, 3);
    let seen: Vec<_> = logs
        .data
        .iter()
        .map(|l| {
            (
                l.kind.as_str(),
                l.params["operation_id"].as_i64().unwrap(),
                l.params["code"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("rent_sms", item.tzid, ""),
            ("rent_sms", item.tzid, "5678"),
            ("receiving_sms", tzid, "777777"),
        ]
    );
    // Activation: +number, seconds left, order time. Rent: digits without +, minutes left, SMS time.
    let activation = &logs.data[2].params;
    assert_eq!(
        (
            &activation["number"],
            &activation["time_left"],
            &activation["time_start"]
        ),
        (
            &json!("+44900000001"),
            &json!(1200),
            &json!("2026-01-01 00:00:01")
        )
    );
    let rented = &logs.data[1].params;
    assert_eq!(
        (
            &rented["number"],
            &rented["time_left"],
            &rented["time_start"]
        ),
        (
            &json!("7900000002"),
            &json!(1440),
            &json!("2026-01-01 00:00:03")
        )
    );

    user.clear_webhook_url().await.unwrap();
    assert_eq!(user.profile().await.unwrap().webhook_url, None);
    mock.add_sms(tzid, "Code 888888");
    assert_eq!(user.webhook_logs(1).await.unwrap().total, 3);
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
    assert!(history.paylist.is_empty());
    assert_eq!(history.forms["1"], "Card");
    assert_eq!(history.buttons["USD"], json!([5, 10, 20]));
    assert_eq!(history.domains, ["onlinesim.io"]);
}

#[tokio::test(start_paused = true)]
async fn free_list() {
    let (_mock, client) = mock_with(100.0);
    let uk = client
        .free()
        .list(&FreeListOptions::default())
        .await
        .unwrap();
    let codes: Vec<_> = uk.countries.iter().map(|c| c.country).collect();
    assert_eq!(codes, [44, 7]);
    assert!(uk.numbers.contains_key("7459608320"));
    assert_eq!((uk.messages.country, uk.messages.page.per_page), (44, 10));
    assert_eq!(uk.messages.page.data.len(), 2);
    assert_eq!(uk.messages.page.data[0].code, "195895");
    assert_eq!(
        uk.messages.page.data[0].created_at,
        "2026-01-01T00:00:00.000000Z"
    );
    for (count, per_page) in [(0, 10), (9999, 500)] {
        let options = FreeListOptions {
            count: Some(count),
            ..FreeListOptions::default()
        };
        let list = client.free().list(&options).await.unwrap();
        assert_eq!(list.messages.page.per_page, per_page); // clamped to 10..500
    }
    let unknown = FreeListOptions {
        country: Some(999),
        ..FreeListOptions::default()
    };
    let unknown = client.free().list(&unknown).await.unwrap();
    assert_eq!(unknown.messages.country, 44); // unknown → the first country
    let (_, wire) = raw(&MockOnlineSim::default(), "getFreeList", None);
    assert_eq!(
        (&wire["response"], &wire["content"]),
        (&json!(1), &Value::Null)
    );
    assert!(wire["ignore"].is_string());
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
