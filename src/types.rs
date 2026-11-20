//! Wire types. They mirror the schemas of the OnlineSim OpenAPI spec (snake_case field names as sent by the
//! server), with client-side normalisation: decimal-string money (`balance`, rent `sum`) becomes `f64`,
//! numbers sent as strings are parsed, and maps the server may send as an empty PHP array `[]` become empty
//! maps. Unknown fields are ignored.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::de;

// ----- numbers -----

/// State of a number operation ([`Operation::response`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OperationStatus {
    /// A number is being assigned.
    TzInpool,
    /// Waiting for an SMS.
    TzNumWait,
    /// An SMS arrived.
    TzNumAnswer,
    /// Finished.
    TzOverOk,
    /// Failed.
    ErrorNoItems,
    /// A state this SDK version does not know.
    #[serde(other)]
    Unknown,
}

impl OperationStatus {
    /// Wire value, e.g. `TZ_NUM_WAIT`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TzInpool => "TZ_INPOOL",
            Self::TzNumWait => "TZ_NUM_WAIT",
            Self::TzNumAnswer => "TZ_NUM_ANSWER",
            Self::TzOverOk => "TZ_OVER_OK",
            Self::ErrorNoItems => "ERROR_NO_ITEMS",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// One SMS of an operation when `msg_list` is set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationSms {
    /// Service slug.
    #[serde(default, deserialize_with = "de::text0")]
    pub service: String,
    /// Code or text, depending on `message_to_code`.
    #[serde(default, deserialize_with = "de::opt_text")]
    pub msg: Option<String>,
    /// Full text.
    #[serde(default, deserialize_with = "de::opt_text")]
    pub text: Option<String>,
    /// Extracted code.
    #[serde(default, deserialize_with = "de::opt_text")]
    pub code: Option<String>,
}

/// `msg` of an operation: the code (or full text) by default, every SMS with `msg_list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OperationMsg {
    /// Code, or the full SMS text with `message_to_code: false`.
    Text(String),
    /// All SMS (`msg_list: true`).
    List(Vec<OperationSms>),
}

impl OperationMsg {
    /// The text when this is [`OperationMsg::Text`].
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::List(_) => None,
        }
    }
}

fn opt_msg<'de, D: Deserializer<'de>>(d: D) -> Result<Option<OperationMsg>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(items) => Some(OperationMsg::List(
            serde_json::from_value(Value::Array(items)).map_err(serde::de::Error::custom)?,
        )),
        other => de::text(&other).map(OperationMsg::Text),
    })
}

/// Number operation (a `getState` item).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    /// Operation id.
    pub tzid: i64,
    /// State; `None` when no number could be issued.
    #[serde(default)]
    pub response: Option<OperationStatus>,
    /// Number with `+`.
    #[serde(default, deserialize_with = "de::opt_text")]
    pub number: Option<String>,
    /// Service slug.
    #[serde(default)]
    pub service: Option<String>,
    /// Country code.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub country: Option<i64>,
    /// Price, USD.
    #[serde(default, deserialize_with = "de::opt_num")]
    pub sum: Option<f64>,
    /// Seconds left.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub time: Option<i64>,
    /// Operation form (`index`, …).
    #[serde(default, deserialize_with = "de::opt_text")]
    pub form: Option<String>,
    /// Subscription package, when ordered through one.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub subscription_package_id: Option<i64>,
    /// Webhook URL of the operation.
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// `None` until an SMS arrives.
    #[serde(default, deserialize_with = "opt_msg")]
    pub msg: Option<OperationMsg>,
    /// Extension flag.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub extend: Option<i64>,
    /// Seconds until the operation may be closed.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub guard_interval_remaining_seconds: Option<i64>,
}

