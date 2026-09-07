//! Built-in OnlineSim HTTP mocks for unit and integration tests.
//!
//! Enable with Cargo feature `mock` (implies `async`):
//!
//! ```toml
//! [dev-dependencies]
//! onlinesim-rs-api = { version = "0.1", features = ["mock"] }
//! ```
//!
//! # Why
//!
//! Ordering real numbers costs money and is flaky in CI. [`MockOnlineSim`] runs a local
//! HTTP server that speaks the OnlineSim JSON protocol so you can exercise
//! `get` → `wait_code` → `close` without hitting production.
//!
//! # Ephemeral (tests)
//!
//! ```no_run
//! # #[cfg(all(feature = "mock", feature = "async"))]
//! # async fn demo() -> onlinesim_rs_api::Result<()> {
//! use onlinesim_rs_api::mock::{MockOnlineSim, SmsScript};
//! use onlinesim_rs_api::WaitCodeOptions;
//!
//! let mock = MockOnlineSim::start().await;
//! mock.script_sms(SmsScript {
//!     service: "telegram".into(),
//!     number: "+19001234567".into(),
//!     country: 1,
//!     code: "123456".into(),
//!     polls_before_code: 1,
//!     ..SmsScript::default()
//! });
//!
//! let client = mock.client()?;
//! let ordered = client.numbers().get_with_number("telegram").await?;
//! let code = client
//!     .numbers()
//!     .wait_code(
//!         ordered.tzid,
//!         WaitCodeOptions {
//!             interval_secs: 0,
//!             max_attempts: 5,
//!             ..WaitCodeOptions::default()
//!         },
//!     )
//!     .await?;
//! assert_eq!(code, "123456");
//! # Ok(())
//! # }
//! ```
//!
//! # Persistent state (CLI)
//!
//! Opt-in file path keeps balance / ops across process restarts. Test helpers
//! (`scripts`, `no_number_for`, `balance_error`) are **not** persisted.
//!
//! ```no_run
//! # #[cfg(all(feature = "mock", feature = "async"))]
//! # async fn demo() -> onlinesim_rs_api::Result<()> {
//! use onlinesim_rs_api::mock::MockOnlineSim;
//! use std::path::PathBuf;
//!
//! let mock = MockOnlineSim::builder()
//!     .state_path(PathBuf::from("/tmp/onlinesim-mock.json"))
//!     // .state_path_from_env()  // ONLINESIM_MOCK_STATE
//!     .start()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! Interval `0` is useful in tests (no sleep). Production code should use a few seconds.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use fs4::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use crate::config::DEFAULT_COUNTRY;
use crate::error::{Error, Result};
use crate::Client;

/// Env var read by [`MockOnlineSimBuilder::state_path_from_env`].
pub const MOCK_STATE_ENV: &str = "ONLINESIM_MOCK_STATE";

/// Script describing one mocked SMS purchase + delivery.
#[derive(Debug, Clone)]
pub struct SmsScript {
    /// Service slug expected on `getNum` (matched loosely; first unused script wins).
    pub service: String,
    /// Phone number returned when `number=true`.
    pub number: String,
    /// Country dial code.
    pub country: i64,
    /// SMS code delivered after `polls_before_code` empty polls.
    pub code: String,
    /// How many `getState` calls return `TZ_NUM_WAIT` without `msg` before delivering.
    pub polls_before_code: u32,
    /// Price string for `getPrice`.
    pub price: String,
}

