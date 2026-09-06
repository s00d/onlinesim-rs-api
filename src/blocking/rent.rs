//! Blocking rent API.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::DEFAULT_COUNTRY;
use crate::error::{Error, Result};
use crate::types::rent::{RentItem, RentTariff};

use super::http::BlockingHttp;

/// Number rent operations (blocking).
#[derive(Debug, Clone)]
pub struct RentApi {
    pub(crate) http: BlockingHttp,
}

#[derive(Debug, Deserialize)]
struct ItemWrap {
    item: RentItem,
}

#[derive(Debug, Deserialize)]
struct ListWrap {
    list: Vec<RentItem>,
}

impl RentApi {
    /// Rent a number (default country).
    pub fn get(&self) -> Result<RentItem> {
        self.get_with(DEFAULT_COUNTRY, 1, false)
    }

    /// Rent a number with explicit parameters.
    pub fn get_with(&self, country: i64, days: i64, extension: bool) -> Result<RentItem> {
        let resp: ItemWrap = self.http.get_onlinesim(
            "rent/getRentNum",
            json!({
                "country": country,
                "days": days,
                "extension": extension,
                "pagination": false,
            }),
            true,
        )?;
        Ok(resp.item)
    }

    /// List active rents.
    pub fn state(&self) -> Result<Vec<RentItem>> {
        let resp: ListWrap =
            self.http
                .get_onlinesim("rent/getRentState", json!({ "pagination": false }), true)?;
        Ok(resp.list)
    }

    /// State for one rent.
    ///
    /// # Warning
    ///
    /// Prefer [`Self::state`] and filter locally: one request for all active
    /// rents is cheaper and less likely to hit rate limits than polling each
    /// `tzid` separately.
    pub fn state_one(&self, tzid: i64) -> Result<RentItem> {
        let resp: ListWrap = self.http.get_onlinesim(
            "rent/getRentState",
            json!({ "tzid": tzid, "pagination": false }),
            true,
        )?;
        resp.list
            .into_iter()
            .next()
            .ok_or_else(|| Error::Unexpected("empty rent state".into()))
    }

    /// Extend a rent (default 1 day).
    pub fn extend(&self, tzid: i64) -> Result<RentItem> {
        self.extend_days(tzid, 1)
    }

    /// Extend a rent by days.
    pub fn extend_days(&self, tzid: i64, days: i64) -> Result<RentItem> {
        let resp: ItemWrap = self.http.get_onlinesim(
            "rent/extendRentState",
            json!({ "tzid": tzid, "days": days }),
            true,
        )?;
        Ok(resp.item)
    }

    /// Reload rent port.
    pub fn port_reload(&self, tzid: i64) -> Result<bool> {
        let _: Value = self
            .http
            .get_onlinesim("rent/portReload", json!({ "tzid": tzid }), true)?;
        Ok(true)
    }

    /// All rent tariffs.
    pub fn tariffs(&self) -> Result<HashMap<String, RentTariff>> {
        self.http.get_onlinesim("rent/tariffsRent", json!({}), true)
    }

    /// Rent tariff for the default country.
    pub fn tariffs_one(&self) -> Result<RentTariff> {
        self.tariffs_one_in(DEFAULT_COUNTRY)
    }

    /// Rent tariff for an explicit country.
    pub fn tariffs_one_in(&self, country: i64) -> Result<RentTariff> {
        self.http
            .get_onlinesim("rent/tariffsRent", json!({ "country": country }), true)
    }

    /// Close a rent.
    pub fn close(&self, tzid: i64) -> Result<bool> {
        let _: Value =
            self.http
                .get_onlinesim("rent/closeRentNum", json!({ "tzid": tzid }), true)?;
        Ok(true)
    }
}