/// Result of `numbers().get_with_number`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NumberOrder {
    /// Operation id.
    pub tzid: i64,
    /// Full number with `+`, e.g. `+447459608320`.
    #[serde(deserialize_with = "de::text0")]
    pub number: String,
    /// Country code.
    #[serde(default, deserialize_with = "de::int0")]
    pub country: i64,
    /// Seconds left.
    #[serde(default, deserialize_with = "de::int0")]
    pub time: i64,
    /// Service slug.
    #[serde(default)]
    pub service: String,
    /// Service title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Seconds until the operation may be closed.
    #[serde(
        default,
        deserialize_with = "de::opt_int",
        skip_serializing_if = "Option::is_none"
    )]
    pub guard_interval_remaining_seconds: Option<i64>,
}

/// A service of a country in `tariffs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceStat {
    /// Numbers in stock; `None` when unknown (the server sends `"?"`).
    #[serde(default, deserialize_with = "de::opt_int")]
    pub count: Option<i64>,
    /// Popular service.
    #[serde(default)]
    pub popular: bool,
    /// Country code.
    #[serde(default, deserialize_with = "de::int0")]
    pub code: i64,
    /// Price, USD.
    #[serde(default, deserialize_with = "de::money")]
    pub price: f64,
    /// Service id.
    #[serde(default, deserialize_with = "de::int0")]
    pub id: i64,
    /// Display name.
    #[serde(default, deserialize_with = "de::text0")]
    pub service: String,
    /// Service id for `numbers().get` / `numbers().price`.
    #[serde(default, deserialize_with = "de::text0")]
    pub slug: String,
}

/// Services, prices and stock of one country (`getNumbersStats`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CountryStat {
    /// Country name.
    #[serde(default)]
    pub name: String,
    /// Localised name.
    #[serde(default)]
    pub locale_name: Option<String>,
    /// Sort position.
    #[serde(default, deserialize_with = "de::int0")]
    pub position: i64,
    /// Country code.
    #[serde(deserialize_with = "de::int0")]
    pub code: i64,
    /// Price of other services; `None` when not set (the server sends `false`).
    #[serde(default, deserialize_with = "de::opt_num")]
    pub other: Option<f64>,
    /// Newly added country.
    #[serde(rename = "new", default)]
    pub is_new: bool,
    /// Enabled.
    #[serde(default)]
    pub enabled: bool,
    /// `service_<slug>` → service.
    #[serde(default, deserialize_with = "de::map")]
    pub services: HashMap<String, ServiceStat>,
}

// ----- rent -----

/// One SMS of a rent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RentSms {
    /// Message id.
    #[serde(default, deserialize_with = "de::int0")]
    pub id: i64,
    /// Service slug.
    #[serde(default, deserialize_with = "de::text0")]
    pub service: String,
    /// Full text.
    #[serde(default, deserialize_with = "de::text0")]
    pub text: String,
    /// Extracted code, empty when none. A `notify` SMS (a prohibited sender) has the integer `0` on the wire,
    /// normalised to `"0"`.
    #[serde(default, deserialize_with = "de::text0")]
    pub code: String,
    /// Arrival time.
    #[serde(default, deserialize_with = "de::text0")]
    pub created_at: String,
}

