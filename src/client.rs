//! Async client.

use std::sync::Arc;

use crate::api::free::FreeApi;
use crate::api::numbers::NumbersApi;
use crate::api::rent::RentApi;
use crate::api::user::UserApi;
use crate::builder::ClientBuilder;
use crate::config::Config;
use crate::error::Result;
use crate::http::Http;
use crate::wait_code::async_poller::AsyncWaitPoller;

/// Async OnlineSim client.
#[derive(Debug, Clone)]
pub struct Client {
    http: Http,
    wait_poller: Arc<AsyncWaitPoller>,
}

impl Client {
    /// Start a builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Convenience: client with only an API key.
    pub fn new(apikey: impl Into<String>) -> Result<Self> {
        ClientBuilder::new().apikey(apikey).build()
    }

    pub(crate) fn from_config(config: Config) -> Result<Self> {
        Ok(Self {
            http: Http::new(config)?,
            wait_poller: Arc::new(AsyncWaitPoller::default()),
        })
    }

    /// Temporary SMS numbers API.
    pub fn numbers(&self) -> NumbersApi {
        NumbersApi {
            http: self.http.clone(),
            wait_poller: self.wait_poller.clone(),
        }
    }

    /// Number rent API.
    pub fn rent(&self) -> RentApi {
        RentApi {
            http: self.http.clone(),
        }
    }

    /// User / balance API.
    pub fn user(&self) -> UserApi {
        UserApi {
            http: self.http.clone(),
        }
    }

    /// Free public numbers API.
    pub fn free(&self) -> FreeApi {
        FreeApi {
            http: self.http.clone(),
        }
    }
}
