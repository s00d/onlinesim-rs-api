//! A fake OnlineSim API for your own tests: no network, key, money or clock.
//!
//! [`MockOnlineSim`] answers every endpoint the SDK calls like the real backend (HTTP 456 for a wrong key,
//! HTTP 200 `ERROR_WRONG_KEY` for a missing one,
//! string/int `response`, decimal-string money, `[]` for empty maps) and keeps a balance, operations and
//! rents. It is a [`Transport`](crate::Transport) and a [`BlockingTransport`](crate::BlockingTransport)
//! ([`MockOnlineSim::transport`]), so `client()` / `client_blocking()` give normal clients that never open
//! a socket (feature `mock-transport`). With feature `mock` it can also serve the same state over local
//! HTTP ([`MockOnlineSim::start`]) for code that builds its own HTTP client.
//!
//! ```
//! # #[cfg(feature = "async")]
//! # async fn demo() -> onlinesim_rs_api::Result<()> {
//! use std::time::Duration;
//! use onlinesim_rs_api::WaitCodeOptions;
//! use onlinesim_rs_api::mock::{MockOnlineSim, MockOptions};
//!
//! let mock = MockOnlineSim::new(MockOptions { balance: 10.0, ..MockOptions::default() });
//! let client = mock.client()?;
//!
//! let tzid = client.numbers().get("google", 44, &[]).await?;
//! mock.add_sms_after(tzid, "Your code: 123-456", 2); // arrives on the third poll
//! let options = WaitCodeOptions { interval: Duration::ZERO, ..WaitCodeOptions::default() };
//! assert_eq!(client.numbers().wait_code(tzid, options).await?, "123456");
//! assert_eq!(mock.balance(), 9.8);
//! # Ok(())
//! # }
//! ```
//!
//! Deterministic: ids and numbers come from a counter (`+44900000001`, tzid 1, …), timestamps from a fixed
//! epoch. Rents are reserved by getRentNum and charged by their first SMS that is not `notify`. Not imitated:
//! anything that needs real time (the 5 s lock after `setOperationOk`, the 10 s lock of `extendRentState`,
//! the 120 s before an unconfirmed rent may be closed, its 20-minute wait for the first SMS);
//! `guard_interval > 0` stands in for "too early to close".

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::transport::{HttpRequest, HttpResponse};

mod handlers;
#[cfg(feature = "mock")]
mod server;
mod state;

use state::{Failure, FailureKind, MockState, from_mills, to_mills};
pub use state::{
    MockOperation, MockOptions, MockPrices, MockRent, MockRentSms, MockRentTariff, MockRequest,
    MockSms, SmsScript, extract_code,
};

/// Base URL of clients made by [`MockOnlineSim::client`] on the in-process transport. Never contacted.
pub const MOCK_BASE_URL: &str = "https://mock.onlinesim.invalid/api/";

/// The one lock around the state. Everything goes through [`Shared::with`].
#[derive(Clone)]
struct Shared(Arc<Mutex<MockState>>);