impl Default for SmsScript {
    fn default() -> Self {
        Self {
            service: "telegram".into(),
            number: "+19001234567".into(),
            country: DEFAULT_COUNTRY,
            code: "123456".into(),
            polls_before_code: 0,
            price: "10".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Operation {
    service: String,
    number: String,
    country: i64,
    code: String,
    polls: u32,
    polls_before_code: u32,
    closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedMockState {
    version: u32,
    balance: f64,
    zbalance: f64,
    income: f64,
    webhook_url: Option<String>,
    next_tzid: i64,
    /// String keys for JSON object compatibility.
    ops: BTreeMap<String, Operation>,
}

impl PersistedMockState {
    fn from_runtime(state: &MockState) -> Self {
        let mut ops = BTreeMap::new();
        for (id, op) in &state.ops {
            ops.insert(id.to_string(), op.clone());
        }
        Self {
            version: 1,
            balance: state.balance,
            zbalance: state.zbalance,
            income: state.income,
            webhook_url: state.webhook_url.clone(),
            next_tzid: state.next_tzid,
            ops,
        }
    }

    fn apply_to(&self, state: &mut MockState) {
        state.balance = self.balance;
        state.zbalance = self.zbalance;
        state.income = self.income;
        state.webhook_url = self.webhook_url.clone();
        state.next_tzid = self.next_tzid;
        state.ops.clear();
        for (k, op) in &self.ops {
            if let Ok(id) = k.parse::<i64>() {
                state.ops.insert(id, op.clone());
            }
        }
    }

    /// Reload persisted fields from disk; keep ephemeral setup (`scripts`, etc.).
    fn reload_into(&self, state: &mut MockState) {
        let scripts = std::mem::take(&mut state.scripts);
        let no_number_for = std::mem::take(&mut state.no_number_for);
        let balance_error = state.balance_error.take();
        self.apply_to(state);
        state.scripts = scripts;
        state.no_number_for = no_number_for;
        state.balance_error = balance_error;
    }
}

#[derive(Debug, Default)]
struct MockState {
    balance: f64,
    zbalance: f64,
    income: f64,
    /// If set, next `getBalance` returns this API error code once (not persisted).
    balance_error: Option<String>,
    webhook_url: Option<String>,
    next_tzid: i64,
    scripts: Vec<SmsScript>,
    ops: HashMap<i64, Operation>,
    no_number_for: Vec<String>,
}

impl MockState {
    fn fresh() -> Self {
        Self {
            balance: 100.0,
            zbalance: 0.0,
            income: 25.0,
            balance_error: None,
            webhook_url: None,
            next_tzid: 1000,
            scripts: Vec::new(),
            ops: HashMap::new(),
            no_number_for: Vec::new(),
        }
    }
}

fn io_err(err: impl std::fmt::Display) -> Error {
    Error::Unexpected(format!("mock state: {err}"))
}

fn open_state_file(path: &Path) -> Result<File> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(io_err)
}

fn read_persisted_locked(file: &mut File) -> Result<Option<PersistedMockState>> {
    file.seek(SeekFrom::Start(0)).map_err(io_err)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).map_err(io_err)?;
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed: PersistedMockState = serde_json::from_str(trimmed)?;
    Ok(Some(parsed))
}

fn write_persisted_locked(file: &mut File, state: &MockState) -> Result<()> {
    let persisted = PersistedMockState::from_runtime(state);
    let data = serde_json::to_vec_pretty(&persisted)?;
    file.seek(SeekFrom::Start(0)).map_err(io_err)?;
    file.set_len(0).map_err(io_err)?;
    file.write_all(&data).map_err(io_err)?;
    file.sync_all().map_err(io_err)?;
    Ok(())
}

fn with_state_file_lock<R>(path: &Path, f: impl FnOnce(&mut File) -> Result<R>) -> Result<R> {
    let mut file = open_state_file(path)?;
    // Prefer fs4 over std::fs::File::lock (Rust 1.89+) so MSRV stays 1.75.
    FileExt::lock(&file).map_err(io_err)?;
    let result = f(&mut file);
    // Unlock is best-effort; dropping the file also releases the lock.
    let _ = FileExt::unlock(&file);
    result
}

fn load_state_from_path(path: &Path) -> Result<MockState> {
    let mut state = MockState::fresh();
    with_state_file_lock(path, |file| {
        if let Some(persisted) = read_persisted_locked(file)? {
            persisted.apply_to(&mut state);
        } else {
            write_persisted_locked(file, &state)?;
        }
        Ok(())
    })?;
    Ok(state)
}

fn persist_snapshot(path: &Path, state: &Arc<Mutex<MockState>>) -> Result<()> {
    with_state_file_lock(path, |file| {
        let g = state.lock().expect("mock state");
        write_persisted_locked(file, &g)
    })
}

fn mutate_and_persist<R>(
    state: &Arc<Mutex<MockState>>,
    path: Option<&Path>,
    f: impl FnOnce(&mut MockState) -> R,
) -> R {
    match path {
        None => {
            let mut g = state.lock().expect("mock state");
            f(&mut g)
        }
        Some(path) => {
            let mut f = Some(f);
            let mut out = None;
            let persist_result = with_state_file_lock(path, |file| {
                let mut g = state.lock().expect("mock state");
                if let Some(disk) = read_persisted_locked(file)? {
                    disk.reload_into(&mut g);
                }
                out = Some(f.take().expect("mutate closure")(&mut g));
                write_persisted_locked(file, &g)
            });
            match (out, persist_result, f) {
                (Some(v), Ok(()), _) | (Some(v), Err(_), _) => v,
                // Lock/open failed before mutation — still apply in-memory.
                (None, _, Some(f)) => {
                    let mut g = state.lock().expect("mock state");
                    f(&mut g)
                }
                (None, _, None) => unreachable!("mutate closure consumed without result"),
            }
        }
    }
}

/// Builder for [`MockOnlineSim`] with optional on-disk state.
#[derive(Debug, Default, Clone)]
pub struct MockOnlineSimBuilder {
    state_path: Option<PathBuf>,
}

impl MockOnlineSimBuilder {
    /// Create a builder (ephemeral unless a state path is set).
    pub fn new() -> Self {
        Self::default()
    }

    /// Persist mock state to this JSON file (load on start, save after mutations).
    pub fn state_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_path = Some(path.into());
        self
    }

    /// If [`MOCK_STATE_ENV`] (`ONLINESIM_MOCK_STATE`) is set, use it as [`Self::state_path`].
    pub fn state_path_from_env(mut self) -> Self {
        if let Ok(path) = std::env::var(MOCK_STATE_ENV) {
            let path = path.trim();
            if !path.is_empty() {
                self.state_path = Some(PathBuf::from(path));
            }
        }
        self
    }

    /// Start the mock server, loading persisted state when a path is configured.
    pub async fn start(self) -> Result<MockOnlineSim> {
        let state = match &self.state_path {
            Some(path) => load_state_from_path(path)?,
            None => MockState::fresh(),
        };
        let server = MockServer::start().await;
        let state = Arc::new(Mutex::new(state));
        let path = self.state_path.clone();
        mount_handlers(&server, state.clone(), path.clone()).await;
        Ok(MockOnlineSim {
            server,
            state,
            state_path: path,
        })
    }
}

/// Local OnlineSim mock server.
pub struct MockOnlineSim {
    server: MockServer,
    state: Arc<Mutex<MockState>>,
    state_path: Option<PathBuf>,
}

impl Drop for MockOnlineSim {
    fn drop(&mut self) {
        if let Some(path) = &self.state_path {
            let _ = persist_snapshot(path, &self.state);
        }
    }
}

impl MockOnlineSim {
    /// Start a mock server with default balance `100` (ephemeral, in-memory only).
    pub async fn start() -> Self {
        Self::builder()
            .start()
            .await
            .expect("ephemeral mock start cannot fail")
    }

