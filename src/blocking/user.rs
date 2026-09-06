//! Blocking user API.

use serde_json::{json, Value};

use crate::error::Result;
use crate::types::user::{Balance, Pay, PayList, ProfileResponse, User};

use super::http::BlockingHttp;

/// User / balance operations (blocking).
#[derive(Debug, Clone)]
pub struct UserApi {
    pub(crate) http: BlockingHttp,
}

impl UserApi {
    /// Account balance.
    pub fn balance(&self) -> Result<Balance> {
        self.http
            .get_onlinesim("getBalance", json!({ "income": true }), true)
    }

    /// User profile.
    pub fn profile(&self) -> Result<User> {
        let resp: ProfileResponse =
            self.http
                .get_onlinesim("getProfile", json!({ "income": true }), true)?;
        Ok(resp.profile)
    }

    /// Payment history.
    pub fn payment_history(&self) -> Result<PayList> {
        self.http
            .get_onlinesim("getPaymentHistory", json!({}), true)
    }

    /// Create an empty payment.
    pub fn create_empty(&self, params: Value) -> Result<Pay> {
        self.http.get_onlinesim("pay/createEmpty", params, false)
    }
}
