---
title: Webhooks
description: Receive SMS as HTTP callbacks and parse them with parse_webhook.
navigation:
  order: 5
---

Set a URL with [`client.user().set_webhook_url`](/user#set_webhook_url). For each SMS the server POSTs a JSON body to it: `webhook_type` `receiving_sms` for numbers, `rent_sms` for rents. Delivery attempts are listed by [`client.user().webhook_logs`](/user#webhook_logs).

Parsing needs no client: it works with `default-features = false`.

## parse_webhook

Parse an inbound webhook body from its JSON text; `parse_webhook_value` takes an already parsed `serde_json::Value`.

```rust
fn parse_webhook(body: &str) -> Result<WebhookPayload>
fn parse_webhook_value(value: &serde_json::Value) -> Result<WebhookPayload>
```

Lenient: numeric fields may arrive as strings, missing or malformed fields become `None`, unknown fields are dropped. It fails (`Error::Decode`) only when the body is not a JSON object.

```rust
use onlinesim_rs_api::parse_webhook;
use onlinesim_rs_api::types::WebhookType;

// In your HTTP handler, with the request body as text:
fn on_webhook(body: &str) -> onlinesim_rs_api::Result<()> {
    let hook = parse_webhook(body)?;
    if hook.webhook_type == Some(WebhookType::ReceivingSms) {
        println!("{:?} {:?}", hook.operation_id, hook.code);
    }
    Ok(()) // answer HTTP 200
}

fn main() {
    on_webhook(r#"{"webhook_type":"receiving_sms","operation_id":"1520","code":"195895"}"#).unwrap();
}
```

## Payload fields

| Field | Type | Description |
|---|---|---|
| `webhook_type` | `Option<WebhookType>` | `ReceivingSms` or `RentSms`; `None` for anything else. |
| `operation_id` | `Option<i64>` | `tzid` of the operation or rent. |
| `user_id` | `Option<i64>` | Account id. |
| `country_code` | `Option<i64>` | Country code of the number. |
| `number` | `Option<String>` | The number, e.g. `+79001234567`. |
| `sender` | `Option<String>` | Sender of the SMS, e.g. `VK`. |
| `message` | `Option<String>` | Full SMS text. |
| `code` | `Option<String>` | Code extracted from the SMS. |
| `time_start` | `Option<String>` | Start of the operation. |
| `time_left` | `Option<i64>` | Time left. |
