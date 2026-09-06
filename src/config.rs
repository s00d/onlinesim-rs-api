//! Shared client configuration.

/// Default OnlineSim API base URL.
pub const DEFAULT_BASE_URL: &str = "https://onlinesim.host/api/";

/// Default country dial code used by API helpers.
pub const DEFAULT_COUNTRY: i64 = 1;

/// Browser-like User-Agent used by the JS client.
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/84.0.4147.89 Safari/537.36";

/// Configuration shared by HTTP transports and API modules.
#[derive(Debug, Clone)]
pub struct Config {
    /// API key (`apikey` query/body parameter).
    pub apikey: Option<String>,
    /// Language code (`lang`), default `"en"`.
    pub lang: String,
    /// Optional developer id (`dev_id`).
    pub dev_id: Option<i64>,
    /// Optional OAuth bearer token for OnlineSim endpoints.
    pub oauth: Option<String>,
    /// OnlineSim API base URL.
    pub base_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            apikey: None,
            lang: "en".to_string(),
            dev_id: None,
            oauth: None,
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }
}
