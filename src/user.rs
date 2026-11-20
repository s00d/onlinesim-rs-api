//! Account: `client.user()`.

use serde_json::{Value, json};

use crate::error::Error;
use crate::types::{Balance, Paginator, PaymentHistory, Profile, WebhookLog};
use crate::wire::{Call, Request, decode};

/// Account. Get it with `client.user()`.
#[derive(Debug, Clone, Copy)]
pub struct User<'a, C>(pub(crate) &'a C);

section_methods!(User {
    /// Balance in USD. `balance` includes the frozen `zbalance`: available = balance − max(zbalance, 0).
    fn balance() -> Balance;

    /// Account profile.
    fn profile() -> Profile;

    /// Top-up orders (paginated, newest first) and the available payment methods. Pages start at 1.
    fn payment_history(page: u32) -> PaymentHistory;

    /// Set the URL that receives SMS webhooks. `ApiError` `VALIDATION_ERROR` for an invalid URL.
    fn set_webhook_url(url: &str) -> ();

    /// Remove the webhook URL (also from operations in progress).
    fn clear_webhook_url() -> ();

    /// Webhook delivery log, newest first, 10 per page. Pages start at 1.
    fn webhook_logs(page: u32) -> Paginator<WebhookLog>;
});

fn balance() -> Call<Balance> {
    Call::new(Request::get("getBalance.php").arg("income", 1), |res| {
        decode(res?)
    })
}

fn profile() -> Call<Profile> {
    Call::new(Request::get("getProfile.php"), |res| field(res?, "profile"))
}

fn payment_history(page: u32) -> Call<PaymentHistory> {
    Call::new(
        Request::get("getPaymentHistory.php").arg("page", page),
        |res| decode(res?),
    )
}

fn set_webhook_url(url: &str) -> Call<()> {
    Call::new(
        Request::post("profile.php", json!({ "profile": { "webhook_url": url } })),
        |res| res.map(drop),
    )
}

fn clear_webhook_url() -> Call<()> {
    Call::new(
        Request::post("profile.php", json!({ "profile": { "webhook_url": null } })),
        |res| res.map(drop),
    )
}

fn webhook_logs(page: u32) -> Call<Paginator<WebhookLog>> {
    Call::new(Request::get("webhook-logs").arg("page", page), |res| {
        field(res?, "data")
    })
}

fn field<T: serde::de::DeserializeOwned>(mut value: Value, key: &str) -> crate::Result<T> {
    match value.get_mut(key) {
        Some(v) => decode(v.take()),
        None => Err(Error::Decode(format!("missing {key}"))),
    }
}
