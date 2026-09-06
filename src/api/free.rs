//! Free public numbers API.

use serde_json::json;

use crate::error::Result;
use crate::http::Http;
use crate::types::free::{
    CountriesWrap, FreeCountry, FreeListResponse, FreeMessage, FreeNumber, MessagesWrap,
    NumbersWrap,
};

/// Free / public SMS numbers.
#[derive(Debug, Clone)]
pub struct FreeApi {
    pub(crate) http: Http,
}

impl FreeApi {
    /// List free countries.
    pub async fn countries(&self) -> Result<Vec<FreeCountry>> {
        let resp: CountriesWrap = self
            .http
            .get_onlinesim("getFreeCountryList", json!({}), true)
            .await?;
        Ok(resp.countries)
    }

    /// Free numbers for a country.
    pub async fn numbers(&self, country: i64) -> Result<Vec<FreeNumber>> {
        let resp: NumbersWrap = self
            .http
            .get_onlinesim("getFreePhoneList", json!({ "country": country }), true)
            .await?;
        Ok(resp.numbers)
    }

    /// Messages for a free phone (page defaults to `1`).
    pub async fn messages(&self, phone: i64) -> Result<Vec<FreeMessage>> {
        self.messages_page(phone, 1).await
    }

    /// Messages for a free phone with explicit page.
    pub async fn messages_page(&self, phone: i64, page: i64) -> Result<Vec<FreeMessage>> {
        let resp: MessagesWrap = self
            .http
            .get_onlinesim(
                "getFreeMessageList",
                json!({ "phone": phone, "page": page }),
                true,
            )
            .await?;
        Ok(resp.messages.data)
    }

    /// Combined free list.
    pub async fn free_list(&self) -> Result<FreeListResponse> {
        self.http
            .get_onlinesim("getFreeList", json!({}), true)
            .await
    }
}
