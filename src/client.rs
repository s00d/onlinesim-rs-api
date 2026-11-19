//! Client settings and the async client.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::wire::{Core, DEFAULT_BASE_URL, DEFAULT_TIMEOUT};

/// Settings of [`Client`](crate::Client) and `blocking::Client`.
///
/// ```no_run
/// # #[cfg(feature = "async")]
/// # fn demo() -> onlinesim_rs_api::Result<()> {
/// use std::time::Duration;
///
/// let client = onlinesim_rs_api::Client::builder()
///     .api_key("your-api-key")
///     .lang("en")
///     .timeout(Duration::from_secs(15))
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    api_key: Option<String>,
    base_url: String,
    lang: String,
    dev_id: Option<String>,
    timeout: Duration,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: DEFAULT_BASE_URL.to_string(),
            lang: "en".to_string(),
            dev_id: None,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

impl ClientBuilder {
    /// Defaults: no key, `https://onlinesim.host/api/`, `lang=en`, 30 s timeout.
    pub fn new() -> Self {
        Self::default()
    }

    /// API key, sent as `Authorization: Bearer <key>`, never in the URL. Not needed for tariffs and free
    /// numbers. An empty key means none.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into()).filter(|k| !k.is_empty());
        self
    }

    /// Full API base; trailing slash optional. Default `https://onlinesim.host/api/`.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Language of server texts (`lang` query parameter). Default `en`.
    pub fn lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = lang.into();
        self
    }

    /// Partner developer id (`dev_id` query parameter), sent only when set.
    pub fn dev_id(mut self, dev_id: impl fmt::Display) -> Self {
        self.dev_id = Some(dev_id.to_string()).filter(|d| !d.is_empty());
        self
    }

    /// Per-request timeout. Default 30 s. Applied by the transport to every request, also to an injected
    /// `reqwest` client.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn core(self) -> Result<Core> {
        let base = if self.base_url.ends_with('/') {
            self.base_url
        } else {
            format!("{}/", self.base_url)
        };
        let base = reqwest::Url::parse(&base)
            .map_err(|e| Error::Config(format!("base URL {base:?}: {e}")))?;
        Ok(Core {
            api_key: self.api_key,
            base,
            lang: self.lang,
            dev_id: self.dev_id,
            timeout: self.timeout,
        })
    }

    /// Build an async [`Client`] on a default [`reqwest::Client`].
    #[cfg(feature = "async")]
    pub fn build(self) -> Result<Client> {
        let http = reqwest::Client::builder()
            .build()
            .map_err(|e| Error::Config(e.to_string()))?;
        self.build_with(http)
    }

    /// Build an async [`Client`] on your transport: a configured [`reqwest::Client`] (proxy, TLS, …) or
    /// any [`Transport`](crate::Transport) (a fake in tests).
    #[cfg(feature = "async")]
    pub fn build_with(self, transport: impl crate::Transport) -> Result<Client> {
        Ok(Client {
            core: Arc::new(self.core()?),
            transport: Arc::new(transport),
        })
    }

    /// Build a [`blocking::Client`](crate::blocking::Client) on a default `reqwest::blocking::Client`.
    #[cfg(feature = "blocking")]
    pub fn build_blocking(self) -> Result<crate::blocking::Client> {
        let http = reqwest::blocking::Client::builder()
            .build()
            .map_err(|e| Error::Config(e.to_string()))?;
        self.build_blocking_with(http)
    }

    /// Build a [`blocking::Client`](crate::blocking::Client) on your transport: a configured
    /// `reqwest::blocking::Client` or any [`BlockingTransport`](crate::BlockingTransport).
    #[cfg(feature = "blocking")]
    pub fn build_blocking_with(
        self,
        transport: impl crate::BlockingTransport,
    ) -> Result<crate::blocking::Client> {
        Ok(crate::blocking::Client::new_inner(
            self.core()?,
            Arc::new(transport),
        ))
    }
}

/// Async OnlineSim client. Cheap to clone; clones share the connection pool.
///
/// ```no_run
/// # #[cfg(feature = "async")]
/// # async fn demo() -> onlinesim_rs_api::Result<()> {
/// let client = onlinesim_rs_api::Client::new("your-api-key")?;
/// let balance = client.user().balance().await?;
/// println!("available: {} USD", balance.balance - balance.zbalance);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async")]
#[derive(Clone)]
pub struct Client {
    core: Arc<Core>,
    transport: Arc<dyn crate::Transport>,
}

#[cfg(feature = "async")]
impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.core.base.as_str())
            .field("lang", &self.core.lang)
            .field("timeout", &self.core.timeout)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "async")]
impl Client {
    /// Client with an API key and the defaults.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        ClientBuilder::new().api_key(api_key).build()
    }

    /// Settings builder.
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

    pub(crate) async fn run<T>(&self, call: crate::wire::Call<T>) -> Result<T> {
        let request = self.core.http_request(&call.request);
        let parsed = self.transport.send(request).await;
        (call.finish)(parsed.and_then(crate::wire::parse_response))
    }
}
