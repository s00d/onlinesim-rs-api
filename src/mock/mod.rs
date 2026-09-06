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
//! # Example
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
//! Interval `0` is useful in tests (no sleep). Production code should use a few seconds.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use crate::config::DEFAULT_COUNTRY;
use crate::error::Result;
use crate::Client;

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

#[derive(Debug, Clone)]
struct Operation {
    service: String,
    number: String,
    country: i64,
    code: String,
    polls: u32,
    polls_before_code: u32,
    closed: bool,
}

#[derive(Debug, Default)]
struct MockState {
    balance: f64,
    zbalance: f64,
    income: f64,
    /// If set, next `getBalance` returns this API error code once.
    balance_error: Option<String>,
    /// Profile webhook URL.
    webhook_url: Option<String>,
    next_tzid: i64,
    scripts: Vec<SmsScript>,
    ops: HashMap<i64, Operation>,
    no_number_for: Vec<String>,
}

/// Local OnlineSim mock server.
pub struct MockOnlineSim {
    server: MockServer,
    state: Arc<Mutex<MockState>>,
}

impl MockOnlineSim {
    /// Start a mock server with default balance `100` and mount all handlers.
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        let state = Arc::new(Mutex::new(MockState {
            balance: 100.0,
            zbalance: 0.0,
            income: 25.0,
            balance_error: None,
            webhook_url: None,
            next_tzid: 1000,
            scripts: Vec::new(),
            ops: HashMap::new(),
            no_number_for: Vec::new(),
        }));

        mount_handlers(&server, state.clone()).await;

        Self { server, state }
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
        let mut g = self.state.lock().expect("mock state");
        g.balance = balance;
        g.zbalance = zbalance;
        g.income = income;
        g.balance_error = None;
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

    /// Raw wiremock URI (without `/api/`).
    pub fn uri(&self) -> String {
        self.server.uri()
    }
}

async fn mount_handlers(server: &MockServer, state: Arc<Mutex<MockState>>) {
    Mock::given(method("GET"))
        .and(wiremock::matchers::path_regex(r"^/api/.*"))
        .respond_with(ApiResponder {
            state: state.clone(),
        })
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(wiremock::matchers::path_regex(r"^/api/.*"))
        .respond_with(ApiResponder { state })
        .mount(server)
        .await;
}

#[derive(Clone)]
struct ApiResponder {
    state: Arc<Mutex<MockState>>,
}

impl wiremock::Respond for ApiResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let path = request.url.path();
        let path = path.strip_prefix('/').unwrap_or(path);
        // Expect /api/<endpoint>.php or /api/<endpoint>
        let rest = path
            .strip_prefix("api/")
            .unwrap_or(path)
            .trim_end_matches(".php");

        let query: HashMap<String, String> = request
            .url
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let body = match rest {
            "getBalance" => handle_balance(&self.state),
            "getPrice" => handle_price(&self.state, &query),
            "getNum" => handle_get_num(&self.state, &query),
            "getState" => handle_get_state(&self.state, &query),
            "setOperationOk" => handle_close(&self.state, &query),
            "setOperationRevise" => handle_next(&self.state, &query),
            "getNumbersStats" => handle_tariffs(&query),
            "getProfile" => handle_profile(&self.state),
            "profile" => handle_profile_save(&self.state, request),
            "webhook-logs" => handle_webhook_logs(),
            "getPaymentHistory" => handle_payment_history(),
            "getFreeCountryList" => handle_free_countries(),
            "getFreePhoneList" => handle_free_numbers(&query),
            "getFreeMessageList" => handle_free_messages(),
            "getFreeList" => handle_free_list(),
            "rent/getRentNum" => handle_rent_get(&self.state, &query),
            "rent/getRentState" => handle_rent_state(&self.state, &query),
            "rent/closeRentNum" => json!({ "response": "1" }),
            "rent/portReload" => json!({ "response": "1" }),
            "rent/extendRentState" => handle_rent_get(&self.state, &query),
            "rent/tariffsRent" => handle_rent_tariffs(&query),
            "getService" => json!({ "response": "1", "service": ["telegram", "whatsapp"] }),
            "getServiceNumber" => {
                json!({ "response": "1", "number": ["79001112233", "79001112234"] })
            }
            "getNumRepeat" => handle_get_num(&self.state, &query),
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

fn handle_get_num(state: &Arc<Mutex<MockState>>, query: &HashMap<String, String>) -> Value {
    let mut g = state.lock().expect("mock state");
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
        .or_else(|| if g.scripts.is_empty() { None } else { Some(0) });

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

fn handle_get_state(state: &Arc<Mutex<MockState>>, query: &HashMap<String, String>) -> Value {
    let mut g = state.lock().expect("mock state");
    let tzid = query.get("tzid").and_then(|s| s.parse::<i64>().ok());

    let ids: Vec<i64> = match tzid {
        Some(id) => vec![id],
        None => g.ops.keys().copied().collect(),
    };

    if ids.is_empty() {
        return json!({ "response": "ERROR_NO_OPERATIONS" });
    }

    let mut list = Vec::new();
    for id in ids {
        let Some(op) = g.ops.get_mut(&id) else {
            continue;
        };
        if op.closed {
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

    // getState returns a bare array (JS client treats it as post-parsed body).
    Value::Array(list)
}

fn handle_close(state: &Arc<Mutex<MockState>>, query: &HashMap<String, String>) -> Value {
    let mut g = state.lock().expect("mock state");
    if let Some(tzid) = query.get("tzid").and_then(|s| s.parse::<i64>().ok()) {
        if let Some(op) = g.ops.get_mut(&tzid) {
            op.closed = true;
        }
        return json!({ "response": 1, "tzid": tzid });
    }
    json!({ "response": "ERROR_NO_TZID" })
}

fn handle_next(state: &Arc<Mutex<MockState>>, query: &HashMap<String, String>) -> Value {
    let g = state.lock().expect("mock state");
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

fn handle_profile_save(state: &Arc<Mutex<MockState>>, request: &Request) -> Value {
    let mut g = state.lock().expect("mock state");
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

fn handle_rent_get(state: &Arc<Mutex<MockState>>, query: &HashMap<String, String>) -> Value {
    let mut g = state.lock().expect("mock state");
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
            "extend": [],
            "checked": true,
            "reload": 0,
            "day_extend": 0
        }
    })
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
                "extend": [],
                "checked": true,
                "reload": 0,
                "day_extend": 0
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
