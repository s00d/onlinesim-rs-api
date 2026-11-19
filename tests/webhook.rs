use onlinesim_rs_api::types::WebhookType;
use onlinesim_rs_api::{Error, WebhookPayload, parse_webhook, parse_webhook_value};
use serde_json::json;

#[test]
fn parses_a_receiving_sms_body() {
    let body = json!({
        "webhook_type": "receiving_sms", "operation_id": 1520, "user_id": 2, "country_code": 7,
        "number": "+79001234567", "sender": "VK", "message": "VK: 195895 is your code", "code": "195895",
        "time_start": "2026-11-19 10:00:00", "time_left": 1100
    })
    .to_string();
    assert_eq!(
        parse_webhook(&body).unwrap(),
        WebhookPayload {
            webhook_type: Some(WebhookType::ReceivingSms),
            operation_id: Some(1520),
            user_id: Some(2),
            country_code: Some(7),
            number: Some("+79001234567".into()),
            sender: Some("VK".into()),
            message: Some("VK: 195895 is your code".into()),
            code: Some("195895".into()),
            time_start: Some("2026-11-19 10:00:00".into()),
            time_left: Some(1100),
        }
    );
}

#[test]
fn is_lenient_numeric_strings_missing_unknown_fields_and_types() {
    let parsed = parse_webhook_value(&json!({
        "webhook_type": "rent_sms", "operation_id": "430", "code": 2991, "time_left": "soon", "extra": true
    }))
    .unwrap();
    assert_eq!(
        parsed,
        WebhookPayload {
            webhook_type: Some(WebhookType::RentSms),
            operation_id: Some(430),
            code: Some("2991".into()),
            ..WebhookPayload::default()
        }
    );
    let other = parse_webhook_value(&json!({ "webhook_type": "other" })).unwrap();
    assert_eq!(other.webhook_type, None);
}

#[test]
fn fails_only_for_input_that_is_not_a_json_object() {
    for body in ["not json", "[1,2]", "null"] {
        assert!(
            matches!(parse_webhook(body), Err(Error::Decode(_))),
            "{body}"
        );
    }
}
