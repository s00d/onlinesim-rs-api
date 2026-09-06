//! Rust client for [OnlineSim](https://onlinesim.io) SMS API.
//!
//! # Features
//!
//! - `async` (default) — async [`Client`] based on `reqwest` + `tokio`
//! - `blocking` — synchronous [`blocking::Client`]
//! - `mock` — built-in [`mock::MockOnlineSim`] HTTP server for tests (no real numbers)
//!
//! Incoming SMS webhooks: [`WebhookPayload`] / [`parse_webhook_json`]; configure the URL
//! with `client.user().set_webhook_url(...)` and inspect delivery via `webhook_logs`.
//!
//! # Example
//!
//! ```no_run
//! # #[cfg(feature = "async")]
//! # async fn demo() -> onlinesim_rs_api::Result<()> {
//! use onlinesim_rs_api::Client;
//!
//! let client = Client::new("your-apikey")?;
//! let balance = client.user().balance().await?;
//! println!("balance = {}", balance.balance);
//! # Ok(())
//! # }
//! ```
//!
//! # Testing without real numbers
//!
//! ```no_run
//! # #[cfg(feature = "mock")]
//! # async fn demo() -> onlinesim_rs_api::Result<()> {
//! use onlinesim_rs_api::mock::{MockOnlineSim, SmsScript};
//! use onlinesim_rs_api::WaitCodeOptions;
//!
//! let mock = MockOnlineSim::start().await;
//! mock.script_sms(SmsScript { code: "999111".into(), ..SmsScript::default() });
//! let client = mock.client()?;
//! let tzid = client.numbers().get("telegram").await?;
//! let code = client.numbers().wait_code(tzid, WaitCodeOptions { interval_secs: 0, ..Default::default() }).await?;
//! assert_eq!(code, "999111");
//! # Ok(())
//! # }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod builder;
mod config;
mod error;
mod util;

#[cfg(any(feature = "async", feature = "blocking"))]
mod wait_code;

#[cfg(feature = "async")]
mod api;
#[cfg(feature = "async")]
mod client;
#[cfg(feature = "async")]
mod http;

#[cfg(feature = "blocking")]
pub mod blocking;

#[cfg(feature = "mock")]
#[cfg_attr(docsrs, doc(cfg(feature = "mock")))]
pub mod mock;

pub mod types;

pub use builder::ClientBuilder;
pub use config::{Config, DEFAULT_BASE_URL, DEFAULT_COUNTRY};
pub use error::{request_error_message, Error, Result};
pub use types::numbers::{GetNumberParams, WaitCodeOptions};
pub use types::webhook::{parse_webhook_json, WebhookPayload, WebhookType};

#[cfg(feature = "async")]
pub use client::Client;
