//! Client builder shared by async and blocking clients.

use crate::config::{Config, DEFAULT_BASE_URL};
use crate::error::Result;

/// Builder for async and blocking clients.
#[derive(Debug, Clone, Default)]
pub struct ClientBuilder {
    config: Config,
}

impl ClientBuilder {
    /// Create a new builder with defaults (`lang = "en"`, OnlineSim base URL).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set API key.
    pub fn apikey(mut self, apikey: impl Into<String>) -> Self {
        self.config.apikey = Some(apikey.into());
        self
    }

    /// Set language (`lang` parameter).
    pub fn lang(mut self, lang: impl Into<String>) -> Self {
        self.config.lang = lang.into();
        self
    }

    /// Set developer id.
    pub fn dev_id(mut self, dev_id: i64) -> Self {
        self.config.dev_id = Some(dev_id);
        self
    }

    /// Set OAuth bearer token for OnlineSim endpoints.
    pub fn oauth(mut self, oauth: impl Into<String>) -> Self {
        self.config.oauth = Some(oauth.into());
        self
    }

    /// Override OnlineSim base URL (default `https://onlinesim.host/api/`).
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.config.base_url = base_url.into();
        self
    }

    /// Build an async [`crate::Client`].
    #[cfg(feature = "async")]
    pub fn build(self) -> Result<crate::Client> {
        crate::Client::from_config(self.finish_config())
    }

    /// Build a blocking [`crate::blocking::Client`].
    #[cfg(feature = "blocking")]
    pub fn build_blocking(self) -> Result<crate::blocking::Client> {
        crate::blocking::Client::from_config(self.finish_config())
    }

    fn finish_config(self) -> Config {
        let mut config = self.config;
        if config.base_url.is_empty() {
            config.base_url = DEFAULT_BASE_URL.to_string();
        }
        config
    }
}
