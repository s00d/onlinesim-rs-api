//! Blocking HTTP transport.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::config::{Config, USER_AGENT as UA};
use crate::error::{Error, Result};
use crate::util::{from_api_value, parse_api_value, to_query_pairs, with_auth_params};

/// Shared blocking HTTP client.
#[derive(Debug, Clone)]
pub struct BlockingHttp {
    client: reqwest::blocking::Client,
    pub(crate) config: Config,
}

impl BlockingHttp {
    /// Build a blocking HTTP client from config.
    pub fn new(config: Config) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(UA)
            .build()?;
        Ok(Self { client, config })
    }

    fn headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(UA));
        if let Some(token) = self.config.oauth.as_deref() {
            let value = format!("Bearer {token}");
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&value).map_err(|e| Error::Unexpected(e.to_string()))?,
            );
        }
        Ok(headers)
    }

    fn join_url(base: &str, path: &str, php_suffix: bool) -> String {
        let base = base.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        if php_suffix {
            format!("{base}/{path}.php")
        } else {
            format!("{base}/{path}")
        }
    }

    /// GET against OnlineSim.
    pub fn get_onlinesim<T, P>(&self, path: &str, params: P, php_suffix: bool) -> Result<T>
    where
        T: DeserializeOwned,
        P: Serialize,
    {
        let url = Self::join_url(&self.config.base_url, path, php_suffix);
        let params = with_auth_params(
            serde_json::to_value(params)?,
            self.config.apikey.as_deref(),
            &self.config.lang,
            self.config.dev_id,
        );
        let query = to_query_pairs(params)?;
        let response = self
            .client
            .get(&url)
            .headers(self.headers()?)
            .query(&query)
            .send()?
            .error_for_status()?;
        let value: Value = response.json()?;
        from_api_value(parse_api_value(value)?)
    }

    /// POST JSON against OnlineSim.
    pub fn post_onlinesim<T, B>(&self, path: &str, body: B, php_suffix: bool) -> Result<T>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        let url = Self::join_url(&self.config.base_url, path, php_suffix);
        let body = with_auth_params(
            serde_json::to_value(body)?,
            self.config.apikey.as_deref(),
            &self.config.lang,
            self.config.dev_id,
        );
        let response = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(&body)
            .send()?
            .error_for_status()?;
        let value: Value = response.json()?;
        from_api_value(parse_api_value(value)?)
    }
}
