//! Mock HTTP tests (legacy wiremock) + built-in MockOnlineSim coverage.

#![cfg(feature = "async")]

use onlinesim_rs_api::Client;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn client_for(server: &MockServer) -> Client {
    Client::builder()
        .apikey("test-key")
        .lang("en")
        .base_url(format!("{}/api/", server.uri()))
        .build()
        .expect("client")
}

#[tokio::test]
async fn balance_parses_success_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/getBalance.php"))
        .and(query_param("apikey", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "1",
            "balance": 100.5,
            "zbalance": 1.0,
            "income": 50.0
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let balance = client.user().balance().await.unwrap();
    assert_eq!(balance.balance, 100.5);
    assert_eq!(balance.zbalance, 1.0);
    assert_eq!(balance.income, Some(50.0));
}

#[tokio::test]
async fn no_number_maps_to_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/getNum.php"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "NO_NUMBER"
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let err = client.numbers().get("telegram").await.unwrap_err();
    assert!(matches!(
        err,
        onlinesim_rs_api::Error::NoNumber { code } if code == "NO_NUMBER"
    ));
}

#[tokio::test]
async fn get_with_number_returns_tzid_and_number() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/getNum.php"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": 1,
            "tzid": 42,
            "number": "79001234567",
            "country": 7,
            "time": 900,
            "service": "telegram",
            "title": "Telegram"
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let got = client.numbers().get_with_number("telegram").await.unwrap();
    assert_eq!(got.tzid, 42);
    assert_eq!(got.number, "79001234567");
    assert_eq!(got.country, 7);
    assert_eq!(got.service.as_deref(), Some("telegram"));
}

#[tokio::test]
async fn free_countries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/getFreeCountryList.php"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "1",
            "countries": [
                { "country": 7, "country_text": "Russia" }
            ]
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let countries = client.free().countries().await.unwrap();
    assert_eq!(countries.len(), 1);
    assert_eq!(countries[0].country, 7);
}

#[tokio::test]
async fn rent_get_unwraps_item() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/rent/getRentNum.php"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "1",
            "item": {
                "tzid": 9,
                "number": "79001112233",
                "status": 1,
                "messages": []
            }
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let item = client.rent().get().await.unwrap();
    assert_eq!(item.tzid, 9);
    assert_eq!(item.number.as_deref(), Some("79001112233"));
}

#[tokio::test]
async fn request_error_includes_message() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/getBalance.php"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "ERROR_WRONG_KEY"
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let err = client.user().balance().await.unwrap_err();
    match err {
        onlinesim_rs_api::Error::Request { code, message } => {
            assert_eq!(code, "ERROR_WRONG_KEY");
            assert_eq!(message, "wrong apikey");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn get_state_array_without_top_level_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/getState.php"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "tzid": 1,
                "response": "TZ_NUM_WAIT",
                "number": "+79001112233",
                "service": "telegram",
                "time": 100,
                "country": 7,
                "sum": 10.0
            }
        ])))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let state = client.numbers().state_one(1).await.unwrap();
    assert_eq!(state.tzid, 1);
    assert_eq!(state.response.as_deref(), Some("TZ_NUM_WAIT"));
    assert_eq!(state.sum, Some(10.0));
}
