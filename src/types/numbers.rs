//! Types for temporary SMS numbers (`numbers` API).

use std::collections::HashMap;

use crate::config::DEFAULT_COUNTRY;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Single operation state from `getState`.
///
/// Matches OnlineSim `ReceivingApiController@getState` / model appends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateOne {
    /// Operation id.
    pub tzid: i64,
    /// Operation status (`TZ_NUM_WAIT`, `TZ_NUM_ANSWER`, `TZ_OVER_OK`, …).
    #[serde(default)]
    pub response: Option<String>,
    /// Phone number (`+{cc}{n}` string or numeric legacy).
    #[serde(default)]
    pub number: Option<Value>,
    /// Service slug.
    #[serde(default)]
    pub service: Option<String>,
    /// Remaining time (seconds / display depending on API flags).
    #[serde(default)]
    pub time: Option<i64>,
    /// SMS code or message payload (`string`, list, or object depending on flags).
    #[serde(default)]
    pub msg: Option<Value>,
    /// Whether extension is available (`0`/`1`).
    #[serde(default)]
    pub extend: Option<i64>,
    /// Country dial code.
    #[serde(default)]
    pub country: Option<i64>,
    /// Operation sum / price.
    #[serde(default, deserialize_with = "crate::util::de_opt_f64_flexible")]
    pub sum: Option<f64>,
    /// Form / channel marker from API.
    #[serde(default)]
    pub form: Option<Value>,
    /// Human-readable status text (when provided).
    #[serde(default)]
    pub response_text: Option<String>,
    /// Service title.
    #[serde(default)]
    pub title: Option<String>,
    /// Remaining guard interval before close/ban is allowed.
    #[serde(default)]
    pub guard_interval_remaining_seconds: Option<i64>,
    /// Extra fields from the API.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Service stats inside a country tariff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TariffService {
    /// Available count.
    pub count: i64,
    /// Popular flag.
    pub popular: bool,
    /// Price (API may send `number_format` string).
    #[serde(deserialize_with = "crate::util::de_f64_flexible")]
    pub price: f64,
    /// Service id.
    pub id: Value,
    /// Service name/id.
    pub service: Value,
    /// Slug.
    pub slug: Value,
}

/// Tariffs for one country (`getNumbersStats`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TariffCountryOne {
    /// Country name.
    pub name: String,
    /// Sort position.
    pub position: i64,
    /// Country code.
    pub code: i64,
    /// Newly added country.
    #[serde(rename = "new")]
    pub is_new: bool,
    /// Enabled flag.
    pub enabled: bool,
    /// Localized name when present.
    #[serde(default)]
    pub locale_name: Option<String>,
    /// Services map.
    #[serde(default)]
    pub services: HashMap<String, TariffService>,
}

/// Result of `get_with_number` / enriched `getNum` when `number=true`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NumberWithTz {
    /// Operation id.
    pub tzid: i64,
    /// Phone number.
    pub number: String,
    /// Country code.
    pub country: i64,
    /// Remaining time when provided by API.
    #[serde(default)]
    pub time: Option<i64>,
    /// Service slug when provided.
    #[serde(default)]
    pub service: Option<String>,
    /// Service title when provided.
    #[serde(default)]
    pub title: Option<String>,
    /// Status text when provided.
    #[serde(default)]
    pub response_text: Option<String>,
    /// Guard interval seconds when provided.
    #[serde(default)]
    pub guard_interval_remaining_seconds: Option<i64>,
}

/// Options for [`crate::api::numbers::NumbersApi::wait_code`] / blocking twin.
///
/// Concurrent waits on one client share a single batch `getState` poller.
#[derive(Debug, Clone)]
pub struct WaitCodeOptions {
    /// Poll interval in **seconds** (default: `3`).
    ///
    /// When several `wait_code` calls share a poller, the **minimum** interval wins.
    pub interval_secs: u64,
    /// Maximum number of poll attempts (default: `10`, matching JS attempt cap).
    pub max_attempts: u32,
    /// If true, call `next` instead of `close` when a code arrives.
    pub not_end: bool,
    /// If true, request full message text (`message_to_code = 0`).
    ///
    /// If any shared waiter sets this, the batch poll uses full messages.
    pub full_message: bool,
}

impl Default for WaitCodeOptions {
    fn default() -> Self {
        Self {
            interval_secs: 3,
            max_attempts: 10,
            not_end: false,
            full_message: false,
        }
    }
}

/// Parameters for ordering a number.
#[derive(Debug, Clone)]
pub struct GetNumberParams<'a> {
    /// Service slug (required).
    pub service: &'a str,
    /// Country dial code (default [`DEFAULT_COUNTRY`]).
    pub country: i64,
    /// Rejected number ids.
    pub reject: &'a [i64],
    /// Request number extension capability.
    pub extension: bool,
}

impl<'a> GetNumberParams<'a> {
    /// Create params with defaults (`country` = [`DEFAULT_COUNTRY`], empty reject, `extension=false`).
    pub fn new(service: &'a str) -> Self {
        Self {
            service,
            country: DEFAULT_COUNTRY,
            reject: &[],
            extension: false,
        }
    }

    /// Set country.
    pub fn country(mut self, country: i64) -> Self {
        self.country = country;
        self
    }

    /// Set reject list.
    pub fn reject(mut self, reject: &'a [i64]) -> Self {
        self.reject = reject;
        self
    }

    /// Set extension flag.
    pub fn extension(mut self, extension: bool) -> Self {
        self.extension = extension;
        self
    }
}
