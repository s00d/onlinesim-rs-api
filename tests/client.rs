//! Client options, request encoding and the response function.

mod common;

use std::io::Read;
use std::net::TcpListener;
use std::time::Duration;

use common::{Fake, Reply, client, network_error, ok};
use onlinesim_rs_api::{Client, Error, Method};
use serde_json::json;

#[tokio::test]
async fn sends_the_key_as_bearer_header_never_in_the_query_plus_lang() {
    let (client, fake) = client(vec![ok(json!({ "response": "1", "price": 0.36 }))]);
    client.numbers().price("vkcom", 44).await.unwrap();
    let call = fake.last();
    assert_eq!(call.method, Method::Get);
    assert_eq!(call.header("Authorization"), Some("Bearer test-key"));
    assert_eq!(call.header("Accept"), Some("application/json"));
    assert!(
        call.header("User-Agent")
            .unwrap()
            .starts_with("onlinesim-rs-api/")
    );
    let pairs: Vec<_> = call.url.query_pairs().into_owned().collect();
    assert!(!pairs.iter().any(|(k, _)| k == "apikey" || k == "dev_id"));
    assert!(pairs.contains(&("lang".into(), "en".into())));
    assert_eq!(call.timeout, Duration::from_secs(30));
}

#[tokio::test]
async fn applies_lang_dev_id_timeout_and_a_base_url_without_trailing_slash() {
    let fake = Fake::new(vec![ok(json!({ "response": "1", "price": 1 }))]);
    let client = Client::builder()
        .api_key("k")
        .base_url("https://api.test/api")
        .lang("ru")
        .dev_id(42)
        .timeout(Duration::from_secs(5))
        .build_with(fake.clone())
        .unwrap();
    client.numbers().price("vkcom", 7).await.unwrap();
    let call = fake.last();
    assert_eq!(
        format!("{}{}", call.url.origin().ascii_serialization(), call.path()),
        "https://api.test/api/getPrice.php"
    );
    let pairs: Vec<_> = call.url.query_pairs().into_owned().collect();
    assert!(pairs.contains(&("lang".into(), "ru".into())));
    assert!(pairs.contains(&("dev_id".into(), "42".into())));
    assert_eq!(call.timeout, Duration::from_secs(5));
}

#[tokio::test]
async fn sends_no_authorization_header_without_a_key() {
    let fake = Fake::new(vec![ok(json!({ "response": "1", "price": 1 }))]);
    let client = Client::builder()
        .api_key("")
        .build_with(fake.clone())
        .unwrap();
    client.numbers().price("vkcom", 7).await.unwrap();
    let call = fake.last();
    assert_eq!(call.header("Authorization"), None);
    assert!(
        call.url
            .as_str()
            .starts_with("https://onlinesim.host/api/getPrice.php?")
    );
}

#[test]
fn rejects_a_bad_base_url() {
    let error = Client::builder()
        .base_url("not a url")
        .build_with(Fake::default())
        .unwrap_err();
    assert!(matches!(error, Error::Config(_)), "{error:?}");
}

#[tokio::test]
async fn maps_no_number_to_an_api_error_with_is_no_number() {
    let (client, _) = client(vec![
        ok(json!({ "response": "NO_NUMBER" })),
        ok(json!({ "response": "NO_NUMBER_FOR_FORWARD" })),
    ]);
    let error = client.numbers().get("google", 44, &[]).await.unwrap_err();
    assert!(error.is_no_number());
    let Error::Api(api) = &error else {
        panic!("{error:?}")
    };
    assert_eq!(api.code, "NO_NUMBER");
    assert_eq!(
        api.message,
        "No numbers available for this service right now"
    );
    let error = client.numbers().get("google", 7, &[]).await.unwrap_err();
    assert!(error.is_no_number());
}

#[tokio::test]
async fn maps_http_456_wrong_key_to_api_error_body_before_status() {
    let (client, _) = client(vec![Reply::Json(
        456,
        json!({ "response": "ERROR_WRONG_KEY" }),
    )]);
    let error = client.user().balance().await.unwrap_err();
    let Error::Api(api) = error else {
        panic!("{error:?}")
    };
    assert_eq!(api.code, "ERROR_WRONG_KEY");
    assert_eq!(api.message, "Wrong API key");
    assert!(!api.is_no_number());
}

