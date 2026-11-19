mod common;

use common::{client, fixture, ok, q};
use onlinesim_rs_api::types::{OperationMsg, OperationStatus};
use onlinesim_rs_api::{ApiError, Error, OrderBy, StateOneOptions, StateOptions};
use serde_json::json;

#[tokio::test]
async fn price() {
    let (client, fake) = client(vec![ok(json!({ "response": "1", "price": 0.36 }))]);
    assert_eq!(
        client.numbers().price("vkcom", 44).await.unwrap(),
        Some(0.36)
    );
    let call = fake.last();
    assert_eq!(call.path(), "/api/getPrice.php");
    assert_eq!(call.query(), q(&[("service", "vkcom"), ("country", "44")]));
}

#[tokio::test]
async fn price_is_none_when_the_server_has_none() {
    let (client, _) = client(vec![ok(json!({ "response": "1", "price": null }))]);
    assert_eq!(client.numbers().price("vkcom", 7).await.unwrap(), None);
}

#[tokio::test]
async fn get_sends_no_number_flag_and_reject_as_a_comma_list() {
    let (client, fake) = client(vec![ok(json!({ "response": 1, "tzid": 1520 }))]);
    let tzid = client
        .numbers()
        .get("google", 44, &["4470", "4471"])
        .await
        .unwrap();
    assert_eq!(tzid, 1520);
    let call = fake.last();
    assert_eq!(call.path(), "/api/getNum.php");
    assert_eq!(
        call.query(),
        q(&[
            ("service", "google"),
            ("country", "44"),
            ("reject", "4470,4471")
        ])
    );
}

#[tokio::test]
async fn get_omits_an_empty_reject() {
    let (client, fake) = client(vec![ok(json!({ "response": 1, "tzid": 1 }))]);
    client.numbers().get("google", 7, &[]).await.unwrap();
    assert_eq!(
        fake.last().query(),
        q(&[("service", "google"), ("country", "7")])
    );
}

#[tokio::test]
async fn get_with_number_sends_number_1_and_returns_the_order() {
    let (client, fake) = client(vec![ok(json!({
        "response": 1, "tzid": 1520, "number": "+447459608320", "country": 44, "time": 1200, "service": "google"
    }))]);
    let order = client
        .numbers()
        .get_with_number("google", 44, &[])
        .await
        .unwrap();
    assert_eq!(
        (
            order.tzid,
            order.number.as_str(),
            order.country,
            order.time,
            order.service.as_str()
        ),
        (1520, "+447459608320", 44, 1200, "google")
    );
    assert_eq!(
        fake.last().query(),
        q(&[("service", "google"), ("country", "44"), ("number", "1")])
    );
}

#[tokio::test]
async fn state_sends_booleans_as_1_0_lowercase_orderby_and_the_number_flag() {
    let (client, fake) = client(vec![ok(fixture("getState"))]);
    let ops = client
        .numbers()
        .state(StateOptions {
            order_by: OrderBy::Desc,
            msg_list: true,
            ..StateOptions::default()
        })
        .await
        .unwrap();
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0].tzid, 594);
    assert_eq!(ops[0].response, OperationStatus::TzNumAnswer);
    assert_eq!(ops[0].msg, Some(OperationMsg::Text("195895".into())));
    assert_eq!(ops[1].msg, None);
    assert_eq!(ops[1].guard_interval_remaining_seconds, Some(120));
    let call = fake.last();
    assert_eq!(call.path(), "/api/getState.php");
    assert_eq!(
        call.query(),
        q(&[
            ("message_to_code", "1"),
            ("orderby", "desc"),
            ("msg_list", "1"),
            ("number", "1")
        ])
    );
}

#[tokio::test]
async fn state_defaults() {
    let (client, fake) = client(vec![ok(json!([]))]);
    client
        .numbers()
        .state(StateOptions::default())
        .await
        .unwrap();
    assert_eq!(
        fake.last().query(),
        q(&[
            ("message_to_code", "1"),
            ("orderby", "asc"),
            ("msg_list", "0"),
            ("number", "1")
        ])
    );
}

