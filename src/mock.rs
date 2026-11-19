//! A local fake OnlineSim server for your own tests (feature `mock`): order → SMS → close without real
//! numbers or money.
//!
//! ```no_run
//! # async fn demo() -> onlinesim_rs_api::Result<()> {
//! use std::time::Duration;
//! use onlinesim_rs_api::WaitCodeOptions;
//! use onlinesim_rs_api::mock::{MockOnlineSim, SmsScript};
//!
//! let mock = MockOnlineSim::start().await;
//! mock.script_sms(SmsScript { code: "999111".into(), polls_before_code: 1, ..SmsScript::default() });
//!
//! let client = mock.client()?;
//! let order = client.numbers().get_with_number("telegram", 1, &[]).await?;
//! let options = WaitCodeOptions { interval: Duration::from_millis(10), ..WaitCodeOptions::default() };
//! assert_eq!(client.numbers().wait_code(order.tzid, options).await?, "999111");
//! # Ok(())
//! # }
//! ```
//!
//! It speaks the wire format of the real API (Bearer key, `response` codes, getState, rent), keeps its
//! state in memory, and answers `ERROR_WRONG_KEY` (HTTP 456) to keyed endpoints called without a key.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{Value, json};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use crate::error::Result;

/// One scripted number: what `getNum` hands out and which SMS arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsScript {
    /// Service the script applies to; the first unused script of the ordered service wins, else the
    /// first unused script.
    pub service: String,
    /// Number with `+`.
    pub number: String,
    /// Country code.
    pub country: i64,
    /// Code of the SMS.
    pub code: String,
    /// `getState` polls that see no SMS before the code arrives.
    pub polls_before_code: u32,
}

impl Default for SmsScript {
    fn default() -> Self {
        Self {
            service: "telegram".into(),
            number: "+19001234567".into(),
            country: 1,
            code: "123456".into(),
            polls_before_code: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct Op {
    script: SmsScript,
    polls: u32,
    /// The code was consumed by `setOperationRevise`.
    revised: bool,
}

#[derive(Debug, Clone)]
struct RentOp {
    country: i64,
    days: i64,
    extension: bool,
}

#[derive(Debug)]
struct State {
    balance: f64,
    zbalance: f64,
    webhook_url: Option<String>,
    next_tzid: i64,
    scripts: Vec<SmsScript>,
    no_number_for: Vec<String>,
    fail_next: Option<String>,
    ops: BTreeMap<i64, Op>,
    rents: BTreeMap<i64, RentOp>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            balance: 100.0,
            zbalance: 0.0,
            webhook_url: None,
            next_tzid: 1000,
            scripts: Vec::new(),
            no_number_for: Vec::new(),
            fail_next: None,
            ops: BTreeMap::new(),
            rents: BTreeMap::new(),
        }
    }
}

/// The fake server. Dropping it stops the server.
#[derive(Debug)]
pub struct MockOnlineSim {
    server: MockServer,
    state: Arc<Mutex<State>>,
}

/// Locks the state; a panic inside a handler cannot poison it for the next requests.
fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl MockOnlineSim {
    /// Start on a free local port. Balance 100 USD, no scripts.
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        let state = Arc::new(Mutex::new(State::default()));
        Mock::given(wiremock::matchers::path_regex(r"^/api/"))
            .respond_with(Responder(state.clone()))
            .mount(&server)
            .await;
        Self { server, state }
    }

    /// API base of the fake, e.g. `http://127.0.0.1:41234/api/`.
    pub fn base_url(&self) -> String {
        format!("{}/api/", self.server.uri())
    }

    /// Async client on the fake, with a test key.
    pub fn client(&self) -> Result<crate::Client> {
        crate::Client::builder()
            .api_key("mock-key")
            .base_url(self.base_url())
            .build()
    }

    /// Blocking client on the fake, with a test key (needs feature `blocking`; create it outside the
    /// async runtime, e.g. in `spawn_blocking`).
    #[cfg(feature = "blocking")]
    pub fn client_blocking(&self) -> Result<crate::blocking::Client> {
        crate::Client::builder()
            .api_key("mock-key")
            .base_url(self.base_url())
            .build_blocking()
    }

    /// Queue a scripted number.
    pub fn script_sms(&self, script: SmsScript) {
        lock(&self.state).scripts.push(script);
    }

    /// Answer `NO_NUMBER` when `service` is ordered.
    pub fn fail_no_number(&self, service: impl Into<String>) {
        lock(&self.state).no_number_for.push(service.into());
    }

    /// Answer the next request with this error code (once).
    pub fn fail_next(&self, code: impl Into<String>) {
        lock(&self.state).fail_next = Some(code.into());
    }