    /// Builder for optional persistent state.
    pub fn builder() -> MockOnlineSimBuilder {
        MockOnlineSimBuilder::new()
    }

    /// Delete a persisted state file. Missing file is OK.
    pub fn reset_state(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_err(e)),
        }
    }

    /// Base URL suitable for [`crate::ClientBuilder::base_url`] (ends with `/api/`).
    pub fn base_url(&self) -> String {
        format!("{}/api/", self.server.uri())
    }

    /// Build an async [`Client`] pointed at this mock.
    pub fn client(&self) -> Result<Client> {
        Client::builder()
            .apikey("mock-key")
            .base_url(self.base_url())
            .build()
    }

    /// Build a blocking [`crate::blocking::Client`] pointed at this mock.
    #[cfg(feature = "blocking")]
    pub fn client_blocking(&self) -> Result<crate::blocking::Client> {
        crate::ClientBuilder::new()
            .apikey("mock-key")
            .base_url(self.base_url())
            .build_blocking()
    }

    /// Set balance fields returned by `getBalance`.
    pub fn set_balance(&self, balance: f64, zbalance: f64, income: f64) {
        mutate_and_persist(&self.state, self.state_path.as_deref(), |g| {
            g.balance = balance;
            g.zbalance = zbalance;
            g.income = income;
            g.balance_error = None;
        });
    }

    /// Next `getBalance` call returns the given API error code (once).
    pub fn fail_balance(&self, code: impl Into<String>) {
        self.state.lock().expect("mock state").balance_error = Some(code.into());
    }

    /// Queue an SMS script. Next matching `getNum` consumes it.
    pub fn script_sms(&self, script: SmsScript) {
        self.state.lock().expect("mock state").scripts.push(script);
    }

    /// Force `getNum` for a service to return `NO_NUMBER`.
    pub fn fail_no_number(&self, service: impl Into<String>) {
        self.state
            .lock()
            .expect("mock state")
            .no_number_for
            .push(service.into());
    }

    /// Configured persist path, if any.
    pub fn state_path(&self) -> Option<&Path> {
        self.state_path.as_deref()
    }

    /// Raw wiremock URI (without `/api/`).
    pub fn uri(&self) -> String {
        self.server.uri()
    }
}

