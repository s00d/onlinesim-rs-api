//! Async HTTP transport for OnlineSim.

use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::config::{Config, USER_AGENT as UA};
use crate::error::{Error, Result};
use crate::rate_limit::{RateLimiter, MAX_RETRIES, OP_OK_INTERVAL};
use crate::util::{from_api_value, parse_api_value, to_query_pairs, with_auth_params};

/// Shared async HTTP client.
#[derive(Debug, Clone)]
pub struct Http {
    client: reqwest::Client,
    pub(crate) config: Config,
    limiter: Arc<RateLimiter>,
}

impl Http {
    /// Build an HTTP client from config.
    pub fn new(config: Config) -> Result<Self> {
        let client = reqwest::Client::builder().user_agent(UA).build()?;
        let limiter = if config.rate_limit {
            Arc::new(RateLimiter::new())
        } else {
            Arc::new(RateLimiter::disabled())
        };
        Ok(Self {
            client,
            config,
            limiter,
        })
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

    async fn send_get(&self, path: &str, params: Value, php_suffix: bool) -> Result<Value> {
        self.limiter.wait_async(path).await?;
        let url = Self::join_url(&self.config.base_url, path, php_suffix);
        let params = with_auth_params(
            params,
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
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }

    async fn send_post(&self, path: &str, body: Value, php_suffix: bool) -> Result<Value> {
        self.limiter.wait_async(path).await?;
        let url = Self::join_url(&self.config.base_url, path, php_suffix);
        let body = with_auth_params(
            body,
            self.config.apikey.as_deref(),
            &self.config.lang,
            self.config.dev_id,
        );
        let response = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }

    /// GET against the OnlineSim base URL (with rate limit + INTERVAL retry).
    pub async fn get_onlinesim<T, P>(&self, path: &str, params: P, php_suffix: bool) -> Result<T>
    where
        T: DeserializeOwned,
        P: Serialize,
    {
        let params = serde_json::to_value(params)?;
        let mut last_err = None;
        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                tokio::time::sleep(OP_OK_INTERVAL).await;
            }
            let value = self.send_get(path, params.clone(), php_suffix).await?;
            match parse_api_value(value) {
                Ok(v) => return from_api_value(v),
                Err(e) if e.is_temporary() && attempt < MAX_RETRIES => {
                    last_err = Some(e);
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.unwrap_or_else(|| Error::Unexpected("retry exhausted".into())))
    }

    /// POST JSON against the OnlineSim base URL.
    pub async fn post_onlinesim<T, B>(&self, path: &str, body: B, php_suffix: bool) -> Result<T>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        let body = serde_json::to_value(body)?;
        let mut last_err = None;
        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            let value = self.send_post(path, body.clone(), php_suffix).await?;
            match parse_api_value(value) {
                Ok(v) => return from_api_value(v),
                Err(e) if e.is_temporary() && attempt < MAX_RETRIES => {
                    last_err = Some(e);
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.unwrap_or_else(|| Error::Unexpected("retry exhausted".into())))
    }
}
