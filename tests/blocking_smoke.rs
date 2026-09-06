//! Blocking client smoke test.

#![cfg(feature = "blocking")]

use onlinesim_rs_api::blocking::Client;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

fn spawn_json_server(status_line: &str, body: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let status = status_line.to_string();
    let body = body.to_string();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        let response = format!(
            "{status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    });
    format!("http://{addr}")
}

#[test]
fn blocking_balance() {
    let body = json!({
        "response": "1",
        "balance": 10.0,
        "zbalance": 0.0,
        "income": 1.0
    })
    .to_string();
    let base = spawn_json_server("HTTP/1.1 200 OK", &body);
    let client = Client::builder()
        .apikey("k")
        .base_url(format!("{base}/api/"))
        .build_blocking()
        .unwrap();
    // Note: path will be {base}/api/getBalance.php — our tiny server ignores path.
    let balance = client.user().balance().unwrap();
    assert_eq!(balance.balance, 10.0);
}
