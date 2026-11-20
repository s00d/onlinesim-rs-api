//! Blocking client (feature `blocking`): the same sections and methods as the async
//! [`Client`](crate::Client), without `.await`.
//!
//! Do not create or drop it inside an async runtime (a `reqwest::blocking` rule).

use std::fmt;
use std::sync::Arc;

use crate::ClientBuilder;
use crate::error::Result;
use crate::transport::BlockingTransport;
use crate::wire::{Call, Core, parse_response};

/// Blocking OnlineSim client. Cheap to clone; clones share the connection pool.
///
/// ```no_run
/// # fn demo() -> onlinesim_rs_api::Result<()> {
/// let client = onlinesim_rs_api::blocking::Client::new("your-api-key")?;
/// let balance = client.user().balance()?;
/// println!("available: {} USD", balance.balance - balance.zbalance.max(0.0));
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Client {
    core: Arc<Core>,
    transport: Arc<dyn BlockingTransport>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("blocking::Client")
            .field("base", &self.core.base.as_str())
            .field("lang", &self.core.lang)
            .field("timeout", &self.core.timeout)
            .finish_non_exhaustive()
    }
}

impl Client {
    pub(crate) fn new_inner(core: Core, transport: Arc<dyn BlockingTransport>) -> Self {
        Self {
            core: Arc::new(core),
            transport,
        }
    }

    /// Client with an API key and the defaults.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        ClientBuilder::new().api_key(api_key).build_blocking()
    }

    /// Settings builder; finish it with [`ClientBuilder::build_blocking`].
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Numbers for receiving SMS.
    pub fn numbers(&self) -> crate::Numbers<'_, Self> {
        crate::Numbers(self)
    }

    /// Rented numbers.
    pub fn rent(&self) -> crate::Rent<'_, Self> {
        crate::Rent(self)
    }

    /// Account.
    pub fn user(&self) -> crate::User<'_, Self> {
        crate::User(self)
    }

    /// Public free numbers.
    pub fn free(&self) -> crate::Free<'_, Self> {
        crate::Free(self)
    }

    pub(crate) fn run<T>(&self, call: Call<T>) -> Result<T> {
        let request = self.core.http_request(&call.request);
        (call.finish)(self.transport.send(request).and_then(parse_response))
    }
}
