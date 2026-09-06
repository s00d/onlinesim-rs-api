//! Blocking free numbers API.

use serde_json::json;

use crate::error::Result;
use crate::types::free::{
    CountriesWrap, FreeCountry, FreeListResponse, FreeMessage, FreeNumber, MessagesWrap,
    NumbersWrap,
};

use super::http::BlockingHttp;

/// Free public numbers (blocking).
#[derive(Debug, Clone)]
pub struct FreeApi {
    pub(crate) http: BlockingHttp,
}

impl FreeApi {
    /// List free countries.
    pub fn countries(&self) -> Result<Vec<FreeCountry>> {
        let resp: CountriesWrap = self
            .http
            .get_onlinesim("getFreeCountryList", json!({}), true)?;
        Ok(resp.countries)
    }

    /// Free numbers for a country.
    pub fn numbers(&self, country: i64) -> Result<Vec<FreeNumber>> {
        let resp: NumbersWrap =
            self.http
                .get_onlinesim("getFreePhoneList", json!({ "country": country }), true)?;
        Ok(resp.numbers)
    }

    /// Messages for a free phone (page defaults to `1`).
    pub fn messages(&self, phone: i64) -> Result<Vec<FreeMessage>> {
        self.messages_page(phone, 1)
    }

    /// Messages for a free phone with explicit page.
    pub fn messages_page(&self, phone: i64, page: i64) -> Result<Vec<FreeMessage>> {
        let resp: MessagesWrap = self.http.get_onlinesim(
            "getFreeMessageList",
            json!({ "phone": phone, "page": page }),
            true,
        )?;
        Ok(resp.messages.data)
    }

    /// Combined free list.
    pub fn free_list(&self) -> Result<FreeListResponse> {
        self.http.get_onlinesim("getFreeList", json!({}), true)
    }
}
