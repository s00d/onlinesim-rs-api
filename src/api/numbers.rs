//! Temporary SMS numbers API.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::DEFAULT_COUNTRY;
use crate::error::{Error, Result};
use crate::http::Http;
use crate::types::numbers::{
    GetNumberParams, NumberWithTz, StateOne, TariffCountryOne, WaitCodeOptions,
};
use crate::wait_code::async_hub::AsyncWaitHub;

/// Temporary numbers / SMS operations.
#[derive(Debug, Clone)]
pub struct NumbersApi {
    pub(crate) http: Http,
    pub(crate) wait_hub: Arc<AsyncWaitHub>,
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
    pub async fn price(&self, service: &str) -> Result<String> {
        self.price_in(service, DEFAULT_COUNTRY).await
    }

    /// Get price for a service in a country.
    pub async fn price_in(&self, service: &str, country: i64) -> Result<String> {
        let resp: PriceResp = self
            .http
            .get_onlinesim(
                "getPrice",
                json!({ "service": service, "country": country }),
                true,
            )
            .await?;
        match resp.price {
            Value::String(s) => Ok(s),
            Value::Number(n) => Ok(n.to_string()),
            other => Ok(other.to_string()),
        }
    }

    /// Order a number with defaults (`country` = [`DEFAULT_COUNTRY`], empty reject).
    pub async fn get(&self, service: &str) -> Result<i64> {
        self.get_with(GetNumberParams::new(service)).await
    }

    /// Order a number with explicit parameters.
    pub async fn get_with(&self, params: GetNumberParams<'_>) -> Result<i64> {
        let resp: TzidResp = self
            .http
            .get_onlinesim(
                "getNum",
                json!({
                    "service": params.service,
                    "country": params.country,
                    "reject": params.reject,
                    "extension": params.extension,
                }),
                true,
            )
            .await?;
        Ok(resp.tzid)
    }

    /// Order a number and return `tzid` + number (default country).
    pub async fn get_with_number(&self, service: &str) -> Result<NumberWithTz> {
        self.get_with_number_params(GetNumberParams::new(service))
            .await
    }

    /// Order a number and return enriched payload (`number=true`).
    pub async fn get_with_number_params(
        &self,
        params: GetNumberParams<'_>,
    ) -> Result<NumberWithTz> {
        let resp: GetNumResp = self
            .http
            .get_onlinesim(
                "getNum",
                json!({
                    "service": params.service,
                    "country": params.country,
                    "reject": params.reject,
                    "extension": params.extension,
                    "number": true,
                }),
                true,
            )
            .await?;
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
    pub async fn state(
        &self,
        message_to_code: i64,
        orderby: &str,
        msg_list: bool,
        clean: bool,
        repeat: bool,
    ) -> Result<Vec<StateOne>> {
        let type_ = if repeat { "repeat" } else { "index" };
        self.http
            .get_onlinesim(
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
            .await
    }

    /// State for a single `tzid` with default flags.
    ///
    /// # Warning
    ///
    /// Prefer [`Self::state`] and filter locally: one request for all active
    /// numbers is cheaper and less likely to hit rate limits than polling each
    /// `tzid` separately.
    pub async fn state_one(&self, tzid: i64) -> Result<StateOne> {
        self.state_one_with(tzid, 1, true, true, false).await
    }

    /// State for a single `tzid` with explicit flags.
    ///
    /// # Warning
    ///
    /// Prefer [`Self::state`] and filter locally: one request for all active
    /// numbers is cheaper and less likely to hit rate limits than polling each
    /// `tzid` separately.
    pub async fn state_one_with(
        &self,
        tzid: i64,
        message_to_code: i64,
        msg_list: bool,
        clean: bool,
        repeat: bool,
    ) -> Result<StateOne> {
        let list: Vec<StateOne> = self
            .http
            .get_onlinesim(
                "getState",
                json!({
                    "tzid": tzid,
                    "message_to_code": message_to_code,
                    "msg_list": msg_list,
                    "clean": clean,
                    "repeat": repeat,
                }),
                true,
            )
            .await?;
        list.into_iter()
            .next()
            .ok_or_else(|| Error::Unexpected("empty getState response".into()))
    }

    /// Request next SMS (`setOperationRevise`).
    pub async fn next(&self, tzid: i64) -> Result<bool> {
        let _: Value = self
            .http
            .get_onlinesim("setOperationRevise", json!({ "tzid": tzid }), true)
            .await?;
        Ok(true)
    }

    /// Close operation (`setOperationOk`).
    pub async fn close(&self, tzid: i64) -> Result<bool> {
        let _: Value = self
            .http
            .get_onlinesim("setOperationOk", json!({ "tzid": tzid }), true)
            .await?;
        Ok(true)
    }

    /// Ban number and close.
    pub async fn ban(&self, tzid: i64) -> Result<bool> {
        let _: Value = self
            .http
            .get_onlinesim("setOperationOk", json!({ "tzid": tzid, "ban": 1 }), true)
            .await?;
        Ok(true)
    }

    /// Repeat reception for a number.
    pub async fn repeat(&self, service: &str, number: i64) -> Result<i64> {
        let resp: TzidResp = self
            .http
            .get_onlinesim(
                "getNumRepeat",
                json!({ "service": service, "number": number }),
                true,
            )
            .await?;
        Ok(resp.tzid)
    }

    /// Tariffs for all countries.
    pub async fn tariffs(&self) -> Result<HashMap<String, TariffCountryOne>> {
        self.http
            .get_onlinesim("getNumbersStats", json!({ "country": "all" }), true)
            .await
    }

    /// Tariffs for one country (default [`DEFAULT_COUNTRY`]).
    pub async fn tariffs_one(&self) -> Result<TariffCountryOne> {
        self.tariffs_one_in(DEFAULT_COUNTRY).await
    }

    /// Tariffs for an explicit country.
    pub async fn tariffs_one_in(&self, country: i64) -> Result<TariffCountryOne> {
        self.http
            .get_onlinesim("getNumbersStats", json!({ "country": country }), true)
            .await
    }

    /// List services.
    pub async fn service(&self) -> Result<Vec<String>> {
        let resp: ServiceResp = self
            .http
            .get_onlinesim("getService", json!({}), true)
            .await?;
        Ok(resp.service)
    }

    /// Numbers available for a service.
    pub async fn service_number(&self, service: &str) -> Result<Vec<String>> {
        let resp: ServiceNumberResp = self
            .http
            .get_onlinesim("getServiceNumber", json!({ "service": service }), true)
            .await?;
        Ok(resp.number)
    }

    /// Wait until an SMS code arrives for `tzid`.
    ///
    /// Concurrent `wait_code` calls on the same [`crate::Client`] share one
    /// background poller that fetches **all** active numbers via [`Self::state`]
    /// (no per-`tzid` `getState`). The poller starts with the first waiter and
    /// stops when none remain.
    ///
    /// `interval_secs` is in **seconds**. When several waiters are active, the
    /// poller uses the **minimum** interval among them.
    pub async fn wait_code(&self, tzid: i64, options: WaitCodeOptions) -> Result<String> {
        self.wait_hub
            .wait_code(self.http.clone(), tzid, options)
            .await
    }
}
