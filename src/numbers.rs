//! Numbers for receiving SMS: `client.numbers()`.

use std::collections::HashMap;

use serde_json::Value;

use crate::error::{ApiError, Error, Result};
use crate::types::{CountryStat, NumberOrder, Operation};
use crate::wire::{Call, Request, decode};

/// Numbers for receiving SMS. Get it with `client.numbers()`.
///
/// An operation (`tzid`) is one ordered number: order it with `get`/`get_with_number`, read the SMS with
/// `state_one` or `wait_code`, finish it with `close` (or `ban`). Countries are phone codes (`7`, `44`, …),
/// services are slugs (`google`, `vkcom`, …) as returned by `tariffs`.
#[derive(Debug, Clone, Copy)]
pub struct Numbers<'a, C>(pub(crate) &'a C);

/// Order of `state` results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderBy {
    /// Oldest first (default).
    #[default]
    Asc,
    /// Newest first.
    Desc,
}

impl OrderBy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Options of `numbers().state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateOptions {
    /// `true` (default): `msg` is the code only; `false`: the full SMS text.
    pub message_to_code: bool,
    /// Default [`OrderBy::Asc`].
    pub order_by: OrderBy,
    /// `true`: `msg` is a list of all SMS. Default `false`.
    pub msg_list: bool,
}

impl Default for StateOptions {
    fn default() -> Self {
        Self {
            message_to_code: true,
            order_by: OrderBy::Asc,
            msg_list: false,
        }
    }
}

/// Options of `numbers().state_one`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateOneOptions {
    /// `true` (default): `msg` is the code only; `false`: the full SMS text.
    pub message_to_code: bool,
    /// `true`: `msg` is a list of all SMS. Default `false`.
    pub msg_list: bool,
}

impl Default for StateOneOptions {
    fn default() -> Self {
        Self {
            message_to_code: true,
            msg_list: false,
        }
    }
}

section_methods!(Numbers {
    /// Price (USD) of one number of `service` in `country`; `None` when the country has no price for it.
    fn price(service: &str, country: u32) -> Option<f64>;

    /// Order a number; returns the operation id (`tzid`). `reject`: number prefixes to skip (`&["4470"]`),
    /// `&[]` for the profile list.
    fn get(service: &str, country: u32, reject: &[&str]) -> i64;

    /// Order a number and get it right away together with the operation id.
    fn get_with_number(service: &str, country: u32, reject: &[&str]) -> NumberOrder;

    /// All operations of the account; empty when there are none.
    fn state(options: StateOptions) -> Vec<Operation>;

    /// One operation. `ApiError` `ERROR_NO_OPERATIONS` when it does not exist (any more).
    fn state_one(tzid: i64, options: StateOneOptions) -> Operation;

    /// Switch the operation to its next SMS and keep it open.
    fn next(tzid: i64) -> ();

    /// Finish the operation.
    fn close(tzid: i64) -> ();

    /// Finish the operation and report the number as bad (only before any SMS arrived).
    fn ban(tzid: i64) -> ();

    /// Services, prices and stock of every country, keyed by country code. No key needed.
    fn tariffs() -> HashMap<String, CountryStat>;

    /// Services, prices and stock of one country. No key needed.
    fn tariffs_one(country: u32) -> CountryStat;
});

fn price(service: &str, country: u32) -> Call<Option<f64>> {
    Call::new(
        Request::get("getPrice.php")
            .arg("service", service)
            .arg("country", country),
        |res| Ok(res?.get("price").and_then(crate::de::num)),
    )
}

fn get(service: &str, country: u32, reject: &[&str]) -> Call<i64> {
    Call::new(
        Request::get("getNum.php")
            .arg("service", service)
            .arg("country", country)
            .list("reject", reject),
        |res| tzid(&res?),
    )
}

fn get_with_number(service: &str, country: u32, reject: &[&str]) -> Call<NumberOrder> {
    // `number` is a presence flag on getNum: sent here only.
    Call::new(
        Request::get("getNum.php")
            .arg("service", service)
            .arg("country", country)
            .list("reject", reject)
            .arg("number", 1),
        |res| decode(res?),
    )
}

fn state(options: StateOptions) -> Call<Vec<Operation>> {
    Call::new(
        Request::get("getState.php")
            .flag("message_to_code", options.message_to_code)
            .arg("orderby", options.order_by.as_str())
            .flag("msg_list", options.msg_list)
            .arg("number", 1),
        |res| match res {
            Err(e) if e.code() == Some("ERROR_NO_OPERATIONS") => Ok(Vec::new()),
            res => operations(res?),
        },
    )
}

fn state_one(tzid: i64, options: StateOneOptions) -> Call<Operation> {
    Call::new(
        Request::get("getState.php")
            .arg("tzid", tzid)
            .flag("message_to_code", options.message_to_code)
            .flag("msg_list", options.msg_list)
            .arg("number", 1),
        |res| {
            operations(res?)?
                .into_iter()
                .next()
                .ok_or_else(|| ApiError::new("ERROR_NO_OPERATIONS", None).into())
        },
    )
}

fn next(tzid: i64) -> Call<()> {
    Call::new(
        Request::get("setOperationRevise.php").arg("tzid", tzid),
        |res| res.map(drop),
    )
}

fn close(tzid: i64) -> Call<()> {
    Call::new(
        Request::get("setOperationOk.php").arg("tzid", tzid),
        |res| res.map(drop),
    )
}

fn ban(tzid: i64) -> Call<()> {
    Call::new(
        Request::get("setOperationOk.php")
            .arg("tzid", tzid)
            .flag("ban", true),
        |res| res.map(drop),
    )
}

fn tariffs() -> Call<HashMap<String, CountryStat>> {
    Call::new(
        Request::get("getNumbersStats.php").arg("country", "all"),
        |res| match res? {
            all @ Value::Object(_) => decode(all),
            _ => Ok(HashMap::new()),
        },
    )
}

fn tariffs_one(country: u32) -> Call<CountryStat> {
    Call::new(
        Request::get("getNumbersStats.php").arg("country", country),
        |res| decode(res?),
    )
}

fn tzid(value: &Value) -> Result<i64> {
    value
        .get("tzid")
        .and_then(crate::de::int)
        .ok_or_else(|| Error::Decode("missing tzid".into()))
}

/// A bare list, or `{"list": [...]}` as sent with `with_response=1`.
fn operations(value: Value) -> Result<Vec<Operation>> {
    match value {
        list @ Value::Array(_) => decode(list),
        Value::Object(mut map) => map.remove("list").map_or(Ok(Vec::new()), decode),
        _ => Ok(Vec::new()),
    }
}
