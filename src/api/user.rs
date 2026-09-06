//! User profile, payments, and webhook settings API.

use serde_json::{json, Value};

use crate::error::Result;
use crate::http::Http;
use crate::types::user::{Balance, Pay, PayList, ProfileResponse, User};
use crate::types::webhook::{WebhookLogsPage, WebhookLogsWrap};

/// User / balance / payment / webhook operations.
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

    /// User profile (includes [`User::webhook_url`]).
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

    /// Set the profile webhook URL that receives SMS payloads.
    ///
    /// Pass `None` or empty string to clear. Backend also updates active
    /// receiving/rent operations when the URL changes (`POST profile.php`).
    pub async fn set_webhook_url(&self, url: Option<&str>) -> Result<()> {
        let webhook_url = url.map(str::trim).filter(|s| !s.is_empty());
        let _: Value = self
            .http
            .post_onlinesim(
                "profile",
                json!({ "profile": { "webhook_url": webhook_url } }),
                true,
            )
            .await?;
        Ok(())
    }

    /// Clear the profile webhook URL.
    pub async fn clear_webhook_url(&self) -> Result<()> {
        self.set_webhook_url(None).await
    }

    /// Paginated webhook delivery logs (`GET /api/webhook-logs`).
    pub async fn webhook_logs(&self, page: i64) -> Result<WebhookLogsPage> {
        let wrap: WebhookLogsWrap = self
            .http
            .get_onlinesim("webhook-logs", json!({ "page": page }), false)
            .await?;
        Ok(wrap.data)
    }
}
