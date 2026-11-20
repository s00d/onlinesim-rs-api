//! `numbers().wait_code`: poll one operation until an SMS code arrives.
//!
//! The decision logic is the pure [`Poller`]; the async and blocking loops only fetch the state, sleep and
//! close. Time is injectable: the async loop runs on tokio's clock (pause it in tests), the blocking loop
//! on a [`Clock`].

use std::time::{Duration, Instant};

use crate::error::{ApiError, Error, Result};
use crate::numbers::{Numbers, StateOneOptions};
use crate::types::{Operation, OperationStatus};

/// Error codes that mean "try again": the rate limit (HTTP 429), a transient server error, too many requests.
const RETRYABLE: &[&str] = &[
    "TRY_AGAIN_LATER",
    "ERROR_UNDEFINED",
    "INTERVAL_CONCURRENT_REQUESTS_ERROR",
];

/// Options of `numbers().wait_code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitCodeOptions {
    /// Poll interval. Default 5 s.
    pub interval: Duration,
    /// Give up after this long. Default 600 s.
    pub timeout: Duration,
    /// Call `next(tzid)` instead of `close(tzid)` after the code, to wait for another SMS. Default `false`.
    pub keep_open: bool,
    /// Return the full SMS text instead of the code. Default `false`.
    pub full_message: bool,
    /// Ignore this value (the previous code when waiting for the next one).
    pub since: Option<String>,
}

impl Default for WaitCodeOptions {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5),
            timeout: Duration::from_secs(600),
            keep_open: false,
            full_message: false,
            since: None,
        }
    }
}

/// Time source of the blocking `wait_code`.
pub trait Clock {
    /// Current instant.
    fn now(&self) -> Instant;
    /// Block for `duration`.
    fn sleep(&self, duration: Duration);
}

/// The real clock: [`Instant::now`] and [`std::thread::sleep`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// What the loop does after a poll.
#[derive(Debug)]
enum Next {
    /// A new code: close (or `next`) the operation, then return it.
    Code(String),
    /// Nothing yet: sleep this long and poll again.
    Sleep(Duration),
    /// Stop with this error.
    Fail(Error),
}

/// Pure wait_code decisions.
struct Poller<'a> {
    tzid: i64,
    options: &'a WaitCodeOptions,
}

impl<'a> Poller<'a> {
    fn new(tzid: i64, options: &'a WaitCodeOptions) -> Self {
        Self { tzid, options }
    }

    fn state_options(&self) -> StateOneOptions {
        StateOneOptions {
            message_to_code: !self.options.full_message,
            msg_list: false,
        }
    }

    /// Decide after a poll that returned `polled`, `elapsed` after the start.
    fn next(&self, polled: Result<Operation>, elapsed: Duration) -> Next {
        match polled {
            Ok(op) => {
                let code = op.msg.as_ref().and_then(|m| m.as_text()).filter(|code| {
                    !code.is_empty() && Some(*code) != self.options.since.as_deref()
                });
                if let Some(code) = code {
                    return Next::Code(code.to_string());
                }
                if let Some(status @ (OperationStatus::TzOverOk | OperationStatus::ErrorNoItems)) =
                    op.response
                {
                    return Next::Fail(ApiError::new(status.as_str(), None).into());
                }
            }
            // Transient: retried until the timeout.
            Err(Error::Transport { .. }) => {}
            Err(Error::Api(e)) if RETRYABLE.contains(&e.code.as_str()) => {}
            Err(e) => return Next::Fail(e),
        }
        match self.options.timeout.checked_sub(elapsed) {
            Some(left) if !left.is_zero() => Next::Sleep(self.options.interval.min(left)),
            _ => Next::Fail(Error::Timeout(format!(
                "No code for operation {} within {:?}",
                self.tzid, self.options.timeout
            ))),
        }
    }
}

#[cfg(feature = "async")]
impl Numbers<'_, crate::Client> {
    /// Poll the operation every `interval` until a code different from `since` arrives, then `close` it
    /// (`next` with `keep_open`) and return the code.
    ///
    /// A finished operation (`TZ_OVER_OK`, `ERROR_NO_ITEMS`) without a new code is an `ApiError` with that
    /// state as the code; running out of `timeout` is [`Error::Timeout`]. [`Error::Transport`] and the
    /// transient `ApiError` codes `TRY_AGAIN_LATER` (HTTP 429), `ERROR_UNDEFINED` and
    /// `INTERVAL_CONCURRENT_REQUESTS_ERROR` are retried until the timeout; any other `ApiError` is returned
    /// at once. Sleeps on tokio's clock.
    pub async fn wait_code(&self, tzid: i64, options: WaitCodeOptions) -> Result<String> {
        let poller = Poller::new(tzid, &options);
        let start = tokio::time::Instant::now();
        loop {
            let polled = self.state_one(tzid, poller.state_options()).await;
            match poller.next(polled, start.elapsed()) {
                Next::Code(code) => {
                    if options.keep_open {
                        self.next(tzid).await?;
                    } else {
                        self.close(tzid).await?;
                    }
                    return Ok(code);
                }
                Next::Sleep(duration) => tokio::time::sleep(duration).await,
                Next::Fail(error) => return Err(error),
            }
        }
    }
}

#[cfg(feature = "blocking")]
impl Numbers<'_, crate::blocking::Client> {
    /// Blocking `wait_code` on the real clock; see [`wait_code_with`](Self::wait_code_with).
    pub fn wait_code(&self, tzid: i64, options: WaitCodeOptions) -> Result<String> {
        self.wait_code_with(tzid, options, &SystemClock)
    }

    /// Poll the operation every `interval` until a code different from `since` arrives, then `close` it
    /// (`next` with `keep_open`) and return the code. Time comes from `clock` (a fake one in tests).
    ///
    /// A finished operation (`TZ_OVER_OK`, `ERROR_NO_ITEMS`) without a new code is an `ApiError` with that
    /// state as the code; running out of `timeout` is [`Error::Timeout`]. [`Error::Transport`] and the
    /// transient `ApiError` codes `TRY_AGAIN_LATER` (HTTP 429), `ERROR_UNDEFINED` and
    /// `INTERVAL_CONCURRENT_REQUESTS_ERROR` are retried until the timeout; any other `ApiError` is returned
    /// at once.
    pub fn wait_code_with(
        &self,
        tzid: i64,
        options: WaitCodeOptions,
        clock: &impl Clock,
    ) -> Result<String> {
        let poller = Poller::new(tzid, &options);
        let start = clock.now();
        loop {
            let polled = self.state_one(tzid, poller.state_options());
            match poller.next(polled, clock.now().saturating_duration_since(start)) {
                Next::Code(code) => {
                    if options.keep_open {
                        self.next(tzid)?;
                    } else {
                        self.close(tzid)?;
                    }
                    return Ok(code);
                }
                Next::Sleep(duration) => clock.sleep(duration),
                Next::Fail(error) => return Err(error),
            }
        }
    }
}
