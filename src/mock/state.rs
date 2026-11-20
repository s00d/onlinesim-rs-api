//! State of the mock backend. [`MockState`] is the single owner of everything the fake API remembers:
//! balance, operations, rents, the profile's webhook URL and its delivery log, scheduled SMS, the failure
//! queue and the request log. Handlers (`handlers.rs`) read and change it only through `&mut MockState`;
//! `MockOnlineSim` and the transports reach it through one lock (`super::Shared`).
//!
//! Deterministic: ids and numbers come from a counter, timestamps from a fixed epoch plus that counter.
//! Money is kept in integer thousandths of a USD ("mills") so sums never drift.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::transport::Method;
use crate::types::OperationStatus;

/// Prices of numbers: country code → service slug → USD.
pub type MockPrices = BTreeMap<i64, BTreeMap<String, f64>>;

/// Rent tariff of one country: rent days → USD, and whether rents can be extended.
#[derive(Debug, Clone, PartialEq)]
pub struct MockRentTariff {
    /// Rent days → price, USD.
    pub days: BTreeMap<i64, f64>,
    /// `true`: rents of this country can be extended (`extend: 1` on the wire).
    pub extend: bool,
}

/// Settings of [`MockOnlineSim`](super::MockOnlineSim). Start from `MockOptions::default()`:
///
/// ```
/// use onlinesim_rs_api::mock::MockOptions;
///
/// let options = MockOptions { balance: 10.0, guard_interval: 120, ..MockOptions::default() };
/// assert_eq!(options.api_key, "mock-key");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MockOptions {
    /// Key the mock accepts as `Authorization: Bearer <api_key>`. Default `mock-key`.
    pub api_key: String,
    /// Starting balance, USD. Default `100`.
    pub balance: f64,
    /// Number prices. Default `{7: {vkcom: 0.5, telegram: 1.2, google: 0.3}, 44: {google: 0.2, vkcom: 0.36}}`.
    pub prices: MockPrices,
    /// Rent tariffs. Default `{7: {days: {1: 1.0, 3: 2.5, 7: 5.0}, extend: true}}`.
    pub rent_tariffs: BTreeMap<i64, MockRentTariff>,
    /// Guard interval of new operations and rents, seconds. `> 0`: an operation without SMS, or an
    /// unconfirmed rent without any SMS, cannot be closed (`NO_COMPLETE_TZID`). Default `0`.
    pub guard_interval: u64,
}

impl Default for MockOptions {
    fn default() -> Self {
        let prices = |list: &[(&str, f64)]| -> BTreeMap<String, f64> {
            list.iter().map(|&(s, p)| (s.to_string(), p)).collect()
        };
        Self {
            api_key: "mock-key".into(),
            balance: 100.0,
            prices: BTreeMap::from([
                (
                    7,
                    prices(&[("vkcom", 0.5), ("telegram", 1.2), ("google", 0.3)]),
                ),
                (44, prices(&[("google", 0.2), ("vkcom", 0.36)])),
            ]),
            rent_tariffs: BTreeMap::from([(
                7,
                MockRentTariff {
                    days: BTreeMap::from([(1, 1.0), (3, 2.5), (7, 5.0)]),
                    extend: true,
                },
            )]),
            guard_interval: 0,
        }
    }
}

/// One SMS received by a number operation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MockSms {
    /// Sender / service.
    pub service: String,
    /// Full text.
    pub text: String,
    /// Code extracted from `text` (see [`extract_code`]), `None` when there is none.
    pub code: Option<String>,
}

/// One SMS received by a rent.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MockRentSms {
    /// Message id.
    pub id: i64,
    /// Sender / service (`other` unless given). `notify`: a notice about a prohibited sender; it gets the
    /// integer `code` `0` on the wire and does not confirm the rent.
    pub service: String,
    /// Full text.
    pub text: String,
    /// Code extracted from `text`, `None` when there is none.
    pub code: Option<String>,
    /// `YYYY-MM-DD HH:MM:SS`.
    pub created_at: String,
}

/// A number operation as the mock stores it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MockOperation {
    /// Operation id.
    pub tzid: i64,
    /// Service slug.
    pub service: String,
    /// Country code.
    pub country: i64,
    /// Full number with `+`.
    pub number: String,
    /// Price, USD. Frozen while open; spent on close after an SMS, refunded on close without one.
    pub sum: f64,
    /// `TzNumWait`, `TzNumAnswer` or `TzOverOk`.
    pub status: OperationStatus,
    /// SMS received, oldest first.
    pub messages: Vec<MockSms>,
    /// Index of the SMS shown in `msg`; `next` (setOperationRevise) moves it past the last one.
    pub cursor: usize,
}

