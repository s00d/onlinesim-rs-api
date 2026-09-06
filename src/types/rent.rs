//! Types for number rent API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// SMS message inside a rent operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RentMessage {
    /// Message id.
    pub id: i64,
    /// Service name.
    #[serde(default)]
    pub service: Option<String>,
    /// Full text.
    #[serde(default)]
    pub text: Option<String>,
    /// Extracted code.
    #[serde(default)]
    pub code: Option<String>,
    /// Creation timestamp.
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Active or newly created rent item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RentItem {
    /// Operation id.
    pub tzid: i64,
    /// Status code.
    #[serde(default)]
    pub status: Option<i64>,
    /// Received messages.
    #[serde(default)]
    pub messages: Vec<RentMessage>,
    /// Country code.
    #[serde(default)]
    pub country: Option<i64>,
    /// Rent flag.
    #[serde(default)]
    pub rent: Option<i64>,
    /// Extension flag.
    #[serde(default)]
    pub extension: Option<i64>,
    /// Sum paid.
    #[serde(default)]
    pub sum: Option<f64>,
    /// Phone number.
    #[serde(default)]
    pub number: Option<String>,
    /// Remaining time.
    #[serde(default)]
    pub time: Option<i64>,
    /// Remaining hours.
    #[serde(default)]
    pub hours: Option<i64>,
    /// Extension price map entries.
    #[serde(default)]
    pub extend: Vec<HashMap<String, f64>>,
    /// Checked flag.
    #[serde(default)]
    pub checked: Option<bool>,
    /// Reload flag.
    #[serde(default)]
    pub reload: Option<i64>,
    /// Day extend flag.
    #[serde(default)]
    pub day_extend: Option<i64>,
    /// Catch-all for extra fields.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Rent tariff for a country.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RentTariff {
    /// Country code.
    pub code: i64,
    /// Enabled flag.
    pub enabled: bool,
    /// Country name.
    pub name: String,
    /// Newly added.
    #[serde(rename = "new")]
    pub is_new: bool,
    /// Sort position.
    pub position: i64,
    /// Count pricing.
    #[serde(default)]
    pub count: HashMap<String, f64>,
    /// Days pricing.
    #[serde(default)]
    pub days: HashMap<String, f64>,
    /// Extend price.
    #[serde(default)]
    pub extend: Option<f64>,
}
