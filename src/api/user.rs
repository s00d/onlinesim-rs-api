//! User profile and payments API.

use serde_json::{json, Value};

use crate::error::Result;
use crate::http::Http;
use crate::types::user::{Balance, Pay, PayList, ProfileResponse, User};

/// User / balance / payment operations.
#[derive(Debug, Clone)]
pub struct UserApi {
    pub(crate) http: Http,
}

impl UserApi {
    /// Account balance (includes `income`).
    pub async fn balance(&self) -> Result<Balance> {
        self.http
            .get_onlinesim("getBalance", json!({ "income": true }), true)
            .await
    }

    /// User profile.
    pub async fn profile(&self) -> Result<User> {
        let resp: ProfileResponse = self
            .http
            .get_onlinesim("getProfile", json!({ "income": true }), true)
            .await?;
        Ok(resp.profile)
    }

    /// Payment history.
    pub async fn payment_history(&self) -> Result<PayList> {
        self.http
            .get_onlinesim("getPaymentHistory", json!({}), true)
            .await
    }

    /// Create an empty payment (`pay/createEmpty`, no `.php` suffix).
    pub async fn create_empty(&self, params: Value) -> Result<Pay> {
        self.http
            .get_onlinesim("pay/createEmpty", params, false)
            .await
    }
}