/// A rent as the mock stores it. Closed rents are removed.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MockRent {
    /// Operation id.
    pub tzid: i64,
    /// Country code.
    pub country: i64,
    /// Number without the country code, as the backend sends it for rents.
    pub number: String,
    /// Rent days, extensions included.
    pub days: i64,
    /// The initial period (`days` of getRentNum).
    pub initial_days: i64,
    /// `0`: reserved, waiting for the first SMS; `1`: confirmed by an SMS that is not `notify`, charged.
    pub status: i64,
    /// Total cost incl. extensions, USD. Frozen while `status` is 0.
    pub sum: f64,
    /// Extendable (`extension=1`, the default).
    pub extension: bool,
    /// SMS received, oldest first.
    pub messages: Vec<MockRentSms>,
}

/// One request the mock received.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MockRequest {
    /// GET or POST.
    pub method: Method,
    /// Path relative to `/api/`, e.g. `getNum.php`, `rent/getRentNum.php`, `getFreeList`.
    pub path: String,
    /// Query parameters (`lang` included).
    pub query: BTreeMap<String, String>,
    /// Parsed JSON body of a POST, `None` for GET.
    pub body: Option<Value>,
}

impl MockRequest {
    pub(super) fn new(
        method: Method,
        path: &str,
        query: BTreeMap<String, String>,
        body: Option<&str>,
    ) -> Self {
        Self {
            method,
            path: path.to_string(),
            query,
            body: body.and_then(|b| serde_json::from_str(b).ok()),
        }
    }
}

/// What a queued failure answers.
#[derive(Debug, Clone)]
pub(super) enum FailureKind {
    /// `{"response": code}`, HTTP 200.
    Code(String),
    /// A non-JSON body with this HTTP status.
    Status(u16),
}

#[derive(Debug, Clone)]
pub(super) struct Failure {
    pub(super) endpoint: Option<String>,
    pub(super) kind: FailureKind,
}

#[derive(Debug, Clone)]
pub(super) struct ScheduledSms {
    text: String,
    service: Option<String>,
    /// getState / getRentState polls of this tzid still to pass before the SMS arrives.
    polls: u32,
}

/// One scripted number of the 0.2.0 mock API (`script_sms`): the next order of `service` gets this
/// number, and an SMS `Your code is <code>` arrives after `polls_before_code` state polls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsScript {
    /// Service the script applies to; the first unused script of the ordered service wins, else the
    /// first unused script.
    pub service: String,
    /// Number with `+`.
    pub number: String,
    /// Country code reported for the operation.
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

pub(super) const MOCK_USER_ID: i64 = 1;
/// Seconds an operation stays open, as reported in `time`. The mock has no clock, so it never runs down.
pub(super) const OPERATION_TIME: i64 = 1200;
/// Minutes an unconfirmed rent waits for its first SMS (`stop_time` = start + this).
pub(super) const RENT_FIRST_SMS_MINUTES: i64 = 20;
/// Sender of a rent SMS from a prohibited sender: integer `code` `0`, confirms nothing.
pub(super) const NOTIFY: &str = "notify";
/// 2026-01-01 00:00:00 UTC, in days since 1970-01-01.
const EPOCH_DAYS: i64 = 20_454;

/// `YYYY-MM-DD HH:MM:SS` at `seconds` after the fixed mock epoch (2026-01-01 00:00:00 UTC).
pub(super) fn stamp(seconds: i64) -> String {
    let days = EPOCH_DAYS + seconds.div_euclid(86_400);
    let secs = seconds.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

/// The code rule shared by every OnlineSim SDK mock: spaces/dashes between digits are removed, then the
/// first run of 4–8 digits (not part of a longer run) is the code; `None` when there is none.
///
/// ```
/// use onlinesim_rs_api::mock::extract_code;
///
/// assert_eq!(extract_code("Your code: 123-456").as_deref(), Some("123456"));
/// assert_eq!(extract_code("no digits here"), None);
/// ```
pub fn extract_code(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut joined = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let is_sep = |c: char| c == '-' || c.is_whitespace();
        if is_sep(c) && joined.last().is_some_and(char::is_ascii_digit) {
            let end = (i..chars.len())
                .find(|&j| !is_sep(chars[j]))
                .unwrap_or(chars.len());
            if chars.get(end).is_some_and(char::is_ascii_digit) {
                i = end;
                continue;
            }
        }
        joined.push(c);
        i += 1;
    }
    let mut start = 0;
    while start < joined.len() {
        if !joined[start].is_ascii_digit() {
            start += 1;
            continue;
        }
        let end = (start..joined.len())
            .find(|&j| !joined[j].is_ascii_digit())
            .unwrap_or(joined.len());
        if (4..=8).contains(&(end - start)) {
            return Some(joined[start..end].iter().collect());
        }
        start = end;
    }
    None
}

