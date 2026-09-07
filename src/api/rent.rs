//! Number rent API.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::DEFAULT_COUNTRY;
use crate::error::{Error, Result};
use crate::http::Http;
use crate::types::rent::{RentItem, RentTariff};

/// Long-term number rent operations.
#[derive(Debug, Clone)]
pub struct RentApi {
    pub(crate) http: Http,
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
    /// Rent a number (defaults: country [`DEFAULT_COUNTRY`], days `1`, extension `true`).
    pub async fn get(&self) -> Result<RentItem> {
        self.get_with(DEFAULT_COUNTRY, 1, true).await
    }

    /// Rent a number with explicit parameters.
    pub async fn get_with(&self, country: i64, days: i64, extension: bool) -> Result<RentItem> {
        let resp: ItemWrap = self
            .http
            .get_onlinesim(
                "rent/getRentNum",
                json!({
                    "country": country,
                    "days": days,
                    "extension": extension,
                    "pagination": false,
                }),
                true,
            )
            .await?;
        Ok(resp.item)
    }

    /// List active rents.
    pub async fn state(&self) -> Result<Vec<RentItem>> {
        let resp: ListWrap = self
            .http
            .get_onlinesim("rent/getRentState", json!({ "pagination": false }), true)
            .await?;
        Ok(resp.list)
    }

    /// State for one rent `tzid`.
    ///
    /// # Warning
    ///
    /// Prefer [`Self::state`] and filter locally: one request for all active
    /// rents is cheaper and less likely to hit rate limits than polling each
    /// `tzid` separately.
    pub async fn state_one(&self, tzid: i64) -> Result<RentItem> {
        let resp: ListWrap = self
            .http
            .get_onlinesim(
                "rent/getRentState",
                json!({ "tzid": tzid, "pagination": false }),
                true,
            )
            .await?;
        resp.list
            .into_iter()
            .next()
            .ok_or_else(|| Error::Unexpected("empty rent state".into()))
    }

    /// Extend a rent (default `days = 1`).
    pub async fn extend(&self, tzid: i64) -> Result<RentItem> {
        self.extend_days(tzid, 1).await
    }

    /// Extend a rent by an explicit number of days.
    pub async fn extend_days(&self, tzid: i64, days: i64) -> Result<RentItem> {
        let resp: ItemWrap = self
            .http
            .get_onlinesim(
                "rent/extendRentState",
                json!({ "tzid": tzid, "days": days }),
                true,
            )
            .await?;
        Ok(resp.item)
    }

    /// Reload rent port.
    pub async fn port_reload(&self, tzid: i64) -> Result<bool> {
        let _: Value = self
            .http
            .get_onlinesim("rent/portReload", json!({ "tzid": tzid }), true)
            .await?;
        Ok(true)
    }

    /// All rent tariffs.
    pub async fn tariffs(&self) -> Result<HashMap<String, RentTariff>> {
        self.http
            .get_onlinesim("rent/tariffsRent", json!({}), true)
            .await
    }

    /// Rent tariff for the default country.
    pub async fn tariffs_one(&self) -> Result<RentTariff> {
        self.tariffs_one_in(DEFAULT_COUNTRY).await
    }

    /// Rent tariff for an explicit country.
    pub async fn tariffs_one_in(&self, country: i64) -> Result<RentTariff> {
        self.http
            .get_onlinesim("rent/tariffsRent", json!({ "country": country }), true)
            .await
    }

    /// Close a rent.
    pub async fn close(&self, tzid: i64) -> Result<bool> {
        let _: Value = self
            .http
            .get_onlinesim("rent/closeRentNum", json!({ "tzid": tzid }), true)
            .await?;
        Ok(true)
    }
}
