mod common;

use common::{client, fixture, ok, q};
use onlinesim_rs_api::Method;
use serde_json::json;

#[tokio::test]
async fn balance_sends_income_1_and_parses_the_decimal_string() {
    let (client, fake) = client(vec![ok(fixture("getBalance"))]);
    let b = client.user().balance().await.unwrap();
    assert_eq!(
        (b.balance, b.zbalance, b.income),
        (1442.33, 1023.41, Some(9.78))
    );
    let call = fake.last();
    assert_eq!(call.path(), "/api/getBalance.php");
    assert_eq!(call.query(), q(&[("income", "1")]));
}

#[tokio::test]
async fn balance_without_income() {
    let (client, _) = client(vec![ok(
        json!({ "response": "1", "balance": "0.000", "zbalance": 0 }),
    )]);
    let b = client.user().balance().await.unwrap();
    assert_eq!((b.balance, b.zbalance, b.income), (0.0, 0.0, None));
}

#[tokio::test]
async fn profile() {
    let (client, fake) = client(vec![ok(fixture("getProfile"))]);
    let p = client.user().profile().await.unwrap();
    assert_eq!(fake.last().path(), "/api/getProfile.php");
    assert_eq!(p.id, 1);
    assert_eq!(p.webhook_url.as_deref(), Some("https://example.com/hook"));
    assert_eq!(p.number_reject, Some(json!(["333"])));
    assert_eq!(p.active.as_deref(), Some("false"));
}

#[tokio::test]
async fn payment_history_normalises_the_empty_paylist() {
    let (client, fake) = client(vec![ok(fixture("getPaymentHistory"))]);
    let h = client.user().payment_history(2).await.unwrap();
    let call = fake.last();
    assert_eq!(call.path(), "/api/getPaymentHistory.php");
    assert_eq!(call.query(), q(&[("page", "2")]));
    assert!(h.paylist.is_empty());
    assert_eq!(
        (h.orders.data[0].id, h.orders.data[0].sum.as_str()),
        (9462, "89.00")
    );
    assert_eq!(h.orders.data[0].extra["merchant_name"], "cashout");
    assert_eq!(h.orders.total, 106);
    assert_eq!(h.currency["USD"], 1.0);
}

#[tokio::test]
async fn set_webhook_url_posts_json() {
    let (client, fake) = client(vec![ok(json!({ "response": "1", "user": { "id": 2 } }))]);
    client
        .user()
        .set_webhook_url("https://example.com/hook")
        .await
        .unwrap();
    let call = fake.last();
    assert_eq!(call.method, Method::Post);
    assert_eq!(call.path(), "/api/profile.php");
    assert_eq!(call.header("Content-Type"), Some("application/json"));
    assert_eq!(call.header("Authorization"), Some("Bearer test-key"));
    assert_eq!(
        call.body,
        Some(json!({ "profile": { "webhook_url": "https://example.com/hook" } }))
    );
}

#[tokio::test]
async fn clear_webhook_url_posts_webhook_url_null() {
    let (client, fake) = client(vec![ok(json!({ "response": "1", "user": { "id": 2 } }))]);
    client.user().clear_webhook_url().await.unwrap();
    let call = fake.last();
    assert_eq!(call.method, Method::Post);
    assert_eq!(
        call.body,
        Some(json!({ "profile": { "webhook_url": null } }))
    );
}

#[tokio::test]
async fn webhook_logs_returns_the_paginator() {
    let page = json!({
        "current_page": 1,
        "data": [{
            "id": 7, "user_id": 2, "webhook_url": "https://example.com/hook", "type": "receiving_sms",
            "params": { "webhook_type": "receiving_sms", "operation_id": 1520, "code": "195895" },
            "status": "success", "error": null,
            "created_at": "2026-11-19 10:00:01", "updated_at": "2026-11-19 10:00:01"
        }],
        "from": 1, "to": 1, "last_page": 1, "per_page": 10, "total": 1
    });
    let (client, fake) = client(vec![ok(json!({ "response": "1", "data": page }))]);
    let logs = client.user().webhook_logs(1).await.unwrap();
    let call = fake.last();
    assert_eq!(call.path(), "/api/webhook-logs");
    assert_eq!(call.query(), q(&[("page", "1")]));
    assert_eq!(logs.total, 1);
    let log = &logs.data[0];
    assert_eq!(
        (log.id, log.kind.as_str(), log.status.as_str()),
        (7, "receiving_sms", "success")
    );
    assert_eq!(log.params["code"], "195895");
}
