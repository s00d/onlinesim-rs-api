//! The mock's routes: endpoint path (relative to `/api/`) → handler. A handler gets the state and the
//! request and returns the JSON body the real backend would send (HTTP 200). Shapes follow the OpenAPI spec
//! and the live captures: `response` as int or string per endpoint, decimal-string money, `[]` for empty
//! maps. Auth, queued failures, 456 and 404 are the dispatcher's job (`mod.rs`), not the handlers'.

use serde_json::{Map, Value, json};

use super::state::{
    MOCK_USER_ID, MockOperation, MockRent, MockRequest, MockSms, MockState, OPERATION_TIME,
    extract_code, from_mills, stamp, to_mills,
};
use crate::transport::Method;
use crate::types::OperationStatus;

pub(super) type Handler = fn(&mut MockState, &MockRequest) -> Value;

/// Endpoints that answer without a key.
pub(super) const PUBLIC_PATHS: &[&str] =
    &["getNumbersStats.php", "rent/tariffsRent.php", "getFreeList"];

pub(super) fn route(path: &str) -> Option<Handler> {
    Some(match path {
        "getPrice.php" => get_price,
        "getNum.php" => get_num,
        "getState.php" => get_state,
        "setOperationRevise.php" => set_operation_revise,
        "setOperationOk.php" => set_operation_ok,
        "getNumbersStats.php" => get_numbers_stats,
        "rent/getRentNum.php" => get_rent_num,
        "rent/getRentState.php" => get_rent_state,
        "rent/extendRentState.php" => extend_rent_state,
        "rent/closeRentNum.php" => close_rent_num,
        "rent/tariffsRent.php" => tariffs_rent,
        "getBalance.php" => get_balance,
        "getProfile.php" => get_profile,
        "profile.php" => profile,
        "webhook-logs" => webhook_logs,
        "getPaymentHistory.php" => get_payment_history,
        "getFreeList" => get_free_list,
        _ => return None,
    })
}

fn error(code: &str) -> Value {
    json!({ "response": code })
}

fn arg<'a>(req: &'a MockRequest, key: &str) -> Option<&'a str> {
    req.query.get(key).map(String::as_str)
}

fn flag(req: &MockRequest, key: &str) -> bool {
    arg(req, key).is_some_and(|v| v != "0")
}