async fn mount_handlers(
    server: &MockServer,
    state: Arc<Mutex<MockState>>,
    state_path: Option<PathBuf>,
) {
    let path = state_path.clone();
    Mock::given(method("GET"))
        .and(wiremock::matchers::path_regex(r"^/api/.*"))
        .respond_with(ApiResponder {
            state: state.clone(),
            state_path: path.clone(),
        })
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(wiremock::matchers::path_regex(r"^/api/.*"))
        .respond_with(ApiResponder {
            state,
            state_path: path,
        })
        .mount(server)
        .await;
}

#[derive(Clone)]
struct ApiResponder {
    state: Arc<Mutex<MockState>>,
    state_path: Option<PathBuf>,
}

impl wiremock::Respond for ApiResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let path = request.url.path();
        let path = path.strip_prefix('/').unwrap_or(path);
        let rest = path
            .strip_prefix("api/")
            .unwrap_or(path)
            .trim_end_matches(".php");

        let query: HashMap<String, String> = request
            .url
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let persist = self.state_path.as_deref();

        let body = match rest {
            "getBalance" => handle_balance(&self.state),
            "getPrice" => handle_price(&self.state, &query),
            "getNum" | "getNumRepeat" => {
                mutate_and_persist(&self.state, persist, |g| handle_get_num_inner(g, &query))
            }
            "getState" => {
                mutate_and_persist(&self.state, persist, |g| handle_get_state_inner(g, &query))
            }
            "setOperationOk" | "rent/closeRentNum" => {
                mutate_and_persist(&self.state, persist, |g| handle_close_inner(g, &query))
            }
            "setOperationRevise" => {
                mutate_and_persist(&self.state, persist, |g| handle_next_inner(g, &query))
            }
            "getNumbersStats" => handle_tariffs(&query),
            "getProfile" => handle_profile(&self.state),
            "profile" => mutate_and_persist(&self.state, persist, |g| {
                handle_profile_save_inner(g, request)
            }),
            "webhook-logs" => handle_webhook_logs(),
            "getPaymentHistory" => handle_payment_history(),
            "getFreeCountryList" => handle_free_countries(),
            "getFreePhoneList" => handle_free_numbers(&query),
            "getFreeMessageList" => handle_free_messages(),
            "getFreeList" => handle_free_list(),
            "rent/getRentNum" => {
                mutate_and_persist(&self.state, persist, |g| handle_rent_get_inner(g, &query))
            }
            "rent/extendRentState" => mutate_and_persist(&self.state, persist, |g| {
                handle_rent_extend_inner(g, &query)
            }),
            "rent/getRentState" => handle_rent_state(&self.state, &query),
            "rent/portReload" => mutate_and_persist(&self.state, persist, |g| {
                handle_rent_port_reload_inner(g, &query)
            }),
            "rent/tariffsRent" => handle_rent_tariffs(&query),
            "getService" => json!({ "response": "1", "service": ["telegram", "whatsapp"] }),
            "getServiceNumber" => {
                json!({ "response": "1", "number": ["79001112233", "79001112234"] })
            }
            "pay/createEmpty" => json!({
                "response": "1",
                "payId": 1,
                "params": { "url": "https://example.test/pay", "sum": "100", "label": "Pay" }
            }),
            other => json!({ "response": "REQUEST_NOT_FOUND", "path": other }),
        };

        ResponseTemplate::new(200).set_body_json(body)
    }
}

fn handle_balance(state: &Arc<Mutex<MockState>>) -> Value {
    let mut g = state.lock().expect("mock state");
    if let Some(code) = g.balance_error.take() {
        return json!({ "response": code });
    }
    json!({
        "response": "1",
        "balance": g.balance,
        "zbalance": g.zbalance,
        "income": g.income,
        "income_usd": g.income,
    })
}