    /// Set the balance and the frozen part.
    pub fn set_balance(&self, balance: f64, zbalance: f64) {
        let mut state = lock(&self.state);
        state.balance = balance;
        state.zbalance = zbalance;
    }

    /// Webhook URL as last set through `profile.php`.
    pub fn webhook_url(&self) -> Option<String> {
        lock(&self.state).webhook_url.clone()
    }
}

struct Responder(Arc<Mutex<State>>);

const KEYLESS: &[&str] = &["getNumbersStats", "rent/tariffsRent", "getFreeList"];

impl Respond for Responder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let path = request.url.path().trim_start_matches("/api/");
        let endpoint = path.strip_suffix(".php").unwrap_or(path);
        let query: HashMap<String, String> = request.url.query_pairs().into_owned().collect();
        let keyed = request.headers.get("authorization").is_some();
        if !keyed && !KEYLESS.contains(&endpoint) {
            return ResponseTemplate::new(456)
                .set_body_json(json!({ "response": "ERROR_WRONG_KEY" }));
        }
        let mut state = lock(&self.0);
        if let Some(code) = state.fail_next.take() {
            return ResponseTemplate::new(200).set_body_json(json!({ "response": code }));
        }
        let body = handle(&mut state, endpoint, &query, &request.body);
        ResponseTemplate::new(200).set_body_json(body)
    }
}

fn handle(s: &mut State, endpoint: &str, q: &HashMap<String, String>, body: &[u8]) -> Value {
    let arg = |key: &str| q.get(key).map(String::as_str);
    let int = |key: &str| arg(key).and_then(|v| v.parse::<i64>().ok());
    match endpoint {
        "getBalance" => json!({
            "response": "1",
            "balance": format!("{:.3}", s.balance),
            "zbalance": s.zbalance,
            "income": 0
        }),
        "getPrice" => json!({ "response": "1", "price": 0.5 }),
        "getNum" => {
            let service = arg("service").unwrap_or("telegram").to_string();
            if s.no_number_for.contains(&service) || arg("reject").is_some() {
                return json!({ "response": "NO_NUMBER" });
            }
            let index = s
                .scripts
                .iter()
                .position(|sc| sc.service == service)
                .or(if s.scripts.is_empty() { None } else { Some(0) });
            let script = match index {
                Some(i) => s.scripts.remove(i),
                None => SmsScript {
                    service: service.clone(),
                    ..SmsScript::default()
                },
            };
            let tzid = s.next_tzid;
            s.next_tzid += 1;
            s.zbalance += 0.5;
            let order = json!({
                "response": 1,
                "tzid": tzid,
                "number": script.number,
                "country": script.country,
                "time": 1200,
                "service": service,
            });
            s.ops.insert(
                tzid,
                Op {
                    script,
                    polls: 0,
                    revised: false,
                },
            );
            if arg("number").is_some() {
                order
            } else {
                json!({ "response": 1, "tzid": tzid })
            }
        }
        "getState" => {
            let full = arg("message_to_code") == Some("0");
            let ids: Vec<i64> = match int("tzid") {
                Some(id) => vec![id],
                None => s.ops.keys().copied().collect(),
            };
            let list: Vec<Value> = ids
                .into_iter()
                .filter_map(|id| s.ops.get_mut(&id).map(|op| operation(id, op, full)))
                .collect();
            if list.is_empty() {
                json!({ "response": "ERROR_NO_OPERATIONS" })
            } else {
                Value::Array(list)
            }
        }
        "setOperationOk" => match int("tzid").and_then(|id| s.ops.remove(&id)) {
            Some(_) => {
                s.zbalance = (s.zbalance - 0.5).max(0.0);
                json!({ "response": 1, "tzid": int("tzid") })
            }
            None => json!({ "response": "ERROR_WRONG_TZID" }),
        },
        "setOperationRevise" => {
            if let Some(op) = int("tzid").and_then(|id| s.ops.get_mut(&id)) {
                op.revised = true;
            }
            json!({ "response": "1", "tzid": arg("tzid") })
        }
        "getNumbersStats" => {
            let one = |code: i64| {
                json!({
                    "name": "mock", "position": 1, "code": code, "other": false, "new": false, "enabled": true,
                    "services": { "service_telegram": {
                        "count": 10, "popular": true, "code": code, "price": 0.5, "id": 1,
                        "service": "Telegram", "slug": "telegram"
                    } }
                })
            };
            match arg("country") {
                Some("all") | None => json!({ "1": one(1), "44": one(44) }),
                Some(code) => one(code.parse().unwrap_or(1)),
            }
        }
        "getProfile" => json!({
            "response": "1",
            "profile": { "id": 1, "name": "mock", "webhook_url": s.webhook_url, "api_access": true }
        }),
        "profile" => {
            let payload: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
            match &payload["profile"]["webhook_url"] {
                Value::Null => s.webhook_url = None,
                Value::String(url) if url.starts_with("http") => s.webhook_url = Some(url.clone()),
                _ => {
                    return json!({ "response": { "webhook_url": ["The webhook url must be a valid URL."] } });
                }
            }
            json!({ "response": "1" })
        }
        "webhook-logs" => json!({ "response": "1", "data": page(json!([])) }),
        "getPaymentHistory" => json!({ "response": "1", "orders": page(json!([])), "paylist": [] }),
        "getFreeList" => json!({
            "response": 1,
            "countries": [{ "country": 44, "country_text": "United Kingdom", "country_original": "united_kingdom" }],
            "numbers": [],
            "messages": { "current_page": 1, "data": [], "total": 0, "per_page": 10, "last_page": 1, "number": null, "country": 44 }
        }),
        "rent/getRentNum" => {
            let tzid = s.next_tzid;
            s.next_tzid += 1;
            let rent = RentOp {
                country: int("country").unwrap_or(7),
                days: int("days").unwrap_or(1),
                extension: arg("extension") != Some("0"),
            };
            let item = rent_item(tzid, &rent);
            s.rents.insert(tzid, rent);
            json!({ "response": 1, "item": item })
        }
        "rent/getRentState" => {
            let list: Vec<Value> = s
                .rents
                .iter()
                .filter(|(id, _)| int("tzid").is_none_or(|t| t == **id))
                .map(|(id, r)| rent_item(*id, r))
                .collect();
            json!({ "response": 1, "list": list })
        }
        "rent/extendRentState" => match int("tzid").and_then(|id| s.rents.get_mut(&id)) {
            Some(rent) if rent.extension => {
                rent.days += int("days").unwrap_or(1);
                let item = rent_item(int("tzid").unwrap_or_default(), rent);
                json!({ "response": 1, "item": item })
            }
            _ => json!({ "response": "ERROR_WRONG_TZID" }),
        },
        "rent/closeRentNum" => match int("tzid").and_then(|id| s.rents.remove(&id)) {
            Some(_) => json!({ "response": "1" }),
            None => json!({ "response": "ERROR_WRONG_TZID" }),
        },
        "rent/tariffsRent" => {
            let one = |code: i64| {
                json!({
                    "code": code, "name": "mock", "enabled": true, "new": false, "position": 1, "extend": 1,
                    "days": { "1": 1.0, "7": 5.0 }, "count": { "1": 10, "7": 10 }
                })
            };
            match int("country") {
                Some(7) | Some(1) => one(int("country").unwrap_or(7)),
                Some(_) => json!([]),
                None => json!({ "7": one(7), "1": one(1) }),
            }
        }
        _ => json!({ "response": "ERROR_PARAMS" }),
    }
}

