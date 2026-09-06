//! Blocking temporary SMS numbers API.

use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::DEFAULT_COUNTRY;
use crate::error::{Error, Result};
use crate::types::numbers::{
    GetNumberParams, NumberWithTz, StateOne, TariffCountryOne, WaitCodeOptions,
};

use super::http::BlockingHttp;

/// Temporary numbers / SMS operations (blocking).
#[derive(Debug, Clone)]
pub struct NumbersApi {
    pub(crate) http: BlockingHttp,
}

#[derive(Debug, Deserialize)]
struct PriceResp {
    price: Value,
}

#[derive(Debug, Deserialize)]
struct TzidResp {
    tzid: i64,
}

#[derive(Debug, Deserialize)]
struct GetNumResp {
    tzid: i64,
    number: Value,
    #[serde(default)]
    country: Option<i64>,
    #[serde(default)]
    time: Option<i64>,
    #[serde(default)]
    service: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    response_text: Option<String>,
    #[serde(default)]
    guard_interval_remaining_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ServiceResp {
    service: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ServiceNumberResp {
    number: Vec<String>,
}

impl NumbersApi {
    /// Get price for a service (country default [`DEFAULT_COUNTRY`]).
    pub fn price(&self, service: &str) -> Result<String> {
        self.price_in(service, DEFAULT_COUNTRY)
    }

    /// Get price for a service in a country.
    pub fn price_in(&self, service: &str, country: i64) -> Result<String> {
        let resp: PriceResp = self.http.get_onlinesim(
            "getPrice",
            json!({ "service": service, "country": country }),
            true,
        )?;
        match resp.price {
            Value::String(s) => Ok(s),
            Value::Number(n) => Ok(n.to_string()),
            other => Ok(other.to_string()),
        }
    }

    /// Order a number with default country.
    pub fn get(&self, service: &str) -> Result<i64> {
        self.get_with(GetNumberParams::new(service))
    }

    /// Order a number with explicit parameters.
    pub fn get_with(&self, params: GetNumberParams<'_>) -> Result<i64> {
        let resp: TzidResp = self.http.get_onlinesim(
            "getNum",
            json!({
                "service": params.service,
                "country": params.country,
                "reject": params.reject,
                "extension": params.extension,
            }),
            true,
        )?;
        Ok(resp.tzid)
    }

    /// Order a number and return `tzid` + number.
    pub fn get_with_number(&self, service: &str) -> Result<NumberWithTz> {
        self.get_with_number_params(GetNumberParams::new(service))
    }

    /// Order a number with explicit parameters and return enriched payload.
    pub fn get_with_number_params(&self, params: GetNumberParams<'_>) -> Result<NumberWithTz> {
        let resp: GetNumResp = self.http.get_onlinesim(
            "getNum",
            json!({
                "service": params.service,
                "country": params.country,
                "reject": params.reject,
                "extension": params.extension,
                "number": true,
            }),
            true,
        )?;
        let number = match resp.number {
            Value::String(s) => s,
            Value::Number(n) => n.to_string(),
            other => other.to_string(),
        };
        Ok(NumberWithTz {
            tzid: resp.tzid,
            number,
            country: resp.country.unwrap_or(params.country),
            time: resp.time,
            service: resp.service,
            title: resp.title,
            response_text: resp.response_text,
            guard_interval_remaining_seconds: resp.guard_interval_remaining_seconds,
        })
    }

    /// List operation states.
    pub fn state(
        &self,
        message_to_code: i64,
        orderby: &str,
        msg_list: bool,
        clean: bool,
        repeat: bool,
    ) -> Result<Vec<StateOne>> {
        let type_ = if repeat { "repeat" } else { "index" };
        self.http.get_onlinesim(
            "getState",
            json!({
                "message_to_code": message_to_code,
                "orderby": orderby,
                "msg_list": if msg_list { 1 } else { 0 },
                "clean": if clean { 1 } else { 0 },
                "type": type_,
            }),
            true,
        )
    }

    /// State for a single `tzid` with default flags.
    ///
    /// # Warning
    ///
    /// Prefer [`Self::state`] and filter locally: one request for all active
    /// numbers is cheaper and less likely to hit rate limits than polling each
    /// `tzid` separately.
    pub fn state_one(&self, tzid: i64) -> Result<StateOne> {
        self.state_one_with(tzid, 1, true, true, false)
    }

    /// State for a single `tzid` with explicit flags.
    ///
    /// # Warning
    ///
    /// Prefer [`Self::state`] and filter locally: one request for all active
    /// numbers is cheaper and less likely to hit rate limits than polling each
    /// `tzid` separately.
    pub fn state_one_with(
        &self,
        tzid: i64,
        message_to_code: i64,
        msg_list: bool,
        clean: bool,
        repeat: bool,
    ) -> Result<StateOne> {
        let list: Vec<StateOne> = self.http.get_onlinesim(
            "getState",
            json!({
                "tzid": tzid,
                "message_to_code": message_to_code,
                "msg_list": msg_list,
                "clean": clean,
                "repeat": repeat,
            }),
            true,
        )?;
        list.into_iter()
            .next()
            .ok_or_else(|| Error::Unexpected("empty getState response".into()))
    }

    /// Request next SMS.
    pub fn next(&self, tzid: i64) -> Result<bool> {
        let _: Value =
            self.http
                .get_onlinesim("setOperationRevise", json!({ "tzid": tzid }), true)?;
        Ok(true)
    }

    /// Close operation.
    pub fn close(&self, tzid: i64) -> Result<bool> {
        let _: Value = self
            .http
            .get_onlinesim("setOperationOk", json!({ "tzid": tzid }), true)?;
        Ok(true)
    }

    /// Ban number and close.
    pub fn ban(&self, tzid: i64) -> Result<bool> {
        let _: Value =
            self.http
                .get_onlinesim("setOperationOk", json!({ "tzid": tzid, "ban": 1 }), true)?;
        Ok(true)
    }

    /// Repeat reception.
    pub fn repeat(&self, service: &str, number: i64) -> Result<i64> {
        let resp: TzidResp = self.http.get_onlinesim(
            "getNumRepeat",
            json!({ "service": service, "number": number }),
            true,
        )?;
        Ok(resp.tzid)
    }

    /// Tariffs for all countries.
    pub fn tariffs(&self) -> Result<HashMap<String, TariffCountryOne>> {
        self.http
            .get_onlinesim("getNumbersStats", json!({ "country": "all" }), true)
    }

    /// Tariffs for one country (default [`DEFAULT_COUNTRY`]).
    pub fn tariffs_one(&self) -> Result<TariffCountryOne> {
        self.tariffs_one_in(DEFAULT_COUNTRY)
    }

    /// Tariffs for an explicit country.
    pub fn tariffs_one_in(&self, country: i64) -> Result<TariffCountryOne> {
        self.http
            .get_onlinesim("getNumbersStats", json!({ "country": country }), true)
    }

    /// List services.
    pub fn service(&self) -> Result<Vec<String>> {
        let resp: ServiceResp = self.http.get_onlinesim("getService", json!({}), true)?;
        Ok(resp.service)
    }

    /// Numbers available for a service.
    pub fn service_number(&self, service: &str) -> Result<Vec<String>> {
        let resp: ServiceNumberResp =
            self.http
                .get_onlinesim("getServiceNumber", json!({ "service": service }), true)?;
        Ok(resp.number)
    }

    /// Poll until an SMS code arrives (`interval_secs` in seconds).
    pub fn wait_code(&self, tzid: i64, options: WaitCodeOptions) -> Result<String> {
        let message_to_code = if options.full_message { 0 } else { 1 };
        let mut last_code = String::new();
        let mut attempts = 0u32;

        loop {
            thread::sleep(Duration::from_secs(options.interval_secs));
            attempts += 1;
            if attempts > options.max_attempts {
                return Err(Error::Timeout);
            }

            let response = self.state_one_with(tzid, message_to_code, false, true, false)?;

            if let Some(msg) = response.msg {
                let code = match msg {
                    Value::String(s) => s,
                    Value::Number(n) => n.to_string(),
                    other => other.to_string(),
                };
                if code != last_code {
                    last_code = code;
                    if options.not_end {
                        self.next(tzid)?;
                    } else {
                        self.close(tzid)?;
                    }
                    break;
                }
            }
        }

        Ok(last_code)
    }
}
