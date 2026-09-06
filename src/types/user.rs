//! Types for user / balance API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Account balance snapshot (`getBalance`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    /// Main balance (`payment`).
    pub balance: f64,
    /// Frozen / held balance (`now`).
    #[serde(default)]
    pub zbalance: f64,
    /// Income when `income=true`.
    #[serde(default)]
    pub income: Option<f64>,
    /// Income in USD when provided.
    #[serde(default)]
    pub income_usd: Option<f64>,
}

/// Payment summary inside a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPayment {
    /// Payment total.
    #[serde(default)]
    pub payment: f64,
    /// Income.
    #[serde(default)]
    pub income: f64,
    /// Spent.
    #[serde(default)]
    pub spent: f64,
    /// Current balance.
    #[serde(default)]
    pub now: f64,
}

/// User profile (`getProfile`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    /// User id.
    pub id: i64,
    /// Display name.
    #[serde(default)]
    pub name: Option<String>,
    /// Username.
    #[serde(default)]
    pub username: Option<String>,
    /// Email.
    #[serde(default)]
    pub email: Option<String>,
    /// API key.
    #[serde(default)]
    pub apikey: Option<String>,
    /// API access flag (bool or string in API).
    #[serde(default)]
    pub api_access: Option<Value>,
    /// Locale (`ru` / `en` / `zh` / `de`, …).
    #[serde(default)]
    pub locale: Option<String>,
    /// Preferred number region.
    #[serde(default)]
    pub number_region: Option<i64>,
    /// Preferred number country.
    #[serde(default)]
    pub number_country: Option<i64>,
    /// Rejected numbers.
    #[serde(default)]
    pub number_reject: Option<Vec<String>>,
    /// URL that receives SMS webhooks (`POST` JSON). Empty / null disables webhooks.
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// User group.
    #[serde(default)]
    pub ugroup: Option<i64>,
    /// Verify status.
    #[serde(default)]
    pub verify: Option<i64>,
    /// Block status.
    #[serde(default)]
    pub block: Option<i64>,
    /// Payment block.
    #[serde(default)]
    pub payment: Option<UserPayment>,
    /// Extra fields from Eloquent serialization.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Single payment order row inside history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PayOrder {
    /// Order id.
    pub id: i64,
    /// User id.
    #[serde(default)]
    pub id_user: Option<i64>,
    /// Status (`0`/`1`/`2`).
    #[serde(default)]
    pub stat: Option<i64>,
    /// Sum as string.
    #[serde(default)]
    pub sum: Option<String>,
    /// Cashback.
    #[serde(default)]
    pub cashback: Option<f64>,
    /// Form id.
    #[serde(default)]
    pub form: Option<i64>,
    /// Sale id.
    #[serde(default)]
    pub id_sale: Option<String>,
    /// Promo code.
    #[serde(default)]
    pub promo: Option<String>,
    /// Original sum.
    #[serde(default)]
    pub original_sum: Option<f64>,
    /// Currency code.
    #[serde(default)]
    pub currency: Option<String>,
    /// Paid at.
    #[serde(default)]
    pub pay_at: Option<String>,
    /// Created at.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Updated at.
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Paginated orders block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PayOrdersPage {
    /// Current page.
    #[serde(default)]
    pub current_page: Option<i64>,
    /// First page URL.
    #[serde(default)]
    pub first_page_url: Option<String>,
    /// From index.
    #[serde(default)]
    pub from: Option<i64>,
    /// Last page.
    #[serde(default)]
    pub last_page: Option<i64>,
    /// Last page URL.
    #[serde(default)]
    pub last_page_url: Option<String>,
    /// Order rows (API may use map or array).
    #[serde(default)]
    pub data: Value,
    /// Extra pagination fields.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Payment method entry in `paylist`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PayMethod {
    /// List id.
    #[serde(default)]
    pub list_id: Option<i64>,
    /// Enabled.
    #[serde(default)]
    pub enable: Option<bool>,
    /// Method id.
    #[serde(default)]
    pub id: Option<i64>,
    /// Internal name.
    #[serde(default)]
    pub name: Option<String>,
    /// Title.
    #[serde(default)]
    pub title: Option<String>,
    /// Image URL.
    #[serde(default)]
    pub image: Option<String>,
    /// Invert flag.
    #[serde(default)]
    pub invert: Option<bool>,
    /// Extra add fields.
    #[serde(default)]
    pub add: HashMap<String, String>,
    /// Percent coefficient.
    #[serde(default, rename = "coofPersent")]
    pub coeff_percent: Option<f64>,
    /// Supported currencies map.
    #[serde(default)]
    pub curr: HashMap<String, Value>,
    /// Links method.
    #[serde(default)]
    pub links_method: Option<String>,
    /// Quick-sum links.
    #[serde(default)]
    pub links: Option<Value>,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Payment history / paylist payload (`getPaymentHistory`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PayList {
    /// Payment forms map.
    #[serde(default)]
    pub forms: HashMap<String, Value>,
    /// Currency rates.
    #[serde(default)]
    pub currency: HashMap<String, f64>,
    /// Orders pagination object.
    #[serde(default)]
    pub orders: Option<PayOrdersPage>,
    /// Available pay methods.
    #[serde(default)]
    pub paylist: HashMap<String, PayMethod>,
    /// Domains block when present.
    #[serde(default)]
    pub domains: Option<Value>,
    /// Buttons block when present.
    #[serde(default)]
    pub buttons: Option<Value>,
    /// Extra fields.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Payment create params (`createEmpty` / `_loader::createPayment`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PayParams {
    /// Redirect / pay URL.
    #[serde(default)]
    pub url: Option<String>,
    /// Button label.
    #[serde(default)]
    pub label: Option<String>,
    /// Sum.
    #[serde(default)]
    pub sum: Option<String>,
    /// Comment.
    #[serde(default)]
    pub comment: Option<String>,
    /// Open in new tab.
    #[serde(default)]
    pub newtab: Option<bool>,
    /// noreferrer flag.
    #[serde(default)]
    pub noreferrer: Option<bool>,
    /// Extra driver-specific fields.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Result of creating an empty payment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pay {
    /// Payment id.
    #[serde(rename = "payId")]
    pub pay_id: i64,
    /// Payment parameters.
    #[serde(default)]
    pub params: PayParams,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProfileResponse {
    pub profile: User,
}