fn handle_price(state: &Arc<Mutex<MockState>>, query: &HashMap<String, String>) -> Value {
    let g = state.lock().expect("mock state");
    let service = query.get("service").map(String::as_str).unwrap_or("");
    let price = g
        .scripts
        .iter()
        .find(|s| s.service == service)
        .map(|s| s.price.clone())
        .unwrap_or_else(|| "10".into());
    json!({ "response": "1", "price": price })
}

fn handle_get_num_inner(g: &mut MockState, query: &HashMap<String, String>) -> Value {
    let service = query
        .get("service")
        .cloned()
        .unwrap_or_else(|| "telegram".into());

    if g.no_number_for.iter().any(|s| s == &service) {
        return json!({ "response": "NO_NUMBER" });
    }

    let script_idx = g
        .scripts
        .iter()
        .position(|s| s.service == service)
        .or(if g.scripts.is_empty() { None } else { Some(0) });

    let script = match script_idx {
        Some(i) => g.scripts.remove(i),
        None => SmsScript {
            service: service.clone(),
            ..SmsScript::default()
        },
    };

    let tzid = g.next_tzid;
    g.next_tzid += 1;
    g.ops.insert(
        tzid,
        Operation {
            service: script.service.clone(),
            number: script.number.clone(),
            country: script.country,
            code: script.code.clone(),
            polls: 0,
            polls_before_code: script.polls_before_code,
            closed: false,
        },
    );

    let with_number = matches!(
        query.get("number").map(String::as_str),
        Some("true") | Some("1")
    );

    if with_number {
        json!({
            "response": 1,
            "tzid": tzid,
            "number": script.number,
            "country": script.country,
            "time": 900,
            "service": script.service,
            "title": script.service,
            "response_text": "waiting",
            "guard_interval_remaining_seconds": 0
        })
    } else {
        json!({ "response": 1, "tzid": tzid })
    }
}

fn handle_get_state_inner(g: &mut MockState, query: &HashMap<String, String>) -> Value {
    let tzid = query.get("tzid").and_then(|s| s.parse::<i64>().ok());

    let ids: Vec<i64> = match tzid {
        Some(id) => vec![id],
        None => g
            .ops
            .iter()
            .filter(|(_, op)| op.service != "rent")
            .map(|(id, _)| *id)
            .collect(),
    };

    if ids.is_empty() {
        return json!({ "response": "ERROR_NO_OPERATIONS" });
    }

    let mut list = Vec::new();
    for id in ids {
        let Some(op) = g.ops.get_mut(&id) else {
            continue;
        };
        if op.closed || op.service == "rent" {
            continue;
        }
        op.polls += 1;
        let ready = op.polls > op.polls_before_code;
        let mut item = json!({
            "tzid": id,
            "response": if ready { "TZ_NUM_ANSWER" } else { "TZ_NUM_WAIT" },
            "number": op.number,
            "service": op.service,
            "time": 800,
            "extend": 0,
            "country": op.country,
            "sum": 10.0,
        });
        if ready {
            item.as_object_mut()
                .unwrap()
                .insert("msg".into(), Value::String(op.code.clone()));
        }
        list.push(item);
    }

    if list.is_empty() {
        return json!({ "response": "ERROR_NO_OPERATIONS" });
    }

    Value::Array(list)
}

fn handle_close_inner(g: &mut MockState, query: &HashMap<String, String>) -> Value {
    if let Some(tzid) = query.get("tzid").and_then(|s| s.parse::<i64>().ok()) {
        if let Some(op) = g.ops.get_mut(&tzid) {
            op.closed = true;
            return json!({ "response": 1, "tzid": tzid });
        }
        return json!({ "response": "ERROR_NO_TZID" });
    }
    json!({ "response": "ERROR_NO_TZID" })
}

fn handle_next_inner(g: &mut MockState, query: &HashMap<String, String>) -> Value {
    if let Some(tzid) = query.get("tzid").and_then(|s| s.parse::<i64>().ok()) {
        if g.ops.contains_key(&tzid) {
            return json!({ "response": "1", "tzid": tzid });
        }
    }
    json!({ "response": "ERROR_NO_TZID" })
}