fn int(req: &MockRequest, key: &str, fallback: i64) -> i64 {
    arg(req, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(fallback)
}

/// USD with `digits` decimals (3 or 2), as the backend sends money strings.
fn decimal(mills: i64, digits: u32) -> String {
    let sign = if mills < 0 { "-" } else { "" };
    let mills = mills.unsigned_abs();
    if digits == 2 {
        let cents = (mills + 5) / 10;
        format!("{sign}{}.{:02}", cents / 100, cents % 100)
    } else {
        format!("{sign}{}.{:03}", mills / 1000, mills % 1000)
    }
}

fn country_name(code: i64) -> (String, String) {
    match code {
        7 => ("russia".into(), "Russia".into()),
        44 => ("united_kingdom".into(), "United Kingdom".into()),
        _ => (format!("country_{code}"), format!("Country {code}")),
    }
}

/// `[]` for an empty map, as PHP sends it.
fn map_or_list(map: Map<String, Value>) -> Value {
    if map.is_empty() {
        json!([])
    } else {
        Value::Object(map)
    }
}

/// Laravel paginator over `items`.
fn paginate(items: Vec<Value>, page: i64, per_page: i64, path: &str) -> Value {
    let page = page.max(1);
    let per_page = per_page.max(1);
    let total = items.len() as i64;
    let last_page = ((total + per_page - 1) / per_page).max(1);
    let data: Vec<Value> = items
        .into_iter()
        .skip(((page - 1) * per_page) as usize)
        .take(per_page as usize)
        .collect();
    let url = |p: i64| format!("https://onlinesim.host/api/{path}?page={p}");
    let from = (page - 1) * per_page + 1;
    let count = data.len() as i64;
    json!({
        "current_page": page,
        "data": data,
        "first_page_url": url(1),
        "from": if count > 0 { json!(from) } else { Value::Null },
        "last_page": last_page,
        "last_page_url": url(last_page),
        "links": [],
        "next_page_url": if page < last_page { json!(url(page + 1)) } else { Value::Null },
        "path": format!("https://onlinesim.host/api/{path}"),
        "per_page": per_page,
        "prev_page_url": if page > 1 { json!(url(page - 1)) } else { Value::Null },
        "to": if count > 0 { json!(from + count - 1) } else { Value::Null },
        "total": total,
    })
}

// ----- numbers -----

fn msg_of(sms: &MockSms, to_code: bool) -> String {
    match (&sms.code, to_code) {
        (Some(code), true) => code.clone(),
        _ => sms.text.clone(),
    }
}

fn operation_view(state: &MockState, op: &MockOperation, req: &MockRequest) -> Value {
    let to_code = flag(req, "message_to_code");
    let open = op.status != OperationStatus::TzOverOk;
    let mut view = json!({
        "country": op.country,
        "sum": op.sum,
        "service": op.service,
        "number": op.number,
        "subscription_package_id": null,
        "webhook_url": state.webhook_url,
        "response": op.status.as_str(),
        "tzid": op.tzid,
        "time": if open { OPERATION_TIME } else { 0 },
        "form": "index",
    });
    if flag(req, "msg_list") {
        if !op.messages.is_empty() {
            let list: Vec<Value> = op
                .messages
                .iter()
                .map(|s| json!({ "service": s.service, "msg": msg_of(s, to_code) }))
                .collect();
            view["msg"] = json!(list);
        }
    } else if let Some(current) = op.messages.get(op.cursor) {
        view["msg"] = json!(msg_of(current, to_code));
    }
    if arg(req, "number").is_some() {
        let guard = if open && op.messages.is_empty() {
            state.options.guard_interval
        } else {
            0
        };
        view["guard_interval_remaining_seconds"] = json!(guard);
    }
    view
}

fn get_price(state: &mut MockState, req: &MockRequest) -> Value {
    let Some(prices) = state.options.prices.get(&int(req, "country", 7)) else {
        return error("UNDEFINED_COUNTRY");
    };
    let price = prices.get(arg(req, "service").unwrap_or_default());
    json!({ "response": "1", "price": price })
}

fn get_num(state: &mut MockState, req: &MockRequest) -> Value {
    let country = int(req, "country", 7);
    let service = arg(req, "service").unwrap_or_default().to_string();
    let Some(prices) = state.options.prices.get(&country) else {
        return error("UNDEFINED_COUNTRY");
    };
    let Some(&price) = prices.get(&service) else {
        return error("ERROR_NO_SERVICE");
    };
    if state.available() < to_mills(price) {
        return error("WARNING_LOW_BALANCE");
    }
    if state.no_number_for.contains(&service) {
        return error("NO_NUMBER");
    }

    let script = state
        .scripts
        .iter()
        .position(|s| s.service == service)
        .or((!state.scripts.is_empty()).then_some(0));
    let (number, op_country) = match script {
        Some(i) => (state.scripts[i].number.clone(), state.scripts[i].country),
        None => (
            format!("+{country}{}", MockState::short_number(state.peek_id())),
            country,
        ),
    };
    let digits = number.trim_start_matches('+');
    if arg(req, "reject")
        .unwrap_or_default()
        .split(',')
        .any(|prefix| !prefix.is_empty() && digits.starts_with(prefix))
    {
        return error("NO_NUMBER");
    }

    let tzid = state.next_id();
    if let Some(i) = script {
        let script = state.scripts.remove(i);
        let text = format!("Your code is {}", script.code);
        state.schedule(tzid, &text, script.polls_before_code, None);
    }
    state.operations.insert(
        tzid,
        MockOperation {
            tzid,
            service: service.clone(),
            country: op_country,
            number: number.clone(),
            sum: price,
            status: OperationStatus::TzNumWait,
            messages: Vec::new(),
            cursor: 0,
        },
    );
    if arg(req, "number").is_none() {
        return json!({ "response": 1, "tzid": tzid });
    }
    json!({
        "response": 1,
        "tzid": tzid,
        "number": number,
        "country": op_country,
        "time": OPERATION_TIME,
        "service": service,
    })
}

fn get_state(state: &mut MockState, req: &MockRequest) -> Value {
    let ids: Vec<i64> = match arg(req, "tzid") {
        Some(_) => {
            let tzid = int(req, "tzid", -1);
            state
                .operations
                .contains_key(&tzid)
                .then_some(tzid)
                .into_iter()
                .collect()
        }
        None => state.operations.keys().copied().collect(),
    };
    for &tzid in &ids {
        state.poll(tzid);
    }
    if ids.is_empty() {
        return error("ERROR_NO_OPERATIONS");
    }
    let mut views: Vec<Value> = ids
        .iter()
        .map(|id| operation_view(state, &state.operations[id], req))
        .collect();
    if arg(req, "orderby") == Some("desc") {
        views.reverse();
    }
    Value::Array(views)
}

fn set_operation_revise(state: &mut MockState, req: &MockRequest) -> Value {
    let tzid = int(req, "tzid", 0);
    // Always succeeds, even for unknown tzids, like the backend.
    if let Some(op) = state
        .operations
        .get_mut(&tzid)
        .filter(|op| op.status != OperationStatus::TzOverOk)
    {
        op.cursor = op.messages.len();
        op.status = OperationStatus::TzNumWait;
    }
    json!({ "response": 1, "tzid": tzid })
}

fn set_operation_ok(state: &mut MockState, req: &MockRequest) -> Value {
    let tzid = int(req, "tzid", 0);
    let guard = state.options.guard_interval;
    let Some(op) = state
        .operations
        .get_mut(&tzid)
        .filter(|op| op.status != OperationStatus::TzOverOk)
    else {
        return error("ERROR_WRONG_TZID");
    };
    let received = !op.messages.is_empty();
    if flag(req, "ban") && received {
        return json!({ "response": "0" });
    }
    if !received && guard > 0 {
        return error("NO_COMPLETE_TZID");
    }
    // Closing releases the frozen price (`frozen()` counts open operations only); it is spent only when
    // an SMS arrived.
    op.status = OperationStatus::TzOverOk;
    if received {
        state.balance -= to_mills(op.sum);
    }
    json!({ "response": 1, "tzid": tzid })
}

fn country_stat(state: &MockState, code: i64) -> Value {
    let (name, locale) = country_name(code);
    let mut services = Map::new();
    if let Some(prices) = state.options.prices.get(&code) {
        for (i, (slug, price)) in prices.iter().enumerate() {
            services.insert(
                format!("service_{slug}"),
                json!({
                    "count": 100, "popular": false, "code": code, "price": price,
                    "id": i + 1, "service": slug, "slug": slug,
                }),
            );
        }
    }
    json!({
        "name": name,
        "position": code,
        "code": code,
        "other": false,
        "new": false,
        "enabled": true,
        "services": map_or_list(services),
        "locale_name": locale,
    })
}

fn get_numbers_stats(state: &mut MockState, req: &MockRequest) -> Value {
    if arg(req, "country") == Some("all") {
        let all = state
            .options
            .prices
            .keys()
            .map(|&code| (code.to_string(), country_stat(state, code)))
            .collect();
        return map_or_list(all);
    }
    let code = int(req, "country", 7);
    if state.options.prices.contains_key(&code) {
        country_stat(state, code)
    } else {
        error("NO_COUNTRY")
    }
}

// ----- rent -----

fn rent_item(state: &MockState, rent: &MockRent, req: &MockRequest) -> Value {
    let tariff = state
        .options
        .rent_tariffs
        .get(&rent.country)
        .filter(|t| rent.extension && t.extend);
    let start = rent.tzid;
    let messages: Vec<Value> = rent
        .messages
        .iter()
        .rev()
        .map(|m| {
            json!({
                "id": m.id,
                "service": m.service,
                "text": m.text,
                "code": m.code.clone().unwrap_or_default(),
                "created_at": m.created_at,
            })
        })
        .collect();
    let extend = tariff.map_or_else(Map::new, |t| {
        t.days
            .iter()
            .map(|(d, p)| (d.to_string(), json!(p)))
            .collect()
    });
    let mut item = json!({
        "status": 1,
        "extension": i64::from(rent.extension),
        "messages": messages,
        "webhook_url": state.webhook_url,
        "sum": decimal(to_mills(rent.sum), 2),
        "country": rent.country,
        "start_time": stamp(start),
        "stop_time": stamp(start + rent.days * 86_400),
        "checked_time": stamp(start),
        "number": rent.number,
        "days_autoextend": 0,
        "rent": 1,
        "tzid": rent.tzid,
        "time": rent.days * 24 * 60,
        "days": rent.days,
        "hours": rent.days * 24,
        "extend": map_or_list(extend),
        "checked": true,
        "reload": null,
        "day_extend": tariff.and_then(|t| t.days.get(&1)).copied().unwrap_or(0.0),
        "m_ext": false,
        "freeze": false,
    });
    if arg(req, "number").is_some() {
        item["time_seconds"] = json!(rent.days * 86_400);
    }
    item
}

fn get_rent_num(state: &mut MockState, req: &MockRequest) -> Value {
    let country = int(req, "country", 7);
    let Some(tariff) = state.options.rent_tariffs.get(&country) else {
        return error("NO_COUNTRY");
    };
    let days = int(req, "days", 1);
    let Some(&price) = tariff.days.get(&days) else {
        return error("UNDEFINED_DAYS");
    };
    if state.available() < to_mills(price) {
        return error("WARNING_LOW_BALANCE");
    }
    let tzid = state.next_id();
    state.balance -= to_mills(price);
    let rent = MockRent {
        tzid,
        country,
        number: MockState::short_number(tzid),
        days,
        sum: price,
        // `extension` defaults to 1 on the server; `0` makes the rent non-extendable for good.
        extension: arg(req, "extension") != Some("0"),
        messages: Vec::new(),
    };
    let item = rent_item(state, &rent, req);
    state.rents.insert(tzid, rent);
    json!({ "response": 1, "item": item })
}

fn get_rent_state(state: &mut MockState, req: &MockRequest) -> Value {
    let ids: Vec<i64> = match arg(req, "tzid") {
        Some(_) => {
            let tzid = int(req, "tzid", -1);
            state
                .rents
                .contains_key(&tzid)
                .then_some(tzid)
                .into_iter()
                .collect()
        }
        None => state.rents.keys().copied().collect(),
    };
    for &tzid in &ids {
        state.poll(tzid);
    }
    let list: Vec<Value> = ids
        .iter()
        .map(|id| rent_item(state, &state.rents[id], req))
        .collect();
    json!({ "response": 1, "list": list })
}

fn extend_rent_state(state: &mut MockState, req: &MockRequest) -> Value {
    let tzid = int(req, "tzid", 0);
    let Some(rent) = state.rents.get(&tzid) else {
        return error("ERROR_WRONG_TZID");
    };
    let Some(tariff) = state
        .options
        .rent_tariffs
        .get(&rent.country)
        .filter(|t| rent.extension && t.extend)
    else {
        return error("TRY_AGAIN_LATER");
    };
    let Some(&price) = tariff.days.get(&int(req, "days", 1)) else {
        return error("UNDEFINED_DAYS");
    };
    if state.available() < to_mills(price) {
        return error("WARNING_LOW_BALANCE");
    }
    state.balance -= to_mills(price);
    let rent = state.rents.get_mut(&tzid).expect("checked above");
    rent.days += int(req, "days", 1);
    rent.sum = from_mills(to_mills(rent.sum) + to_mills(price));
    let item = rent_item(state, &state.rents[&tzid], req);
    json!({ "response": 1, "item": item })
}

fn close_rent_num(state: &mut MockState, req: &MockRequest) -> Value {
    let tzid = int(req, "tzid", 0);
    if state.rents.remove(&tzid).is_none() {
        return error("ERROR_WRONG_TZID");
    }
    state.scheduled.remove(&tzid);
    json!({ "response": "1" })
}

fn tariffs_rent(state: &mut MockState, req: &MockRequest) -> Value {
    let tariff = |code: i64| -> Value {
        let Some(t) = state.options.rent_tariffs.get(&code) else {
            return json!([]);
        };
        let (name, _) = country_name(code);
        let days: Map<String, Value> = t
            .days
            .iter()
            .map(|(d, p)| (d.to_string(), json!(p)))
            .collect();
        let count: Map<String, Value> =
            t.days.keys().map(|d| (d.to_string(), json!(100))).collect();
        json!({
            "code": code, "enabled": true, "name": name, "new": false, "position": code,
            "count": count, "days": days, "extend": i64::from(t.extend), "confirm": false,
            "ext_counts": count,
        })
    };
    if arg(req, "country").is_some() {
        return tariff(int(req, "country", 0));
    }
    let all = state
        .options
        .rent_tariffs
        .keys()
        .map(|&code| (code.to_string(), tariff(code)))
        .collect();
    map_or_list(all)
}

// ----- user -----

fn get_balance(state: &mut MockState, req: &MockRequest) -> Value {
    let mut res = json!({
        "response": "1",
        "balance": decimal(state.balance, 3),
        "zbalance": from_mills(state.frozen()),
    });
    if arg(req, "income").is_some() {
        res["income"] = json!(0);
        res["income_usd"] = json!("0.000");
    }
    res
}

fn get_profile(state: &mut MockState, _req: &MockRequest) -> Value {
    json!({
        "response": "1",
        "profile": {
            "id": MOCK_USER_ID,
            "hash_id": "mock",
            "name": "mock@example.com",
            "username": "mock@example.com",
            "email": "mock@example.com",
            "email_verified_at": stamp(0),
            "active": "true",
            "apikey": state.options.api_key,
            "api_access": true,
            "is_api": true,
            "number": null,
            "country": null,
            "locale": "en",
            "currency": "USD",
            "number_region": null,
            "number_country": null,
            "number_reject": [],
            "ugroup": 1,
            "verify": 0,
            "block": 0,
            "resetCount": null,
            "created_at": stamp(0),
            "notification_at": null,
            "first_login": false,
            "sub": 0,
            "disable": false,
            "webhook_url": state.webhook_url,
            "is_project_subscription": false,
        },
    })
}

/// `http://` or `https://` followed by a non-empty host.
fn is_http_url(value: &str) -> bool {
    let rest = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"));
    rest.and_then(|r| r.split(['/', '?', '#']).next())
        .and_then(|authority| authority.rsplit('@').next())
        .and_then(|host_port| host_port.split(':').next())
        .is_some_and(|host| !host.is_empty() && !host.contains(char::is_whitespace))
}

fn profile(state: &mut MockState, req: &MockRequest) -> Value {
    let body = req.body.as_ref().and_then(|b| b.get("profile"));
    let Some(profile) = body.filter(|p| req.method == Method::Post && p.is_object()) else {
        return error("ERROR_PARAMS");
    };
    match profile.get("webhook_url") {
        None => {}
        Some(Value::Null) => state.webhook_url = None,
        Some(Value::String(url)) if is_http_url(url) => state.webhook_url = Some(url.clone()),
        Some(_) => {
            return json!({ "response": { "webhook_url": ["The webhook url must be a valid URL."] } });
        }
    }
    json!({ "response": "1", "user": { "id": MOCK_USER_ID, "webhook_url": state.webhook_url } })
}

fn webhook_logs(state: &mut MockState, req: &MockRequest) -> Value {
    let logs = state.webhook_logs.iter().rev().cloned().collect();
    json!({ "response": "1", "data": paginate(logs, int(req, "page", 1), 10, "webhook-logs") })
}

fn get_payment_history(_state: &mut MockState, req: &MockRequest) -> Value {
    json!({
        "response": "1",
        "orders": paginate(Vec::new(), int(req, "page", 1), 10, "getPaymentHistory.php"),
        "paylist": [],
        "domains": [],
        "currency": { "USD": 1 },
    })
}

// ----- free -----

struct FreeCountry {
    code: i64,
    name: &'static str,
    text: &'static str,
    /// Number without country code → (sender, text) of its SMS.
    numbers: &'static [(&'static str, &'static [(&'static str, &'static str)])],
}

const FREE: &[FreeCountry] = &[
    FreeCountry {
        code: 7,
        name: "russia",
        text: "Russia",
        numbers: &[("9001234567", &[("Telegram", "Telegram code: 51423")])],
    },
    FreeCountry {
        code: 44,
        name: "united_kingdom",
        text: "United Kingdom",
        numbers: &[(
            "7459608320",
            &[
                ("Amazon", "Amazon: Use 195895 to reset your password."),
                ("Microsoft", "Microsoft access code: 2991"),
            ],
        )],
    },
];

fn get_free_list(_state: &mut MockState, req: &MockRequest) -> Value {
    let wanted = int(req, "country", 44);
    let data = FREE.iter().find(|c| c.code == wanted).unwrap_or(&FREE[0]);
    let country = data.code;
    let numbers: Map<String, Value> = data
        .numbers
        .iter()
        .map(|(n, _)| {
            let info = json!({
                "country": country, "country_original": data.name, "data_humans": "1 day ago",
                "full_number": format!("{country}{n}"), "is_archive": false,
            });
            (n.to_string(), info)
        })
        .collect();
    let (number, sms) = data
        .numbers
        .iter()
        .find(|(n, _)| Some(*n) == arg(req, "number"))
        .unwrap_or(&data.numbers[0]);
    let messages = sms
        .iter()
        .enumerate()
        .map(|(i, (sender, text))| {
            json!({
                "id": i + 1, "text": text, "in_number": sender, "my_number": number,
                "created_at": stamp(i as i64), "data_humans": "1 day ago",
                "code": extract_code(text).unwrap_or_else(|| "-".into()),
            })
        })
        .collect();
    let mut page = paginate(
        messages,
        int(req, "page", 1),
        int(req, "count", 10),
        "getFreeList",
    );
    page["number"] = json!(number);
    page["country"] = json!(country);
    let countries: Vec<Value> = FREE
        .iter()
        .map(|c| json!({ "country": c.code, "country_text": c.text, "country_original": c.name }))
        .collect();
    json!({ "response": 1, "countries": countries, "numbers": numbers, "messages": page })
}