#[tokio::test]
async fn state_returns_empty_on_error_no_operations() {
    let (client, _) = client(vec![ok(json!({ "response": "ERROR_NO_OPERATIONS" }))]);
    assert!(
        client
            .numbers()
            .state(StateOptions::default())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn state_accepts_the_wrapped_list_form() {
    let mut body = json!({ "response": "1" });
    body["list"] = fixture("getState");
    let (client, _) = client(vec![ok(body)]);
    assert_eq!(
        client
            .numbers()
            .state(StateOptions::default())
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn state_parses_a_msg_list() {
    let (client, _) = client(vec![ok(json!([{
        "tzid": 1, "response": "TZ_NUM_ANSWER",
        "msg": [{ "service": "google", "msg": "G-123456" }]
    }]))]);
    let ops = client
        .numbers()
        .state(StateOptions::default())
        .await
        .unwrap();
    let Some(OperationMsg::List(list)) = &ops[0].msg else {
        panic!("{:?}", ops[0].msg)
    };
    assert_eq!(list[0].msg.as_deref(), Some("G-123456"));
}

#[tokio::test]
async fn state_one_returns_the_operation() {
    let first = fixture("getState")[0].clone();
    let (client, fake) = client(vec![ok(json!([first]))]);
    let op = client
        .numbers()
        .state_one(
            594,
            StateOneOptions {
                message_to_code: false,
                ..StateOneOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(op.tzid, 594);
    assert_eq!(op.number.as_deref(), Some("+77781746533"));
    assert_eq!(
        fake.last().query(),
        q(&[
            ("tzid", "594"),
            ("message_to_code", "0"),
            ("msg_list", "0"),
            ("number", "1")
        ])
    );
}

#[tokio::test]
async fn state_one_raises_error_no_operations() {
    let (client, _) = client(vec![
        ok(json!({ "response": "ERROR_NO_OPERATIONS" })),
        ok(json!([])),
    ]);
    for _ in 0..2 {
        let error = client
            .numbers()
            .state_one(1, StateOneOptions::default())
            .await
            .unwrap_err();
        assert_eq!(error.code(), Some("ERROR_NO_OPERATIONS"));
    }
}

#[tokio::test]
async fn next() {
    let (client, fake) = client(vec![ok(json!({ "response": "1", "tzid": "1520" }))]);
    client.numbers().next(1520).await.unwrap();
    let call = fake.last();
    assert_eq!(call.path(), "/api/setOperationRevise.php");
    assert_eq!(call.query(), q(&[("tzid", "1520")]));
}

#[tokio::test]
async fn close() {
    let (client, fake) = client(vec![ok(json!({ "response": 1, "tzid": 1520 }))]);
    client.numbers().close(1520).await.unwrap();
    let call = fake.last();
    assert_eq!(call.path(), "/api/setOperationOk.php");
    assert_eq!(call.query(), q(&[("tzid", "1520")]));
}

#[tokio::test]
async fn ban() {
    let (client, fake) = client(vec![ok(json!({ "response": 1, "tzid": 1520 }))]);
    client.numbers().ban(1520).await.unwrap();
    let call = fake.last();
    assert_eq!(call.path(), "/api/setOperationOk.php");
    assert_eq!(call.query(), q(&[("tzid", "1520"), ("ban", "1")]));
}

#[tokio::test]
async fn close_surfaces_no_complete_tzid() {
    let (client, _) = client(vec![ok(json!({ "response": "NO_COMPLETE_TZID" }))]);
    let error = client.numbers().close(1).await.unwrap_err();
    assert!(matches!(&error, Error::Api(ApiError { code, .. }) if code == "NO_COMPLETE_TZID"));
}

#[tokio::test]
async fn tariffs_requests_country_all_and_normalises_empty_services() {
    let uk = fixture("getNumbersStats_44");
    let mut ru = uk.clone();
    ru["code"] = json!(7);
    ru["services"] = json!([]);
    let (client, fake) = client(vec![ok(json!({ "44": uk, "7": ru }))]);
    let all = client.numbers().tariffs().await.unwrap();
    let call = fake.last();
    assert_eq!(call.path(), "/api/getNumbersStats.php");
    assert_eq!(call.query(), q(&[("country", "all")]));
    assert_eq!(all["44"].services["service_vkcom"].slug, "vkcom");
    assert!(all["7"].services.is_empty());
}

#[tokio::test]
async fn tariffs_one() {
    let (client, fake) = client(vec![ok(fixture("getNumbersStats_44"))]);
    let uk = client.numbers().tariffs_one(44).await.unwrap();
    assert_eq!(fake.last().query(), q(&[("country", "44")]));
    assert_eq!(
        (uk.code, uk.name.as_str(), uk.other),
        (44, "united_kingdom", Some(0.63))
    );
    assert_eq!(uk.locale_name.as_deref(), Some("United Kingdom"));
    let vk = &uk.services["service_vkcom"];
    assert_eq!(
        (vk.count, vk.price, vk.service.as_str()),
        (Some(363), 0.36, "VK")
    );
}

#[tokio::test]
async fn tariffs_one_reads_unknown_stock_and_other_false() {
    let (client, _) = client(vec![ok(json!({
        "name": "x", "position": 1, "code": 1, "other": false, "new": true, "enabled": true,
        "services": { "service_a": { "count": "?", "popular": true, "code": 1, "price": "0.50", "id": 9, "service": "A", "slug": 123 } }
    }))]);
    let c = client.numbers().tariffs_one(1).await.unwrap();
    assert_eq!(c.other, None);
    assert!(c.is_new);
    let a = &c.services["service_a"];
    assert_eq!((a.count, a.price, a.slug.as_str()), (None, 0.5, "123"));
}

#[tokio::test]
async fn tariffs_one_raises_no_country() {
    let (client, _) = client(vec![ok(json!({ "response": "NO_COUNTRY" }))]);
    let error = client.numbers().tariffs_one(999).await.unwrap_err();
    assert_eq!(error.code(), Some("NO_COUNTRY"));
}