fn handle_tariffs(query: &HashMap<String, String>) -> Value {
    let country = query
        .get("country")
        .cloned()
        .unwrap_or_else(|| DEFAULT_COUNTRY.to_string());
    let code: i64 = country.parse().unwrap_or(DEFAULT_COUNTRY);
    let one = json!({
        "name": "USA",
        "position": 1,
        "code": code,
        "new": false,
        "enabled": true,
        "locale_name": "USA",
        "services": {
            "telegram": {
                "count": 100,
                "popular": true,
                "price": 10.0,
                "id": "telegram",
                "service": "telegram",
                "slug": "telegram"
            }
        }
    });
    if country == "all" {
        let mut map = serde_json::Map::new();
        map.insert("response".into(), json!("1"));
        map.insert(DEFAULT_COUNTRY.to_string(), one);
        Value::Object(map)
    } else {
        let mut body = one;
        if let Some(obj) = body.as_object_mut() {
            obj.insert("response".into(), json!("1"));
        }
        body
    }
}

fn handle_profile(state: &Arc<Mutex<MockState>>) -> Value {
    let g = state.lock().expect("mock state");
    json!({
        "response": "1",
        "profile": {
            "id": 1,
            "name": "Mock User",
            "username": "mock",
            "email": "mock@example.test",
            "locale": "en",
            "ugroup": 1,
            "verify": 1,
            "block": 0,
            "webhook_url": g.webhook_url,
            "payment": { "payment": 100.0, "income": 25.0, "spent": 10.0, "now": 0.0 }
        }
    })
}

fn handle_profile_save_inner(g: &mut MockState, request: &Request) -> Value {
    if let Ok(value) = serde_json::from_slice::<Value>(&request.body) {
        if let Some(url) = value.pointer("/profile/webhook_url").and_then(|v| match v {
            Value::Null => Some(None),
            Value::String(s) if s.is_empty() => Some(None),
            Value::String(s) => Some(Some(s.clone())),
            _ => None,
        }) {
            g.webhook_url = url;
        }
    }
    json!({ "response": "1" })
}

fn handle_webhook_logs() -> Value {
    json!({
        "response": "1",
        "data": {
            "current_page": 1,
            "per_page": 10,
            "total": 1,
            "last_page": 1,
            "data": [
                {
                    "id": 1,
                    "type": "receiving_sms",
                    "user_id": 1,
                    "webhook_url": "https://example.test/hook",
                    "params": {
                        "user_id": 1,
                        "country_code": 1,
                        "number": "+19001234567",
                        "sender": "Telegram",
                        "message": "code 123456",
                        "time_start": "2026-01-01 00:00:00",
                        "time_left": 10,
                        "operation_id": 1000,
                        "webhook_type": "receiving_sms",
                        "code": "123456"
                    },
                    "status": "success",
                    "error": null,
                    "created_at": "2026-01-01T00:00:00Z"
                }
            ]
        }
    })
}

fn handle_payment_history() -> Value {
    json!({
        "response": "1",
        "forms": {},
        "currency": { "USD": 1.0 },
        "orders": { "current_page": 1, "data": [] },
        "paylist": {}
    })
}

fn handle_free_countries() -> Value {
    json!({
        "response": 1,
        "countries": [
            { "country": DEFAULT_COUNTRY, "country_text": "USA", "country_original": "USA" }
        ]
    })
}

fn handle_free_numbers(query: &HashMap<String, String>) -> Value {
    let country: i64 = query
        .get("country")
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_COUNTRY);
    json!({
        "response": "1",
        "numbers": [
            {
                "number": "9001234567",
                "country": country,
                "full_number": format!("+{country}9001234567"),
                "updated_at": "2026-01-01T00:00:00Z",
                "data_humans": "just now",
                "maxdate": "2026-01-02"
            }
        ]
    })
}

fn handle_free_messages() -> Value {
    json!({
        "response": 1,
        "messages": {
            "current_page": 1,
            "data": [
                {
                    "text": "Your code is 111222",
                    "in_number": "Telegram",
                    "my_number": 9001234567i64,
                    "created_at": "2026-01-01T00:00:00Z",
                    "data_humans": "just now"
                }
            ]
        }
    })
}

fn handle_free_list() -> Value {
    json!({
        "response": 1,
        "countries": [{ "country": DEFAULT_COUNTRY, "country_text": "USA" }],
        "numbers": {
            "9001234567": {
                "country": DEFAULT_COUNTRY,
                "country_original": "USA",
                "full_number": "+19001234567",
                "is_archive": false
            }
        },
        "messages": {
            "current_page": 1,
            "data": [],
            "number": "9001234567",
            "country": DEFAULT_COUNTRY
        }
    })
}