impl Shared {
    fn with<T>(&self, f: impl FnOnce(&mut MockState) -> T) -> T {
        // A panic inside a caller cannot poison the state for the next requests.
        let mut guard: MutexGuard<'_, MockState> = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }

    /// Answer one request: log → queued failure → unknown path 404 → wrong key 456 / missing key 200 →
    /// handler.
    fn dispatch(&self, request: MockRequest, authorization: Option<&str>) -> HttpResponse {
        self.with(|state| {
            state.requests.push(request.clone());
            if let Some(failure) = state.take_failure(&request.path) {
                return match failure {
                    FailureKind::Code(code) => reply(200, &json!({ "response": code })),
                    FailureKind::Status(status) => html(status, "Mock transport failure"),
                };
            }
            let Some(handler) = handlers::route(&request.path) else {
                return html(404, "Not Found");
            };
            // A wrong key is HTTP 456 (public endpoints check a sent key too); no key at all is HTTP 200.
            let expected = format!("Bearer {}", state.options.api_key);
            match authorization {
                Some(auth) if auth != expected => {
                    return reply(456, &json!({ "response": "ERROR_WRONG_KEY" }));
                }
                None if !handlers::PUBLIC_PATHS.contains(&request.path.as_str()) => {
                    return reply(200, &json!({ "response": "ERROR_WRONG_KEY" }));
                }
                _ => {}
            }
            reply(200, &handler(state, &request))
        })
    }
}

fn reply(status: u16, body: &Value) -> HttpResponse {
    HttpResponse {
        status,
        body: body.to_string(),
    }
}

fn html(status: u16, text: &str) -> HttpResponse {
    HttpResponse {
        status,
        body: format!("<html><body>{status} {text}</body></html>"),
    }
}

/// Path relative to `/api/` (the whole path when there is no `/api/`).
fn api_path(path: &str) -> &str {
    match path.find("/api/") {
        Some(at) => &path[at + 5..],
        None => path.trim_start_matches('/'),
    }
}

/// The in-process transport of a [`MockOnlineSim`]: a [`Transport`](crate::Transport) and a
/// [`BlockingTransport`](crate::BlockingTransport) that answer from the mock's state. Cheap to clone;
/// clones share the state. Any base URL works.
#[derive(Clone)]
pub struct MockTransport(Shared);

impl fmt::Debug for MockTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MockTransport").finish_non_exhaustive()
    }
}

impl MockTransport {
    fn answer(&self, request: HttpRequest) -> Result<HttpResponse> {
        let url = reqwest::Url::parse(&request.url).map_err(|e| {
            Error::transport(format!("Mock: bad URL {:?}: {e}", request.url), None, "")
        })?;
        let authorization = request
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
            .map(|(_, value)| value.as_str());
        let mock_request = MockRequest::new(
            request.method,
            api_path(url.path()),
            url.query_pairs().into_owned().collect(),
            request.body.as_deref(),
        );
        Ok(self.0.dispatch(mock_request, authorization))
    }
}

#[cfg(feature = "async")]
impl crate::Transport for MockTransport {
    fn send(&self, request: HttpRequest) -> crate::BoxFuture<'_, Result<HttpResponse>> {
        let response = self.answer(request);
        Box::pin(std::future::ready(response))
    }
}

#[cfg(feature = "blocking")]
impl crate::BlockingTransport for MockTransport {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse> {
        self.answer(request)
    }
}

/// The fake OnlineSim API. See the [module docs](self).
///
/// Every helper takes `&self`; the state is shared with the clients and transports made from it.
pub struct MockOnlineSim {
    shared: Shared,
    #[cfg(feature = "mock")]
    server: Option<server::Server>,
}

impl fmt::Debug for MockOnlineSim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MockOnlineSim")
            .field("base_url", &self.base_url())
            .finish_non_exhaustive()
    }
}

impl Default for MockOnlineSim {
    fn default() -> Self {
        Self::new(MockOptions::default())
    }
}

impl MockOnlineSim {
    /// An in-process mock (no server, no sockets) with these options.
    pub fn new(options: MockOptions) -> Self {
        Self {
            shared: Shared(Arc::new(Mutex::new(MockState::new(options)))),
            #[cfg(feature = "mock")]
            server: None,
        }
    }

    /// Start a local HTTP server on a free port answering from a mock with the default options
    /// (feature `mock`). Dropping the mock stops the server.
    #[cfg(feature = "mock")]
    #[cfg_attr(docsrs, doc(cfg(feature = "mock")))]
    pub async fn start() -> Self {
        Self::start_with(MockOptions::default()).await
    }

    /// [`start`](Self::start) with these options (feature `mock`).
    #[cfg(feature = "mock")]
    #[cfg_attr(docsrs, doc(cfg(feature = "mock")))]
    pub async fn start_with(options: MockOptions) -> Self {
        let mut mock = Self::new(options);
        mock.server = Some(server::Server::start(mock.shared.clone()).await);
        mock
    }