#[tokio::test]
async fn prefers_the_server_message_but_not_when_it_only_echoes_the_code() {
    let (client, _) = client(vec![
        ok(json!({ "response": "TRY_AGAIN_LATER", "message": "Maintenance until 12:00" })),
        ok(json!({ "response": "UNDEFINED_COUNTRY", "message": "UNDEFINED_COUNTRY" })),
        ok(json!({ "response": "SOMETHING_NEW" })),
    ]);
    let messages = [
        ("TRY_AGAIN_LATER", "Maintenance until 12:00"),
        ("UNDEFINED_COUNTRY", "Unknown country"),
        ("SOMETHING_NEW", "SOMETHING_NEW"),
    ];
    for (code, message) in messages {
        let Error::Api(api) = client.numbers().get("x", 7, &[]).await.unwrap_err() else {
            panic!("not an ApiError")
        };
        assert_eq!((api.code.as_str(), api.message.as_str()), (code, message));
    }
}

#[tokio::test]
async fn maps_a_validation_object_to_validation_error_with_the_field_messages() {
    let (client, _) = client(vec![ok(json!({ "response": {
        "webhook_url": ["The webhook url must be a valid URL.", "Too long."]
    } }))]);
    let Error::Api(api) = client.user().set_webhook_url("nope").await.unwrap_err() else {
        panic!("not an ApiError")
    };
    assert_eq!(api.code, "VALIDATION_ERROR");
    assert_eq!(
        api.message,
        "The webhook url must be a valid URL.; Too long."
    );
}

#[tokio::test]
async fn treats_response_true_as_success() {
    let (client, _) = client(vec![ok(json!({ "response": true }))]);
    client.numbers().close(1).await.unwrap();
}

#[tokio::test]
async fn maps_a_non_json_body_to_transport_error_with_status_and_excerpt() {
    let (client, _) = client(vec![Reply::Raw(502, "<html>Bad gateway</html>")]);
    match client.user().balance().await.unwrap_err() {
        Error::Transport {
            status,
            body,
            message,
        } => {
            assert_eq!(status, Some(502));
            assert_eq!(body, "<html>Bad gateway</html>");
            assert_eq!(message, "HTTP 502: response is not JSON");
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn cuts_the_body_excerpt_at_500_chars() {
    let long: &'static str = Box::leak("x".repeat(600).into_boxed_str());
    let (client, _) = client(vec![Reply::Raw(500, long)]);
    let Error::Transport { body, .. } = client.user().balance().await.unwrap_err() else {
        panic!("not a TransportError")
    };
    assert_eq!(body.len(), 500);
}

#[tokio::test]
async fn maps_a_json_error_status_without_response_to_transport_error() {
    let (client, _) = client(vec![Reply::Json(500, json!({ "error": "boom" }))]);
    let error = client.user().balance().await.unwrap_err();
    assert!(
        matches!(
            error,
            Error::Transport {
                status: Some(500),
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn passes_a_network_failure_through_as_transport_error_without_status() {
    let (client, _) = client(vec![network_error("Network error: reset")]);
    let error = client.user().balance().await.unwrap_err();
    assert!(
        matches!(error, Error::Transport { status: None, .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn maps_a_decode_failure_to_decode_error() {
    let (client, _) = client(vec![ok(json!({ "response": 1, "item": { "no": "tzid" } }))]);
    let error = client.rent().get(7, 1, true).await.unwrap_err();
    assert!(matches!(error, Error::Decode(_)), "{error:?}");
}

#[tokio::test]
async fn returns_bodies_without_response_as_they_are() {
    let (client, _) = client(vec![ok(json!([]))]);
    assert!(
        client
            .numbers()
            .state(Default::default())
            .await
            .unwrap()
            .is_empty()
    );
}

/// A server that accepts the connection and never answers: the timeout is the only way out, so the
/// outcome cannot depend on timing.
fn silent_server() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/api/", listener.local_addr().unwrap());
    (listener, url)
}

#[tokio::test]
async fn reqwest_transport_maps_its_timeout_to_transport_error() {
    let (listener, url) = silent_server();
    // Keep accepted sockets open (and unread) until the test ends.
    let hold = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut buf = [0u8; 1];
        let _ = socket.read(&mut buf);
        std::thread::park();
    });
    let client = Client::builder()
        .base_url(url)
        .timeout(Duration::from_millis(50))
        .build()
        .unwrap();
    match client.numbers().tariffs_one(44).await.unwrap_err() {
        Error::Transport {
            status, message, ..
        } => {
            assert_eq!(status, None);
            assert_eq!(message, "Request timed out after 50ms");
        }
        other => panic!("{other:?}"),
    }
    drop(hold);
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_reqwest_transport_maps_its_timeout_to_transport_error() {
    let (listener, url) = silent_server();
    let _hold = std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        std::thread::park();
        drop(socket);
    });
    let client = onlinesim_rs_api::ClientBuilder::new()
        .base_url(url)
        .timeout(Duration::from_millis(50))
        .build_blocking()
        .unwrap();
    let error = client.numbers().tariffs_one(44).unwrap_err();
    assert!(
        matches!(&error, Error::Transport { status: None, message, .. } if message == "Request timed out after 50ms"),
        "{error:?}"
    );
}
