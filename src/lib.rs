//! Typed Rust client for the [OnlineSim](https://onlinesim.io) API: SMS numbers, rent, account and free
//! numbers. Docs: <https://s00d.github.io/onlinesim-rs-api/>.
//!
//! # Features
//!
//! - `async` (default): [`Client`] on `reqwest` + `tokio`.
//! - `blocking`: [`blocking::Client`], the same methods without `.await`.
//! - `mock-transport`: [`mock::MockOnlineSim`], an in-process fake OnlineSim API for your own tests (no extra
//!   dependencies; use it with `async` or `blocking`).
//! - `mock`: `mock-transport` plus [`mock::MockOnlineSim::start`], the same fake over local HTTP (wiremock).
//!
//! # Example
//!
//! ```no_run
//! # #[cfg(feature = "async")]
//! # async fn demo() -> onlinesim_rs_api::Result<()> {
//! use onlinesim_rs_api::{Client, WaitCodeOptions};
//!
//! let client = Client::new("your-api-key")?;
//! let balance = client.user().balance().await?;
//! println!("available: {} USD", balance.balance - balance.zbalance);
//!
//! let order = client.numbers().get_with_number("google", 44, &[]).await?;
//! println!("enter {} in the app", order.number);
//! let code = client.numbers().wait_code(order.tzid, WaitCodeOptions::default()).await?;
//! println!("code: {code}");
//! # Ok(())
//! # }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod de;
mod error;
mod webhook;

/// Modules that need a client (feature `async` or `blocking`).
macro_rules! with_client {
    ($($item:item)*) => { $( #[cfg(any(feature = "async", feature = "blocking"))] $item )* };
}

with_client! {
    #[macro_use]
    mod section;

    mod client;
    mod free;
    mod numbers;
    mod rent;
    mod transport;
    mod user;
    mod wait;
    mod wire;

    pub use client::ClientBuilder;
    pub use free::{Free, FreeListOptions};
    pub use numbers::{Numbers, OrderBy, StateOneOptions, StateOptions};
    pub use rent::Rent;
    pub use transport::{BlockingTransport, BoxFuture, HttpRequest, HttpResponse, Method, Transport};
    pub use user::User;
    pub use wait::{Clock, SystemClock, WaitCodeOptions};
    pub use wire::{DEFAULT_BASE_URL, DEFAULT_TIMEOUT};
}

#[cfg(feature = "blocking")]
#[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]
pub mod blocking;

#[cfg(all(
    feature = "mock-transport",
    any(feature = "async", feature = "blocking")
))]
#[cfg_attr(docsrs, doc(cfg(feature = "mock-transport")))]
pub mod mock;

pub mod types;

#[cfg(feature = "async")]
pub use client::Client;
pub use error::{ApiError, Error, Result, error_message};
pub use webhook::{WebhookPayload, parse_webhook, parse_webhook_value};