pub(super) fn to_mills(usd: f64) -> i64 {
    (usd * 1000.0).round() as i64
}

pub(super) fn from_mills(mills: i64) -> f64 {
    mills as f64 / 1000.0
}

/// Everything the fake backend remembers. See the module docs.
#[derive(Debug)]
pub(super) struct MockState {
    pub(super) options: MockOptions,
    /// Total balance incl. the frozen part, mills.
    pub(super) balance: i64,
    /// Frozen on top of open operations (the 0.2.0 `set_balance(_, zbalance)`), mills.
    pub(super) extra_frozen: i64,
    pub(super) operations: BTreeMap<i64, MockOperation>,
    pub(super) rents: BTreeMap<i64, MockRent>,
    pub(super) webhook_url: Option<String>,
    pub(super) webhook_logs: Vec<Value>,
    pub(super) requests: Vec<MockRequest>,
    pub(super) failures: Vec<Failure>,
    pub(super) scheduled: BTreeMap<i64, Vec<ScheduledSms>>,
    /// Source of tzids, numbers, message ids and timestamps.
    seq: i64,
    /// 0.2.0 `script_sms` queue.
    pub(super) scripts: Vec<SmsScript>,
    /// 0.2.0 `fail_no_number` services.
    pub(super) no_number_for: Vec<String>,
}

impl MockState {
    pub(super) fn new(options: MockOptions) -> Self {
        Self {
            balance: to_mills(options.balance),
            options,
            extra_frozen: 0,
            operations: BTreeMap::new(),
            rents: BTreeMap::new(),
            webhook_url: None,
            webhook_logs: Vec::new(),
            requests: Vec::new(),
            failures: Vec::new(),
            scheduled: BTreeMap::new(),
            seq: 0,
            scripts: Vec::new(),
            no_number_for: Vec::new(),
        }
    }

    /// Back to the constructor's state.
    pub(super) fn reset(&mut self) {
        *self = Self::new(self.options.clone());
    }

    /// Frozen by open operations and unconfirmed rents (plus the extra frozen part), mills.
    pub(super) fn frozen(&self) -> i64 {
        let operations: i64 = self
            .operations
            .values()
            .filter(|op| op.status != OperationStatus::TzOverOk)
            .map(|op| to_mills(op.sum))
            .sum();
        let rents: i64 = self
            .rents
            .values()
            .filter(|r| r.status == 0)
            .map(|r| to_mills(r.sum))
            .sum();
        self.extra_frozen + operations + rents
    }

    /// Available balance (total minus frozen), mills.
    pub(super) fn available(&self) -> i64 {
        self.balance - self.frozen()
    }

    /// The id the next operation/rent will get.
    pub(super) fn peek_id(&self) -> i64 {
        self.seq + 1
    }

    pub(super) fn next_id(&mut self) -> i64 {
        self.seq += 1;
        self.seq
    }

    /// Short (country-less) number for an id: deterministic, 9 digits.
    pub(super) fn short_number(id: i64) -> String {
        (900_000_000 + id).to_string()
    }

    /// Remove and return the first queued failure that applies to `path`.
    pub(super) fn take_failure(&mut self, path: &str) -> Option<FailureKind> {
        let i = self
            .failures
            .iter()
            .position(|f| f.endpoint.as_deref().is_none_or(|e| e == path))?;
        Some(self.failures.remove(i).kind)
    }

