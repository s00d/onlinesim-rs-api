//! Rented numbers: `client.rent()`.

use std::collections::HashMap;

use serde_json::Value;

use crate::error::{ApiError, Error, Result};
use crate::types::{RentItem, RentTariff};
use crate::wire::{Call, Request, decode};

/// Rented numbers. Get it with `client.rent()`. `time` of a rent is in minutes.
#[derive(Debug, Clone, Copy)]
pub struct Rent<'a, C>(pub(crate) &'a C);

section_methods!(Rent {
    /// Rent a number for `days` days. `extension: false` makes the rent non-extendable for good
    /// (`true` is the server default).
    fn get(country: u32, days: u32, extension: bool) -> RentItem;

    /// All rents of the account.
    fn state() -> Vec<RentItem>;

    /// One rent. `ApiError` `ERROR_NO_OPERATIONS` when it does not exist.
    fn state_one(tzid: i64) -> RentItem;

    /// Extend a rent by `days` days (only rents created with `extension: true`).
    fn extend(tzid: i64, days: u32) -> RentItem;

    /// Rent prices of every country, keyed by country code. No key needed.
    fn tariffs() -> HashMap<String, RentTariff>;

    /// Rent prices of one country. `ApiError` `NO_COUNTRY` for a country without rent. No key needed.
    fn tariffs_one(country: u32) -> RentTariff;

    /// Return the number before the rent period ends.
    fn close(tzid: i64) -> ();
});

/// Always sent: a flat `messages` list and `time_seconds`.
fn detail(request: Request) -> Request {
    request.arg("pagination", 0).arg("number", 1)
}

fn get(country: u32, days: u32, extension: bool) -> Call<RentItem> {
    Call::new(
        detail(
            Request::get("rent/getRentNum.php")
                .arg("country", country)
                .arg("days", days)
                .flag("extension", extension),
        ),
        |res| item(res?),
    )
}

fn state() -> Call<Vec<RentItem>> {
    Call::new(detail(Request::get("rent/getRentState.php")), |res| {
        list(res?)
    })
}

fn state_one(tzid: i64) -> Call<RentItem> {
    Call::new(
        detail(Request::get("rent/getRentState.php").arg("tzid", tzid)),
        |res| {
            list(res?)?
                .into_iter()
                .next()
                .ok_or_else(|| ApiError::new("ERROR_NO_OPERATIONS", None).into())
        },
    )
}

fn extend(tzid: i64, days: u32) -> Call<RentItem> {
    Call::new(
        detail(
            Request::get("rent/extendRentState.php")
                .arg("tzid", tzid)
                .arg("days", days),
        ),
        |res| item(res?),
    )
}

fn tariffs() -> Call<HashMap<String, RentTariff>> {
    Call::new(Request::get("rent/tariffsRent.php"), |res| match res? {
        all @ Value::Object(_) => decode(all),
        _ => Ok(HashMap::new()),
    })
}

fn tariffs_one(country: u32) -> Call<RentTariff> {
    Call::new(
        Request::get("rent/tariffsRent.php").arg("country", country),
        |res| match res? {
            // An unknown country answers `[]`.
            one @ Value::Object(_) if one.get("code").is_some() => decode(one),
            _ => Err(ApiError::new("NO_COUNTRY", None).into()),
        },
    )
}

fn close(tzid: i64) -> Call<()> {
    Call::new(
        Request::get("rent/closeRentNum.php").arg("tzid", tzid),
        |res| res.map(drop),
    )
}

fn item(mut value: Value) -> Result<RentItem> {
    match value.get_mut("item") {
        Some(item) => decode(item.take()),
        None => Err(Error::Decode("missing item".into())),
    }
}

fn list(mut value: Value) -> Result<Vec<RentItem>> {
    match value.get_mut("list") {
        Some(list @ Value::Array(_)) => decode(list.take()),
        _ => Ok(Vec::new()),
    }
}
