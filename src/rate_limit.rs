//! Client-side pacing to avoid `INTERVAL_CONCURRENT_REQUESTS_ERROR`.
//!
//! OpenAPI: general request intensity limit; `setOperationOk` at most once per
//! 5 seconds for the same tzid (we pace all `setOperationOk` calls at ≥5s).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

/// Default global gap between any two API calls (1 rps).
pub(crate) const GLOBAL_INTERVAL: Duration = Duration::from_secs(1);

/// Minimum gap between `setOperationOk` calls (OpenAPI).
pub(crate) const OP_OK_INTERVAL: Duration = Duration::from_secs(5);

/// Extra attempts after the first failure on temporary INTERVAL errors.
pub(crate) const MAX_RETRIES: u32 = 2;

#[derive(Debug, Default)]
struct Inner {
    last_global: Option<Instant>,
    last_path: HashMap<String, Instant>,
}

/// Shared rate limiter (async + blocking).
#[derive(Debug, Default)]
pub(crate) struct RateLimiter {
    inner: Mutex<Inner>,
    /// When false, Wait is a no-op (tests / mock).
    enabled: bool,
}

impl RateLimiter {
    /// Enabled limiter with default intervals.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            enabled: true,
        }
    }

    /// Disabled limiter (no sleeps).
    pub fn disabled() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            enabled: false,
        }
    }

    /// Block until the next slot for `path` is free.
    #[cfg(feature = "blocking")]
    pub(crate) fn wait(&self, path: &str) -> Result<()> {
        self.wait_inner(path, |d| {
            if !d.is_zero() {
                std::thread::sleep(d);
            }
            Ok(())
        })
    }

    /// Async wait (uses `tokio::time::sleep` when the `async` feature is on).
    #[cfg(feature = "async")]
    pub(crate) async fn wait_async(&self, path: &str) -> Result<()> {
        let sleep_for = self.reserve(path)?;
        if !sleep_for.is_zero() {
            tokio::time::sleep(sleep_for).await;
        }
        Ok(())
    }

    fn reserve(&self, path: &str) -> Result<Duration> {
        if !self.enabled {
            return Ok(Duration::ZERO);
        }
        let mut g = self
            .inner
            .lock()
            .map_err(|_| Error::Unexpected("rate limiter poisoned".into()))?;
        let now = Instant::now();
        let mut wait_until = now;

        if let Some(last) = g.last_global {
            let t = last + GLOBAL_INTERVAL;
            if t > wait_until {
                wait_until = t;
            }
        }
        if path == "setOperationOk" {
            if let Some(last) = g.last_path.get(path) {
                let t = *last + OP_OK_INTERVAL;
                if t > wait_until {
                    wait_until = t;
                }
            }
        }

        let delay = wait_until.saturating_duration_since(now);
        g.last_global = Some(wait_until);
        if path == "setOperationOk" {
            g.last_path.insert(path.to_string(), wait_until);
        }
        Ok(delay)
    }

    #[cfg(feature = "blocking")]
    fn wait_inner<F>(&self, path: &str, sleep: F) -> Result<()>
    where
        F: FnOnce(Duration) -> Result<()>,
    {
        let delay = self.reserve(path)?;
        sleep(delay)
    }
}