    /// An SMS arrives at operation or rent `tzid` now. Returns `false` when there is no open one.
    pub(super) fn deliver(&mut self, tzid: i64, text: &str, service: Option<&str>) -> bool {
        let code = extract_code(text);
        if let Some(op) = self
            .operations
            .get_mut(&tzid)
            .filter(|op| op.status != OperationStatus::TzOverOk)
        {
            let sms = MockSms {
                service: service.unwrap_or(&op.service).to_string(),
                text: text.to_string(),
                code,
            };
            op.messages.push(sms.clone());
            op.status = OperationStatus::TzNumAnswer;
            // Activation: `+<digits>`, seconds left, the order time.
            let (country, number, start) = (op.country, op.number.clone(), stamp(op.tzid));
            self.webhook(
                "receiving_sms",
                tzid,
                country,
                &number,
                &sms,
                OPERATION_TIME,
                start,
            );
            return true;
        }
        if self.rents.contains_key(&tzid) {
            let id = self.next_id();
            let sms = MockSms {
                service: service.unwrap_or("other").to_string(),
                text: text.to_string(),
                code,
            };
            let rent = self.rents.get_mut(&tzid).expect("checked above");
            rent.messages.push(MockRentSms {
                id,
                service: sms.service.clone(),
                text: sms.text.clone(),
                code: sms.code.clone(),
                created_at: stamp(id),
            });
            if rent.status == 0 && sms.service != NOTIFY {
                // The first real SMS confirms the rent: the reservation is charged (`frozen()` drops it).
                rent.status = 1;
                self.balance -= to_mills(rent.sum);
            }
            let rent = &self.rents[&tzid];
            // Rent: country code + number in digits (no `+`), minutes left, the SMS time.
            let number = format!("{}{}", rent.country, rent.number);
            let minutes_left = if rent.status == 1 {
                rent.days * 24 * 60
            } else {
                RENT_FIRST_SMS_MINUTES
            };
            let country = rent.country;
            self.webhook(
                "rent_sms",
                tzid,
                country,
                &number,
                &sms,
                minutes_left,
                stamp(id),
            );
            return true;
        }
        false
    }

    pub(super) fn schedule(&mut self, tzid: i64, text: &str, polls: u32, service: Option<&str>) {
        self.scheduled.entry(tzid).or_default().push(ScheduledSms {
            text: text.to_string(),
            service: service.map(str::to_string),
            polls,
        });
    }

    /// One state poll of `tzid`: delivers the scheduled SMS whose polls ran out, counts down the rest.
    pub(super) fn poll(&mut self, tzid: i64) {
        let Some(list) = self.scheduled.remove(&tzid) else {
            return;
        };
        let (due, mut rest): (Vec<_>, Vec<_>) = list.into_iter().partition(|s| s.polls == 0);
        for s in &mut rest {
            s.polls -= 1;
        }
        if !rest.is_empty() {
            self.scheduled.insert(tzid, rest);
        }
        // Dropped silently when the operation was closed meanwhile: a real SMS would not arrive either.
        for s in due {
            self.deliver(tzid, &s.text, s.service.as_deref());
        }
    }

    /// No HTTP call: the mock only records what the backend would have sent.
    #[allow(clippy::too_many_arguments)]
    fn webhook(
        &mut self,
        kind: &str,
        tzid: i64,
        country: i64,
        number: &str,
        sms: &MockSms,
        time_left: i64,
        time_start: String,
    ) {
        let Some(url) = self.webhook_url.clone() else {
            return;
        };
        let id = self.webhook_logs.len() + 1;
        let time = stamp(self.seq);
        self.webhook_logs.push(json!({
            "id": id,
            "user_id": MOCK_USER_ID,
            "webhook_url": url,
            "type": kind,
            "params": {
                "webhook_type": kind,
                "operation_id": tzid,
                "user_id": MOCK_USER_ID,
                "country_code": country,
                "number": number,
                "sender": sms.service,
                "message": sms.text,
                "code": sms.code.as_deref().unwrap_or_default(),
                "time_start": time_start,
                "time_left": time_left,
            },
            "status": "success",
            "error": null,
            "created_at": time,
            "updated_at": time,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_count_from_the_fixed_epoch() {
        assert_eq!(stamp(0), "2026-01-01 00:00:00");
        assert_eq!(stamp(61), "2026-01-01 00:01:01");
        assert_eq!(stamp(59 * 86_400 + 3_600), "2026-03-01 01:00:00");
        assert_eq!(stamp(365 * 86_400), "2027-01-01 00:00:00");
    }

    #[test]
    fn code_rule() {
        let cases = [
            ("Your code: 123-456", Some("123456")),
            ("code 12 34 56 here", Some("123456")),
            ("Microsoft access code: 2991", Some("2991")),
            ("123", None),
            ("1234567890 then 4321", Some("4321")),
            ("call 555 - 1234", Some("5551234")),
            ("abc-1234-", Some("1234")),
            ("plain text", None),
        ];
        for (text, code) in cases {
            assert_eq!(extract_code(text).as_deref(), code, "{text}");
        }
    }
}
