//! Incoming SMS webhook payload and delivery logs.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};

/// Service that produced the webhook (`receiving_sms` or `rent_sms`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookType {
    /// Temporary / single-service SMS reception.
    ReceivingSms,
    /// Long-term number rent.
    RentSms,
    /// Unknown / future type.
    #[serde(other)]
    Other,
}

impl WebhookType {
    /// Wire string value.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReceivingSms => "receiving_sms",
            Self::RentSms => "rent_sms",
            Self::Other => "other",
        }
    }
}

/// JSON body OnlineSim POSTs to your `webhook_url` when an SMS arrives.
///
/// Fields may arrive as numbers or strings (OpenAPI shows strings; backend uses ints).
///
/// Respond with **HTTP 200** to acknowledge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookPayload {
    /// Profile user id.
    #[serde(deserialize_with = "crate::util::de_i64_flexible")]
    pub user_id: i64,
    /// Country dial code of the number.
    #[serde(deserialize_with = "crate::util::de_i64_flexible")]
    pub country_code: i64,
    /// Phone number (international format when available).
    pub number: String,
    /// SMS sender name or number.
    pub sender: String,
    /// Full SMS text.
    pub message: String,
    /// Operation start time (`Y-m-d H:i:s`).
    pub time_start: String,
    /// Remaining time for the operation (minutes in backend).
    #[serde(deserialize_with = "crate::util::de_i64_flexible")]
    pub time_left: i64,
    /// Operation id (`tzid`).
    #[serde(deserialize_with = "crate::util::de_i64_flexible")]
    pub operation_id: i64,
    /// `receiving_sms` or `rent_sms`.
    pub webhook_type: WebhookType,
    /// Extracted verification code from the SMS (via backend `messageToCode`).
    pub code: String,
}

impl WebhookPayload {
    /// Parse a JSON body as received by your HTTP server.
    ///
    /// Accepts either a flat payload or `{ "data": { ... } }` (proxy wrap).
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let value: Value = serde_json::from_slice(bytes)?;
        Self::from_value(value)
    }

    /// Parse from a [`serde_json::Value`].
    pub fn from_value(mut value: Value) -> Result<Self> {
        if let Some(data) = value.get("data").cloned() {
            if data.is_object() {
                value = data;
            }
        }
        Ok(serde_json::from_value(value)?)
    }

    /// Convenience: parse from UTF-8 JSON string.
    pub fn parse(s: &str) -> Result<Self> {
        Self::from_slice(s.as_bytes())
    }
}

/// Delivery status stored in webhook logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookLogStatus {
    /// Queued / in flight.
    Pending,
    /// Delivered (proxy reported success).
    Success,
    /// Delivery failed.
    Fail,
    /// Unknown status string.
    #[serde(other)]
    Other,
}

/// One row from `GET /api/webhook-logs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookLog {
    /// Log id.
    pub id: i64,
    /// Webhook type string (`receiving_sms` / `rent_sms`).
    #[serde(default)]
    pub r#type: Option<String>,
    /// User id.
    #[serde(default)]
    pub user_id: Option<i64>,
    /// Target URL.
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// Payload that was sent (`params` column).
    #[serde(default)]
    pub params: Option<Value>,
    /// Delivery status.
    #[serde(default)]
    pub status: Option<String>,
    /// Error message when failed.
    #[serde(default)]
    pub error: Option<String>,
    /// Created at.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, Value>,
}

/// Paginated webhook logs (`data` after stripping API `response`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookLogsPage {
    /// Log rows.
    #[serde(default)]
    pub data: Vec<WebhookLog>,
    /// Total rows.
    #[serde(default)]
    pub total: Option<i64>,
    /// Per page.
    #[serde(default)]
    pub per_page: Option<i64>,
    /// Current page.
    #[serde(default)]
    pub current_page: Option<i64>,
    /// Last page.
    #[serde(default)]
    pub last_page: Option<i64>,
    /// Extra pagination fields.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WebhookLogsWrap {
    pub data: WebhookLogsPage,
}

/// Ensure a successful parse returns a clear error type for callers.
pub fn parse_webhook_json(json: &str) -> Result<WebhookPayload> {
    WebhookPayload::parse(json).map_err(|e| match e {
        Error::Json(err) => Error::Unexpected(format!("invalid webhook payload: {err}")),
        other => other,
    })
}
