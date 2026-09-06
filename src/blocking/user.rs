//! Blocking user / webhook API.

use serde_json::{json, Value};

use crate::error::Result;
use crate::types::user::{Balance, Pay, PayList, ProfileResponse, User};
use crate::types::webhook::{WebhookLogsPage, WebhookLogsWrap};

use super::http::BlockingHttp;

/// User / balance / webhook operations (blocking).
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

    /// User profile (includes `webhook_url`).
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

    /// Set profile webhook URL (`None` clears).
    pub fn set_webhook_url(&self, url: Option<&str>) -> Result<()> {
        let webhook_url = url.map(str::trim).filter(|s| !s.is_empty());
        let _: Value = self.http.post_onlinesim(
            "profile",
            json!({ "profile": { "webhook_url": webhook_url } }),
            true,
        )?;
        Ok(())
    }

    /// Clear profile webhook URL.
    pub fn clear_webhook_url(&self) -> Result<()> {
        self.set_webhook_url(None)
    }

    /// Paginated webhook delivery logs.
    pub fn webhook_logs(&self, page: i64) -> Result<WebhookLogsPage> {
        let wrap: WebhookLogsWrap =
            self.http
                .get_onlinesim("webhook-logs", json!({ "page": page }), false)?;
        Ok(wrap.data)
    }
}