    /// API base of the mock: the server's `http://127.0.0.1:<port>/api/` after [`start`](Self::start),
    /// else [`MOCK_BASE_URL`] (never contacted).
    pub fn base_url(&self) -> String {
        #[cfg(feature = "mock")]
        if let Some(server) = &self.server {
            return server.base_url();
        }
        MOCK_BASE_URL.to_string()
    }

    /// The in-process transport, for clients you build yourself:
    /// `Client::builder().api_key("mock-key").build_with(mock.transport())`.
    pub fn transport(&self) -> MockTransport {
        MockTransport(self.shared.clone())
    }

    /// A [`ClientBuilder`](crate::ClientBuilder) with the mock's key and [`base_url`](Self::base_url).
    pub fn builder(&self) -> crate::ClientBuilder {
        let key = self.shared.with(|s| s.options.api_key.clone());
        crate::ClientBuilder::new()
            .api_key(key)
            .base_url(self.base_url())
    }

    /// An async client wired to the mock with its key: in-process, or over HTTP after
    /// [`start`](Self::start). To test a wrong key, build one yourself with
    /// `Client::builder().api_key("wrong").build_with(mock.transport())`.
    #[cfg(feature = "async")]
    pub fn client(&self) -> Result<crate::Client> {
        #[cfg(feature = "mock")]
        if self.server.is_some() {
            return self.builder().build();
        }
        self.builder().build_with(self.transport())
    }

    /// A blocking client wired to the mock with its key (feature `blocking`): in-process, or over HTTP
    /// after [`start`](Self::start) (then create it outside the async runtime, e.g. in `spawn_blocking`).
    #[cfg(feature = "blocking")]
    #[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]
    pub fn client_blocking(&self) -> Result<crate::blocking::Client> {
        #[cfg(feature = "mock")]
        if self.server.is_some() {
            return self.builder().build_blocking();
        }
        self.builder().build_blocking_with(self.transport())
    }

    /// An SMS arrives now at number operation or rent `tzid` (detected by the id). `service` defaults to
    /// the operation's service (`other` for rents). The first rent SMS whose service is not `notify`
    /// confirms the rent and charges its price; a `notify` SMS gets `code` `0` and confirms nothing.
    ///
    /// # Panics
    ///
    /// When there is no open operation or rent `tzid`: a test bug.
    pub fn add_sms(&self, tzid: i64, text: &str) {
        self.add_sms_from(tzid, text, None);
    }

    /// [`add_sms`](Self::add_sms) with a sender / service.
    ///
    /// # Panics
    ///
    /// When there is no open operation or rent `tzid`.
    pub fn add_sms_from(&self, tzid: i64, text: &str, service: Option<&str>) {
        let delivered = self.shared.with(|s| s.deliver(tzid, text, service));
        assert!(delivered, "MockOnlineSim: no open operation or rent {tzid}");
    }

    /// The SMS arrives after `polls` state requests for `tzid` (getState / getRentState, listing it alone or
    /// with others): with `polls = 2` the third poll sees it. Lets `wait_code` run with a zero interval.
    ///
    /// # Panics
    ///
    /// When there is no operation or rent `tzid`.
    pub fn add_sms_after(&self, tzid: i64, text: &str, polls: u32) {
        self.add_sms_after_from(tzid, text, polls, None);
    }

    /// [`add_sms_after`](Self::add_sms_after) with a sender / service.
    ///
    /// # Panics
    ///
    /// When there is no operation or rent `tzid`.
    pub fn add_sms_after_from(&self, tzid: i64, text: &str, polls: u32, service: Option<&str>) {
        let known = self.shared.with(|s| {
            let known = s.operations.contains_key(&tzid) || s.rents.contains_key(&tzid);
            if known {
                s.schedule(tzid, text, polls, service);
            }
            known
        });
        assert!(known, "MockOnlineSim: no operation or rent {tzid}");
    }