fn handle_rent_get_inner(g: &mut MockState, query: &HashMap<String, String>) -> Value {
    let country: i64 = query
        .get("country")
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_COUNTRY);
    let tzid = g.next_tzid;
    g.next_tzid += 1;
    g.ops.insert(
        tzid,
        Operation {
            service: "rent".into(),
            number: format!("+{country}9000000000"),
            country,
            code: "000000".into(),
            polls: 0,
            polls_before_code: 0,
            closed: false,
        },
    );
    json!({
        "response": 1,
        "item": {
            "tzid": tzid,
            "status": 1,
            "messages": [],
            "country": country,
            "rent": 1,
            "extension": 0,
            "sum": 50.0,
            "number": format!("+{country}9000000000"),
            "time": 86400,
            "hours": 24,
            "extend": {},
            "checked": true,
            "reload": 0,
            "day_extend": 0.0
        }
    })
}

fn handle_rent_extend_inner(g: &mut MockState, query: &HashMap<String, String>) -> Value {
    let Some(tzid) = query.get("tzid").and_then(|s| s.parse::<i64>().ok()) else {
        return json!({ "response": "ERROR_NO_TZID" });
    };
    let days: i64 = query.get("days").and_then(|s| s.parse().ok()).unwrap_or(1);
    let Some(op) = g.ops.get_mut(&tzid) else {
        return json!({ "response": "ERROR_NO_TZID" });
    };
    if op.service != "rent" || op.closed {
        return json!({ "response": "ERROR_NO_TZID" });
    }
    let add_secs = days.saturating_mul(86_400);
    // Mock keeps no separate time field on Operation; surface extended window in response.
    json!({
        "response": 1,
        "item": {
            "tzid": tzid,
            "status": 1,
            "messages": [],
            "country": op.country,
            "rent": 1,
            "extension": 1,
            "sum": 50.0,
            "number": op.number,
            "time": add_secs,
            "hours": days.saturating_mul(24),
            "extend": { "1": 40.0, "7": 200.0 },
            "checked": true,
            "reload": 0,
            "day_extend": 40.0
        }
    })
}

fn handle_rent_port_reload_inner(g: &mut MockState, query: &HashMap<String, String>) -> Value {
    let Some(tzid) = query.get("tzid").and_then(|s| s.parse::<i64>().ok()) else {
        return json!({ "response": "ERROR_NO_TZID" });
    };
    if g.ops
        .get(&tzid)
        .is_some_and(|op| op.service == "rent" && !op.closed)
    {
        json!({ "response": "1", "tzid": tzid })
    } else {
        json!({ "response": "ERROR_NO_TZID" })
    }
}

fn handle_rent_state(state: &Arc<Mutex<MockState>>, query: &HashMap<String, String>) -> Value {
    let g = state.lock().expect("mock state");
    let tzid = query.get("tzid").and_then(|s| s.parse::<i64>().ok());
    let list: Vec<Value> = g
        .ops
        .iter()
        .filter(|(id, op)| {
            op.service == "rent" && !op.closed && tzid.map(|t| t == **id).unwrap_or(true)
        })
        .map(|(id, op)| {
            json!({
                "tzid": id,
                "status": 1,
                "messages": [],
                "country": op.country,
                "rent": 1,
                "number": op.number,
                "time": 86400,
                "hours": 24,
                "extend": {},
                "checked": true,
                "reload": 0,
                "day_extend": 0.0
            })
        })
        .collect();
    json!({ "response": 1, "list": list })
}

fn handle_rent_tariffs(query: &HashMap<String, String>) -> Value {
    let one = json!({
        "code": DEFAULT_COUNTRY,
        "enabled": true,
        "name": "USA",
        "new": false,
        "position": 1,
        "count": { "1": 10.0 },
        "days": { "1": 50.0, "7": 200.0 },
        "extend": 40.0
    });
    if query.get("country").is_some() {
        let mut body = one;
        if let Some(obj) = body.as_object_mut() {
            obj.insert("response".into(), json!("1"));
        }
        body
    } else {
        let mut map = serde_json::Map::new();
        map.insert("response".into(), json!("1"));
        map.insert(DEFAULT_COUNTRY.to_string(), one);
        Value::Object(map)
    }
}