fn operation(tzid: i64, op: &mut Op, full: bool) -> Value {
    op.polls += 1;
    let ready = !op.revised && op.polls > op.script.polls_before_code;
    let mut item = json!({
        "tzid": tzid,
        "response": if ready { "TZ_NUM_ANSWER" } else { "TZ_NUM_WAIT" },
        "number": op.script.number,
        "service": op.script.service,
        "country": op.script.country,
        "sum": 0.5,
        "time": 1100,
        "form": "index",
        "guard_interval_remaining_seconds": 0,
    });
    if ready {
        item["msg"] = if full {
            Value::String(format!("Your code is {}", op.script.code))
        } else {
            Value::String(op.script.code.clone())
        };
    }
    item
}

fn rent_item(tzid: i64, rent: &RentOp) -> Value {
    json!({
        "tzid": tzid, "status": 1, "number": "+79001234567", "country": rent.country,
        "sum": format!("{:.2}", rent.days as f64), "days": rent.days, "hours": rent.days * 24,
        "time": rent.days * 1440, "time_seconds": rent.days * 86400,
        "extension": if rent.extension { 1 } else { 0 },
        "extend": if rent.extension { json!({ "1": 1.0, "7": 5.0 }) } else { json!([]) },
        "start_time": "2026-01-01 00:00:00", "stop_time": "2026-01-02 00:00:00", "messages": []
    })
}

fn page(data: Value) -> Value {
    json!({ "current_page": 1, "data": data, "from": null, "to": null, "last_page": 1, "per_page": 10, "total": 0 })
}
