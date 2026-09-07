//! Pure unit tests (no HTTP). Integration coverage lives in `api_mock.rs`
//! and uses the published [`onlinesim_rs_api::mock`] module.

use onlinesim_rs_api::Error;
use serde_json::json;

#[test]
fn no_number_codes() {
    assert!(matches!(
        Error::from_api_code("NO_NUMBER"),
        Error::NoNumber { .. }
    ));
    assert!(matches!(
        Error::from_api_code("NO_NUMBER_FOR_FORWARD"),
        Error::NoNumber { .. }
    ));
}

#[test]
fn request_code_message() {
    let err = Error::from_api_code("ERROR_NO_KEY");
    let text = err.to_string();
    assert!(text.contains("no apikey"));
    match err {
        Error::Request { code, message } => {
            assert_eq!(code, "ERROR_NO_KEY");
            assert_eq!(message, "no apikey");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn json_roundtrip_balance_shape() {
    let v = json!({
        "balance": 1.5,
        "zbalance": 0.0,
        "income": 2.0
    });
    let b: onlinesim_rs_api::types::user::Balance = serde_json::from_value(v).unwrap();
    assert_eq!(b.balance, 1.5);
}

#[test]
fn balance_accepts_string_decimals_like_live_api() {
    // Api2Controller@getBalance: balance via Payment accessor = raw DECIMAL string.
    let v = json!({
        "balance": "1674.540",
        "zbalance": 0,
        "income": "25.00",
        "income_usd": "12.5"
    });
    let b: onlinesim_rs_api::types::user::Balance = serde_json::from_value(v).unwrap();
    assert!((b.balance - 1674.54).abs() < 1e-9);
    assert_eq!(b.zbalance, 0.0);
    assert_eq!(b.income, Some(25.0));
    assert_eq!(b.income_usd, Some(12.5));
}

#[test]
fn tariff_price_accepts_number_format_string() {
    let v = json!({
        "count": 3,
        "popular": true,
        "price": "1.50",
        "id": "telegram",
        "service": "telegram",
        "slug": "telegram"
    });
    let s: onlinesim_rs_api::types::numbers::TariffService = serde_json::from_value(v).unwrap();
    assert!((s.price - 1.5).abs() < 1e-9);
}

#[test]
fn tariff_country_services_accepts_empty_array_like_live_api() {
    // getNumbersStats: some countries (e.g. 356) return `"services": []`
    // because PHP encodes an empty assoc array as a JSON array.
    let v = json!({
        "name": "Malta",
        "position": 99,
        "code": 356,
        "other": false,
        "new": false,
        "enabled": true,
        "locale_name": "Malta",
        "services": []
    });
    let c: onlinesim_rs_api::types::numbers::TariffCountryOne = serde_json::from_value(v).unwrap();
    assert!(c.services.is_empty());
    assert_eq!(c.code, 356);
    assert_eq!(c.other, Some(false));
}

#[test]
fn tariff_service_accepts_live_row_with_code() {
    let v = json!({
        "count": 8977,
        "popular": false,
        "code": 49,
        "price": 1,
        "id": 1,
        "service": "VKontakte + Mail.ru",
        "slug": "vkcom"
    });
    let s: onlinesim_rs_api::types::numbers::TariffService = serde_json::from_value(v).unwrap();
    assert_eq!(s.count, 8977);
    assert_eq!(s.code, Some(49));
    assert!((s.price - 1.0).abs() < 1e-9);
}

#[test]
fn rent_days_map_accepts_string_prices() {
    let v = json!({
        "code": 7,
        "enabled": true,
        "name": "Russia",
        "new": false,
        "position": 1,
        "days": { "7": "10.00", "14": 18.5 }
    });
    let t: onlinesim_rs_api::types::rent::RentTariff = serde_json::from_value(v).unwrap();
    assert!((t.days["7"] - 10.0).abs() < 1e-9);
    assert!((t.days["14"] - 18.5).abs() < 1e-9);
}

#[test]
fn rent_item_extend_accepts_empty_array_or_price_object() {
    let empty = json!({
        "tzid": 1,
        "extend": [],
        "day_extend": "40.50",
        "sum": "12.00"
    });
    let item: onlinesim_rs_api::types::rent::RentItem = serde_json::from_value(empty).unwrap();
    assert!(item.extend.is_empty());
    assert_eq!(item.day_extend, Some(40.5));
    assert_eq!(item.sum, Some(12.0));

    let priced = json!({
        "tzid": 2,
        "extend": { "1": "40.00", "7": 200 },
        "day_extend": 40
    });
    let item: onlinesim_rs_api::types::rent::RentItem = serde_json::from_value(priced).unwrap();
    assert!((item.extend["1"] - 40.0).abs() < 1e-9);
    assert!((item.extend["7"] - 200.0).abs() < 1e-9);
}

#[test]
fn free_number_accepts_integer_number() {
    let v = json!({
        "number": 79001234567_i64,
        "country": 7
    });
    let n: onlinesim_rs_api::types::free::FreeNumber = serde_json::from_value(v).unwrap();
    assert_eq!(n.number, "79001234567");
}

#[test]
fn free_message_my_number_accepts_string_or_int() {
    let as_str = json!({ "text": "hi", "my_number": "7900" });
    let m: onlinesim_rs_api::types::free::FreeMessage = serde_json::from_value(as_str).unwrap();
    assert_eq!(m.my_number.as_deref(), Some("7900"));

    let as_int = json!({ "text": "hi", "my_number": 7900 });
    let m: onlinesim_rs_api::types::free::FreeMessage = serde_json::from_value(as_int).unwrap();
    assert_eq!(m.my_number.as_deref(), Some("7900"));
}

#[test]
fn user_number_reject_accepts_object_or_array() {
    let obj = json!({ "id": 1, "number_reject": {} });
    let u: onlinesim_rs_api::types::user::User = serde_json::from_value(obj).unwrap();
    assert_eq!(u.number_reject, Some(json!({})));

    let arr = json!({ "id": 1, "number_reject": [] });
    let u: onlinesim_rs_api::types::user::User = serde_json::from_value(arr).unwrap();
    assert_eq!(u.number_reject, Some(json!([])));
}
