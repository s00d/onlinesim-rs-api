//! Unit tests for response parsing helpers.

use onlinesim_rs_api::Error;
use serde_json::json;

// Re-test via public Error mapping; parse helpers are crate-private.
// Integration coverage lives in tests/mock_api.rs.

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
