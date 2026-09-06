//! Blocking (synchronous) client API.

mod free;
pub(crate) mod http;
mod numbers;
mod rent;
mod user;

pub use free::FreeApi;
pub use numbers::NumbersApi;
pub use rent::RentApi;
pub use user::UserApi;

use std::sync::Arc;

use crate::config::Config;
use crate::error::Result;
use crate::wait_code::blocking_hub::BlockingWaitHub;

use self::http::BlockingHttp;

/// Blocking OnlineSim client.
#[derive(Debug, Clone)]
pub struct Client {
    http: BlockingHttp,
    wait_hub: Arc<BlockingWaitHub>,
}

impl Client {
    /// Start a builder.
    pub fn builder() -> crate::ClientBuilder {
        crate::ClientBuilder::new()
    }

    /// Convenience: client with only an API key.
    pub fn new(apikey: impl Into<String>) -> Result<Self> {
        crate::ClientBuilder::new().apikey(apikey).build_blocking()
    }

    pub(crate) fn from_config(config: Config) -> Result<Self> {
        Ok(Self {
            http: BlockingHttp::new(config)?,
            wait_hub: Arc::new(BlockingWaitHub::default()),
        })
    }

    /// Temporary SMS numbers API.
    pub fn numbers(&self) -> NumbersApi {
        NumbersApi {
            http: self.http.clone(),
            wait_hub: self.wait_hub.clone(),
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
