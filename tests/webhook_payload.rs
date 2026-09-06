//! Incoming webhook payload parsing tests.

use onlinesim_rs_api::{parse_webhook_json, WebhookPayload, WebhookType};

#[test]
fn parse_flat_payload_string_fields() {
    let json = r#"{
        "user_id": "204355",
        "country_code": "7",
        "number": "+79918397701",
        "sender": "OKru",
        "message": "your code 612045",
        "time_start": "2023-04-26 12:37:55",
        "time_left": "880",
        "operation_id": "91763432",
        "webhook_type": "receiving_sms",
        "code": "612045"
    }"#;
    let p = parse_webhook_json(json).unwrap();
    assert_eq!(p.user_id, 204355);
    assert_eq!(p.country_code, 7);
    assert_eq!(p.operation_id, 91763432);
    assert_eq!(p.code, "612045");
    assert_eq!(p.webhook_type, WebhookType::ReceivingSms);
}

#[test]
fn parse_numeric_and_wrapped_data() {
    let json = r#"{
        "data": {
            "user_id": 1,
            "country_code": 1,
            "number": "+19001234567",
            "sender": "Telegram",
            "message": "123456",
            "time_start": "2026-01-01 00:00:00",
            "time_left": 10,
            "operation_id": 42,
            "webhook_type": "rent_sms",
            "code": "123456"
        }
    }"#;
    let p = WebhookPayload::parse(json).unwrap();
    assert_eq!(p.webhook_type, WebhookType::RentSms);
    assert_eq!(p.operation_id, 42);
}