/// Rent operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RentItem {
    /// Operation id.
    pub tzid: i64,
    /// `0`: price reserved, waiting for the first SMS; `1`: confirmed and charged.
    #[serde(default, deserialize_with = "de::int0")]
    pub status: i64,
    /// Number.
    #[serde(default, deserialize_with = "de::text0")]
    pub number: String,
    /// Country code.
    #[serde(default, deserialize_with = "de::int0")]
    pub country: i64,
    /// Cost incl. extensions, USD (the server sends a number, or a decimal string in getRentState). Reserved
    /// while `status` is 0, charged once the rent is confirmed.
    #[serde(default, deserialize_with = "de::money")]
    pub sum: f64,
    /// Rent days.
    #[serde(default, deserialize_with = "de::int0")]
    pub days: i64,
    /// Rent hours.
    #[serde(default, deserialize_with = "de::int0")]
    pub hours: i64,
    /// Minutes left.
    #[serde(default, deserialize_with = "de::int0")]
    pub time: i64,
    /// Seconds left.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub time_seconds: Option<i64>,
    /// Seconds until the rent may be closed.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub guard_interval_remaining_seconds: Option<i64>,
    /// 0 – not extendable, 1 – extendable, 2 – auto-extension.
    #[serde(default, deserialize_with = "de::int0")]
    pub extension: i64,
    /// Auto-extension days.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub days_autoextend: Option<i64>,
    /// Rent flag.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub rent: Option<i64>,
    /// Start time.
    #[serde(default, deserialize_with = "de::text0")]
    pub start_time: String,
    /// End time.
    #[serde(default, deserialize_with = "de::text0")]
    pub stop_time: String,
    /// Last check time.
    #[serde(default)]
    pub checked_time: Option<String>,
    /// Checked flag.
    #[serde(default)]
    pub checked: Option<bool>,
    /// Frozen.
    #[serde(default)]
    pub freeze: Option<bool>,
    /// Reload counter.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub reload: Option<i64>,
    /// Manual extension flag.
    #[serde(default)]
    pub m_ext: Option<bool>,
    /// Price of one day of extension.
    #[serde(default, deserialize_with = "de::opt_num")]
    pub day_extend: Option<f64>,
    /// Days → extension price; empty when not extendable.
    #[serde(default, deserialize_with = "de::num_map")]
    pub extend: HashMap<String, f64>,
    /// Webhook URL of the rent.
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// SMS, newest first.
    #[serde(default, deserialize_with = "de::list")]
    pub messages: Vec<RentSms>,
}

/// Rent prices of one country.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RentTariff {
    /// Country code.
    #[serde(deserialize_with = "de::int0")]
    pub code: i64,
    /// Country name.
    #[serde(default)]
    pub name: String,
    /// Enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Newly added.
    #[serde(rename = "new", default)]
    pub is_new: bool,
    /// Sort position.
    #[serde(default, deserialize_with = "de::int0")]
    pub position: i64,
    /// Extension allowed (`1`) or not (`0`).
    #[serde(default, deserialize_with = "de::int0")]
    pub extend: i64,
    /// Needs confirmation.
    #[serde(default)]
    pub confirm: Option<bool>,
    /// Rent days → price, USD.
    #[serde(default, deserialize_with = "de::num_map")]
    pub days: HashMap<String, f64>,
    /// Rent days → numbers available.
    #[serde(default, deserialize_with = "de::int_map")]
    pub count: HashMap<String, i64>,
    /// Rent days → numbers available for extension.
    #[serde(default, deserialize_with = "de::int_map")]
    pub ext_counts: HashMap<String, i64>,
}

// ----- user -----

/// Account balance, USD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    /// Total balance. Includes `zbalance`: available = balance − max(zbalance, 0).
    #[serde(default, deserialize_with = "de::money")]
    pub balance: f64,
    /// Sum frozen by numbers and unconfirmed rents in progress; may be negative.
    #[serde(default, deserialize_with = "de::money")]
    pub zbalance: f64,
    /// Partner income.
    #[serde(
        default,
        deserialize_with = "de::opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub income: Option<f64>,
}

