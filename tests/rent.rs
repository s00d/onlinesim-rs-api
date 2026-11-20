mod common;

use common::{client, fixture, ok, q};
use serde_json::{Value, json};

fn state() -> Value {
    fixture("rent_getRentState")
}

fn detail(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
    let mut all = q(pairs);
    all.extend(q(&[("pagination", "0"), ("number", "1")]));
    all
}

#[tokio::test]
async fn get_sends_extension_explicitly_pagination_0_and_number_and_parses_sum() {
    let item = state()["list"][0].clone();
    let (client, fake) = client(vec![ok(json!({ "response": 1, "item": item }))]);
    let rent = client.rent().get(7, 3, true).await.unwrap();
    let call = fake.last();
    assert_eq!(call.path(), "/api/rent/getRentNum.php");
    assert_eq!(
        call.query(),
        detail(&[("country", "7"), ("days", "3"), ("extension", "1")])
    );
    assert_eq!(rent.sum, 5.14);
    assert_eq!(rent.extend.len(), 3);
    assert_eq!(rent.extend["7"], 51.0);
    assert_eq!(rent.messages[0].code, "195895");
    assert_eq!(rent.time_seconds, Some(18034680));
}

#[tokio::test]
async fn get_with_extension_false_sends_extension_0() {
    let item = state()["list"][1].clone();
    let (client, fake) = client(vec![ok(json!({ "response": 1, "item": item }))]);
    client.rent().get(7, 1, false).await.unwrap();
    assert_eq!(fake.last().query()["extension"], "0");
}

#[tokio::test]
async fn state_normalises_sum_and_the_empty_extend_map() {
    let (client, fake) = client(vec![ok(state())]);
    let list = client.rent().state().await.unwrap();
    let call = fake.last();
    assert_eq!(call.path(), "/api/rent/getRentState.php");
    assert_eq!(call.query(), detail(&[]));
    assert_eq!(list.iter().map(|r| r.sum).collect::<Vec<_>>(), [5.14, 0.0]);
    assert!(list[1].extend.is_empty());
    assert!(list[1].messages.is_empty());
}

#[tokio::test]
async fn state_one() {
    let item = state()["list"][0].clone();
    let (client, fake) = client(vec![ok(json!({ "response": 1, "list": [item] }))]);
    let rent = client.rent().state_one(430).await.unwrap();
    assert_eq!((rent.tzid, rent.sum), (430, 5.14));
    assert_eq!(fake.last().query(), detail(&[("tzid", "430")]));
}

#[tokio::test]
async fn state_one_raises_error_no_operations_for_an_unknown_tzid() {
    let (client, _) = client(vec![ok(json!({ "response": 1, "list": [] }))]);
    let error = client.rent().state_one(1).await.unwrap_err();
    assert_eq!(error.code(), Some("ERROR_NO_OPERATIONS"));
}

#[tokio::test]
async fn extend() {
    let item = state()["list"][0].clone();
    let (client, fake) = client(vec![ok(json!({ "response": 1, "item": item }))]);
    assert_eq!(client.rent().extend(430, 3).await.unwrap().tzid, 430);
    let call = fake.last();
    assert_eq!(call.path(), "/api/rent/extendRentState.php");
    assert_eq!(call.query(), detail(&[("tzid", "430"), ("days", "3")]));
}

#[tokio::test]
async fn tariffs() {
    let (client, fake) = client(vec![ok(fixture("rent_tariffsRent"))]);
    let all = client.rent().tariffs().await.unwrap();
    let call = fake.last();
    assert_eq!(call.path(), "/api/rent/tariffsRent.php");
    assert!(call.query().is_empty());
    assert_eq!(all["7"].days["30"], 24.11);
    assert_eq!(all["380"].count["7"], 0);
    assert!(all["380"].ext_counts.is_empty());
}

#[tokio::test]
async fn tariffs_one() {
    let ru = fixture("rent_tariffsRent")["7"].clone();
    let (client, fake) = client(vec![ok(ru)]);
    let t = client.rent().tariffs_one(7).await.unwrap();
    assert_eq!(fake.last().query(), q(&[("country", "7")]));
    assert_eq!(
        (t.code, t.name.as_str(), t.extend, t.confirm),
        (7, "russia", 1, Some(false))
    );
    assert_eq!(t.days["3"], 5.14);
}

#[tokio::test]
async fn tariffs_one_raises_no_country_for_the_empty_array_answer() {
    let (client, _) = client(vec![ok(json!([]))]);
    let error = client.rent().tariffs_one(999).await.unwrap_err();
    assert_eq!(error.code(), Some("NO_COUNTRY"));
}

#[tokio::test]
async fn close() {
    let (client, fake) = client(vec![ok(json!({ "response": "1" }))]);
    client.rent().close(430).await.unwrap();
    let call = fake.last();
    assert_eq!(call.path(), "/api/rent/closeRentNum.php");
    assert_eq!(call.query(), q(&[("tzid", "430")]));
}

#[tokio::test]
async fn close_surfaces_no_close_rent_us() {
    let (client, _) = client(vec![ok(json!({ "response": "NO_CLOSE_RENT_US" }))]);
    let error = client.rent().close(1).await.unwrap_err();
    assert_eq!(error.code(), Some("NO_CLOSE_RENT_US"));
}

#[tokio::test]
async fn decodes_sum_as_a_number_or_a_string_and_code_0_of_a_notify_sms() {
    let mut item = state()["list"][0].clone();
    item["status"] = json!(0);
    item["checked"] = json!(false);
    item["sum"] = json!(1.5);
    item["messages"] = json!([
        { "id": 2, "service": "notify", "text": "Messages from this sender are not accepted", "code": 0,
          "created_at": "2026-11-19 10:01:12" },
        { "id": 1, "service": "Google", "text": "G-070051", "code": "070051", "created_at": "2026-11-19 10:01:00" }
    ]);
    let mut listed = item.clone();
    listed["sum"] = json!("2.40");
    let (client, _) = client(vec![
        ok(json!({ "response": 1, "item": item })),
        ok(json!({ "response": 1, "list": [listed] })),
    ]);
    let rent = client.rent().get(7, 3, true).await.unwrap();
    assert_eq!((rent.status, rent.checked, rent.sum), (0, Some(false), 1.5));
    let codes: Vec<_> = rent.messages.iter().map(|m| m.code.as_str()).collect();
    assert_eq!(codes, ["0", "070051"]); // the integer 0 is normalised to "0"
    assert_eq!(client.rent().state_one(rent.tzid).await.unwrap().sum, 2.4);
}
