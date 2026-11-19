//! Inbound webhook bodies (`receiving_sms`, `rent_sms`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::de::{int, text};
use crate::error::{Error, Result};
use crate::types::WebhookType;

/// Inbound webhook body as normalised by [`parse_webhook`]: missing or malformed fields are `None`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WebhookPayload {
    /// `receiving_sms` or `rent_sms`; `None` for anything else.
    pub webhook_type: Option<WebhookType>,
    /// Operation id (`tzid`).
    pub operation_id: Option<i64>,
    /// User id.
    pub user_id: Option<i64>,
    /// Country code of the number.
    pub country_code: Option<i64>,
    /// Number.
    pub number: Option<String>,
    /// SMS sender.
    pub sender: Option<String>,
    /// Full SMS text.
    pub message: Option<String>,
    /// Extracted code.
    pub code: Option<String>,
    /// Operation start time.
    pub time_start: Option<String>,
    /// Time left.
    pub time_left: Option<i64>,
}

/// Parse a webhook body from its JSON text.
///
/// Lenient: numeric fields may arrive as strings, missing or malformed fields become `None`, unknown fields
/// are dropped. Fails ([`Error::Decode`]) only when the body is not a JSON object.
pub fn parse_webhook(body: &str) -> Result<WebhookPayload> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| Error::Decode("webhook payload is not valid JSON".into()))?;
    parse_webhook_value(&value)
}

/// [`parse_webhook`] for an already parsed JSON value.
pub fn parse_webhook_value(value: &Value) -> Result<WebhookPayload> {
    let Value::Object(d) = value else {
        return Err(Error::Decode("webhook payload is not a JSON object".into()));
    };
    let field = |key: &str| d.get(key).unwrap_or(&Value::Null);
    Ok(WebhookPayload {
        webhook_type: match field("webhook_type").as_str() {
            Some("receiving_sms") => Some(WebhookType::ReceivingSms),
            Some("rent_sms") => Some(WebhookType::RentSms),
            _ => None,
        },
        operation_id: int(field("operation_id")),
        user_id: int(field("user_id")),
        country_code: int(field("country_code")),
        number: text(field("number")),
        sender: text(field("sender")),
        message: text(field("message")),
        code: text(field("code")),
        time_start: text(field("time_start")),
        time_left: int(field("time_left")),
    })
}