/// Account profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// User id.
    pub id: i64,
    /// Public hash id.
    #[serde(default)]
    pub hash_id: Option<String>,
    /// Name.
    #[serde(default)]
    pub name: Option<String>,
    /// Username.
    #[serde(default)]
    pub username: Option<String>,
    /// E-mail.
    #[serde(default)]
    pub email: Option<String>,
    /// E-mail verification time.
    #[serde(default)]
    pub email_verified_at: Option<String>,
    /// API key.
    #[serde(default)]
    pub apikey: Option<String>,
    /// `true`, or a map of allowed IPs.
    #[serde(default)]
    pub api_access: Option<Value>,
    /// Locale.
    #[serde(default)]
    pub locale: Option<String>,
    /// Currency.
    #[serde(default)]
    pub currency: Option<String>,
    /// Default country for orders.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub number_country: Option<i64>,
    /// Default reject list (a list or a map).
    #[serde(default)]
    pub number_reject: Option<Value>,
    /// Webhook URL.
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// Phone number (digits).
    #[serde(default, deserialize_with = "de::opt_int")]
    pub number: Option<i64>,
    /// Phone country.
    #[serde(default, deserialize_with = "de::opt_text")]
    pub country: Option<String>,
    /// Phone region.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub number_region: Option<i64>,
    /// API user.
    #[serde(default)]
    pub is_api: Option<bool>,
    /// Active flag (as sent).
    #[serde(default, deserialize_with = "de::opt_text")]
    pub active: Option<String>,
    /// Blocked flag.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub block: Option<i64>,
    /// Disabled.
    #[serde(default)]
    pub disable: Option<bool>,
    /// Verification level.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub verify: Option<i64>,
    /// User group.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub ugroup: Option<i64>,
    /// Subscription flag.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub sub: Option<i64>,
    /// First login.
    #[serde(default)]
    pub first_login: Option<bool>,
    /// Project subscription.
    #[serde(default)]
    pub is_project_subscription: Option<bool>,
    /// Password reset counter.
    #[serde(default, rename = "resetCount", deserialize_with = "de::opt_int")]
    pub reset_count: Option<i64>,
    /// Registration time.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Last notification time.
    #[serde(default)]
    pub notification_at: Option<String>,
}

/// Laravel paginator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: serde::de::DeserializeOwned"))]
pub struct Paginator<T> {
    /// Current page.
    #[serde(default, deserialize_with = "de::int0")]
    pub current_page: i64,
    /// Items of the page.
    #[serde(default = "Vec::new", deserialize_with = "de::list")]
    pub data: Vec<T>,
    /// Index of the first item.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub from: Option<i64>,
    /// Index of the last item.
    #[serde(default, deserialize_with = "de::opt_int")]
    pub to: Option<i64>,
    /// Last page.
    #[serde(default, deserialize_with = "de::int0")]
    pub last_page: i64,
    /// Items per page.
    #[serde(default, deserialize_with = "de::int0")]
    pub per_page: i64,
    /// Items in total.
    #[serde(default, deserialize_with = "de::int0")]
    pub total: i64,
    /// First page URL.
    #[serde(default)]
    pub first_page_url: Option<String>,
    /// Last page URL.
    #[serde(default)]
    pub last_page_url: Option<String>,
    /// Next page URL.
    #[serde(default)]
    pub next_page_url: Option<String>,
    /// Previous page URL.
    #[serde(default)]
    pub prev_page_url: Option<String>,
    /// Base path.
    #[serde(default)]
    pub path: Option<String>,
}

impl<T> Default for Paginator<T> {
    fn default() -> Self {
        Self {
            current_page: 0,
            data: Vec::new(),
            from: None,
            to: None,
            last_page: 0,
            per_page: 0,
            total: 0,
            first_page_url: None,
            last_page_url: None,
            next_page_url: None,
            prev_page_url: None,
            path: None,
        }
    }
}

/// A top-up order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaymentOrder {
    /// Order id.
    pub id: i64,
    /// Decimal string as sent, e.g. `"89.00"`.
    #[serde(default, deserialize_with = "de::text0")]
    pub sum: String,
    /// Currency.
    #[serde(default)]
    pub currency: Option<String>,
    /// Status code.
    #[serde(default, deserialize_with = "de::int0")]
    pub stat: i64,
    /// Payment method text.
    #[serde(default)]
    pub form_text: Option<String>,
    /// Status text.
    #[serde(default)]
    pub status_text: Option<String>,
    /// Creation time.
    #[serde(default)]
    pub created_at: String,
    /// Other fields as sent.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Top-up history and payment methods.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaymentHistory {
    /// Orders, newest first.
    #[serde(default)]
    pub orders: Paginator<PaymentOrder>,
    /// Payment methods by group id. The server sends a JSON array when the ids are 0..n; it is kept, keyed
    /// `"0"`, `"1"`, ….
    #[serde(default, deserialize_with = "de::index_map")]
    pub paylist: HashMap<String, Value>,
    /// Payment domains.
    #[serde(default, deserialize_with = "de::list")]
    pub domains: Vec<String>,
    /// Payment forms.
    #[serde(default, deserialize_with = "de::map")]
    pub forms: HashMap<String, Value>,
    /// Currency rates.
    #[serde(default, deserialize_with = "de::num_map")]
    pub currency: HashMap<String, f64>,
    /// Quick top-up buttons.
    #[serde(default, deserialize_with = "de::map")]
    pub buttons: HashMap<String, Value>,
}