    /// The next request answers `{"response": code}` (`NO_NUMBER`, `TRY_AGAIN_LATER`, …). Failures queue
    /// up; each is used once.
    pub fn fail_next(&self, code: impl Into<String>) {
        self.push_failure(None, FailureKind::Code(code.into()));
    }

    /// The next request to `endpoint` (relative to `/api/`, e.g. `getNum.php`) answers
    /// `{"response": code}`.
    pub fn fail_next_on(&self, endpoint: impl Into<String>, code: impl Into<String>) {
        self.push_failure(Some(endpoint.into()), FailureKind::Code(code.into()));
    }

    /// The next request fails at transport level: a non-JSON body with HTTP `status` (502 in the other
    /// SDKs' default), which the client reports as [`Error::Transport`].
    pub fn fail_next_transport(&self, status: u16) {
        self.push_failure(None, FailureKind::Status(status));
    }

    /// [`fail_next_transport`](Self::fail_next_transport) for the next request to `endpoint` only.
    pub fn fail_next_transport_on(&self, endpoint: impl Into<String>, status: u16) {
        self.push_failure(Some(endpoint.into()), FailureKind::Status(status));
    }

    fn push_failure(&self, endpoint: Option<String>, kind: FailureKind) {
        self.shared
            .with(|s| s.failures.push(Failure { endpoint, kind }));
    }

    /// Set the total balance and an extra frozen part, USD (`getBalance` reports `balance` and, as
    /// `zbalance`, `zbalance` plus the prices frozen by open operations and unconfirmed rents). Usually `set_balance(10.0, 0.0)`.
    pub fn set_balance(&self, balance: f64, zbalance: f64) {
        self.shared.with(|s| {
            s.balance = to_mills(balance);
            s.extra_frozen = to_mills(zbalance);
        });
    }

    /// Total balance, USD, including the part frozen by open operations and unconfirmed rents (as
    /// `getBalance` reports it).
    pub fn balance(&self) -> f64 {
        self.shared.with(|s| from_mills(s.balance))
    }

    /// Number operations, oldest first (copies).
    pub fn operations(&self) -> Vec<MockOperation> {
        self.shared
            .with(|s| s.operations.values().cloned().collect())
    }

    /// Open rents, oldest first (copies).
    pub fn rents(&self) -> Vec<MockRent> {
        self.shared.with(|s| s.rents.values().cloned().collect())
    }

    /// Every request received, in order (copies).
    pub fn requests(&self) -> Vec<MockRequest> {
        self.shared.with(|s| s.requests.clone())
    }

    /// Webhook URL as last set through `profile.php`.
    pub fn webhook_url(&self) -> Option<String> {
        self.shared.with(|s| s.webhook_url.clone())
    }

    /// Back to the constructor's state: balance, no operations, rents, webhook, logs, schedules or
    /// failures. A started server keeps running.
    pub fn reset(&self) {
        self.shared.with(MockState::reset);
    }

    /// 0.2.0 API: queue a scripted number (see [`SmsScript`]). The order must still be priced
    /// ([`MockOptions::prices`]); prefer [`add_sms_after`](Self::add_sms_after).
    pub fn script_sms(&self, script: SmsScript) {
        self.shared.with(|s| s.scripts.push(script));
    }

    /// 0.2.0 API: answer `NO_NUMBER` whenever `service` is ordered. Prefer
    /// `fail_next_on("getNum.php", "NO_NUMBER")` or a `reject` prefix.
    pub fn fail_no_number(&self, service: impl Into<String>) {
        self.shared.with(|s| s.no_number_for.push(service.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_paths() {
        assert_eq!(api_path("/api/getNum.php"), "getNum.php");
        assert_eq!(
            api_path("/x/api/rent/getRentNum.php"),
            "rent/getRentNum.php"
        );
        assert_eq!(api_path("/getFreeList"), "getFreeList");
    }
}
