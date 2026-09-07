//! Types for free public numbers API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Country entry from free country list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeCountry {
    /// Country code.
    pub country: i64,
    /// Localized country name.
    pub country_text: String,
    /// Original country name.
    #[serde(default)]
    pub country_original: Option<String>,
}

/// Free phone number entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeNumber {
    /// Max date string.
    #[serde(default)]
    pub maxdate: Option<String>,
    /// Number (API may send string or integer).
    #[serde(deserialize_with = "crate::util::de_string_flexible")]
    pub number: String,
    /// Country code.
    pub country: i64,
    /// Last update.
    #[serde(default)]
    pub updated_at: Option<String>,
    /// Human-readable date.
    #[serde(default)]
    pub data_humans: Option<String>,
    /// Full international number.
    #[serde(default)]
    pub full_number: Option<String>,
    /// Country label.
    #[serde(default)]
    pub country_text: Option<String>,
}

/// Free SMS message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeMessage {
    /// Message text.
    pub text: String,
    /// Sender number.
    #[serde(default)]
    pub in_number: Option<String>,
    /// Receiver number (API may send string or integer).
    #[serde(default, deserialize_with = "crate::util::de_opt_string_flexible")]
    pub my_number: Option<String>,
    /// Created at.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Human-readable date.
    #[serde(default)]
    pub data_humans: Option<String>,
}

/// Number entry inside `free_list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeListNumber {
    /// Country code.
    pub country: i64,
    /// Original country name.
    #[serde(default)]
    pub country_original: Option<String>,
    /// Human-readable date.
    #[serde(default)]
    pub data_humans: Option<String>,
    /// Full number.
    #[serde(default)]
    pub full_number: Option<String>,
    /// Archive flag.
    #[serde(default)]
    pub is_archive: Option<bool>,
}

/// Paginated messages block inside `free_list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeMessagesPage {
    /// Current page.
    #[serde(default)]
    pub current_page: Option<i64>,
    /// From index.
    #[serde(default)]
    pub from: Option<i64>,
    /// Last page.
    #[serde(default)]
    pub last_page: Option<i64>,
    /// Per page.
    #[serde(default)]
    pub per_page: Option<i64>,
    /// To index.
    #[serde(default)]
    pub to: Option<i64>,
    /// Total.
    #[serde(default)]
    pub total: Option<i64>,
    /// Number filter.
    #[serde(default)]
    pub number: Option<String>,
    /// Country filter.
    #[serde(default)]
    pub country: Option<i64>,
    /// Messages.
    #[serde(default)]
    pub data: Vec<FreeMessage>,
}

/// Combined free list response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeListResponse {
    /// Countries.
    #[serde(default)]
    pub countries: Vec<FreeCountry>,
    /// Numbers map.
    #[serde(default)]
    pub numbers: HashMap<String, FreeListNumber>,
    /// Messages page.
    #[serde(default)]
    pub messages: Option<FreeMessagesPage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CountriesWrap {
    pub countries: Vec<FreeCountry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NumbersWrap {
    pub numbers: Vec<FreeNumber>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MessagesWrap {
    pub messages: FreeMessagesPage,
}