/// Type of an inbound webhook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookType {
    /// SMS of a number operation.
    ReceivingSms,
    /// SMS of a rent.
    RentSms,
}

/// One webhook delivery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookLog {
    /// Log id.
    pub id: i64,
    /// User id.
    #[serde(default, deserialize_with = "de::int0")]
    pub user_id: i64,
    /// Target URL.
    #[serde(default)]
    pub webhook_url: String,
    /// `receiving_sms` or `rent_sms` (as sent).
    #[serde(rename = "type", default)]
    pub kind: String,
    /// Payload as sent by the server (not normalised; see [`parse_webhook_value`](crate::parse_webhook_value)).
    #[serde(default)]
    pub params: Value,
    /// `success`, `fail` or `pending`.
    #[serde(default)]
    pub status: String,
    /// Delivery error.
    #[serde(default)]
    pub error: Option<String>,
    /// Creation time.
    #[serde(default)]
    pub created_at: String,
    /// Update time.
    #[serde(default)]
    pub updated_at: String,
}

// ----- free -----

/// A country with free numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeCountry {
    /// Country code.
    #[serde(deserialize_with = "de::int0")]
    pub country: i64,
    /// Localised name.
    #[serde(default)]
    pub country_text: Option<String>,
    /// Name.
    #[serde(default)]
    pub country_original: Option<String>,
}

/// A free number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeNumber {
    /// Country code.
    #[serde(default, deserialize_with = "de::int0")]
    pub country: i64,
    /// Country name.
    #[serde(default)]
    pub country_original: Option<String>,
    /// Number with the country code, no `+`.
    #[serde(default, deserialize_with = "de::text0")]
    pub full_number: String,
    /// Age, human readable.
    #[serde(default)]
    pub data_humans: String,
    /// Archived.
    #[serde(default)]
    pub is_archive: bool,
}

/// An SMS of a free number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeMessage {
    /// Message id.
    #[serde(default, deserialize_with = "de::int0")]
    pub id: i64,
    /// Text.
    #[serde(default)]
    pub text: String,
    /// Sender.
    #[serde(default, deserialize_with = "de::text0")]
    pub in_number: String,
    /// Receiving number.
    #[serde(default, deserialize_with = "de::text0")]
    pub my_number: String,
    /// Code; `-` when none was found.
    #[serde(default, deserialize_with = "de::text0")]
    pub code: String,
    /// Arrival time.
    #[serde(default)]
    pub created_at: String,
    /// Age, human readable.
    #[serde(default)]
    pub data_humans: String,
}

/// SMS page of the selected free number.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FreeMessages {
    /// The page.
    #[serde(flatten)]
    pub page: Paginator<FreeMessage>,
    /// Selected number (without the country code).
    #[serde(default, deserialize_with = "de::opt_text")]
    pub number: Option<String>,
    /// Selected country.
    #[serde(default, deserialize_with = "de::int0")]
    pub country: i64,
}

/// Result of `free().list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeList {
    /// Countries with free numbers.
    #[serde(default, deserialize_with = "de::list")]
    pub countries: Vec<FreeCountry>,
    /// Numbers of the selected country, keyed by number without the country code.
    #[serde(default, deserialize_with = "de::map")]
    pub numbers: HashMap<String, FreeNumber>,
    /// SMS of the selected number.
    #[serde(default)]
    pub messages: FreeMessages,
}
