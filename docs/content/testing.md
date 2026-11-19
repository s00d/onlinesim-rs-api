---
title: Testing
description: Test your integration offline with MockOnlineSim — no network, key or money.
navigation:
  order: 6
---

`MockOnlineSim` is an in-process fake of the OnlineSim API that ships with the crate. It is a `Transport` and a `BlockingTransport` that answer like the real backend — same shapes, same error codes, HTTP 456 for a wrong key — and keep a balance, operations and rents. Your code talks to a normal `Client` (or `blocking::Client`), so nothing in it changes for tests. It is deterministic: no randomness, no clock; numbers and ids come from a counter.

## Setup

Enable the `mock-transport` feature for your tests. It adds no dependencies and works with the async client (default) and with `blocking`:

```toml
[dev-dependencies]
onlinesim-rs-api = { version = "0.2.1", features = ["mock-transport"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use onlinesim_rs_api::mock::{MockOnlineSim, MockOptions};

fn setup() -> onlinesim_rs_api::Result<()> {
    let mock = MockOnlineSim::new(MockOptions { balance: 10.0, ..MockOptions::default() });
    let client = mock.client()?; // a Client wired to the mock; `client_blocking()` for the blocking one
    Ok(())
}
```

```rust
pub struct MockOptions {
    pub api_key: String,          // default "mock-key"
    pub balance: f64,             // USD, default 100
    pub prices: MockPrices,       // BTreeMap<i64, BTreeMap<String, f64>>: country → service → USD
    pub rent_tariffs: BTreeMap<i64, MockRentTariff>, // country → { days: BTreeMap<i64, f64>, extend: bool }
    pub guard_interval: u64,      // seconds, default 0
}
```

| Field | Default | Description |
|---|---|---|
| `api_key` | `"mock-key"` | Key the mock accepts. `mock.client()` uses it. |
| `balance` | `100.0` | Starting balance, USD. |
| `prices` | `{7: {vkcom: 0.5, telegram: 1.2, google: 0.3}, 44: {google: 0.2, vkcom: 0.36}}` | Number prices; replaces the defaults. Also feeds `numbers().tariffs()`. |
| `rent_tariffs` | `{7: {days: {1: 1.0, 3: 2.5, 7: 5.0}, extend: true}}` | Rent prices; `extend: false` makes rents of that country non-extendable. |
| `guard_interval` | `0` | `> 0`: an operation without SMS cannot be closed (`NO_COMPLETE_TZID`). |

To build the client yourself (other options, a wrong key), pass `mock.transport()`; any base URL works:

```rust
use onlinesim_rs_api::Client;
use onlinesim_rs_api::mock::MockOnlineSim;

fn wrong_key() -> onlinesim_rs_api::Result<()> {
    let mock = MockOnlineSim::default();
    let client = Client::builder().api_key("wrong").build_with(mock.transport())?;
    Ok(())
}
```

`mock.builder()` returns a `ClientBuilder` with the mock's key and base URL already set.

## A full example

Order a number, let an SMS arrive while `wait_code` polls, check the code and the money. `add_sms_after` delivers the SMS after a number of polls instead of after some time, so `wait_code` runs with a zero interval and the test takes milliseconds.

```rust
use std::time::Duration;

use onlinesim_rs_api::WaitCodeOptions;
use onlinesim_rs_api::mock::{MockOnlineSim, MockOptions};
use onlinesim_rs_api::types::OperationStatus;

#[tokio::test]
async fn receives_the_code() {
    let mock = MockOnlineSim::new(MockOptions { balance: 10.0, ..MockOptions::default() });
    let client = mock.client().unwrap();

    let order = client.numbers().get_with_number("google", 44, &[]).await.unwrap();
    assert_eq!(order.number, "+44900000001");

    mock.add_sms_after(order.tzid, "Your Google code: 123-456", 2); // the third poll sees it
    let options = WaitCodeOptions { interval: Duration::ZERO, ..WaitCodeOptions::default() };
    let code = client.numbers().wait_code(order.tzid, options).await.unwrap();

    assert_eq!(code, "123456");
    assert_eq!(mock.operations()[0].status, OperationStatus::TzOverOk); // wait_code closed it
    assert_eq!(mock.balance(), 9.8); // the price is spent once an SMS arrived
}
```

The blocking client works the same way without any runtime: `mock.client_blocking()`, then `client.numbers().wait_code(tzid, options)` with `interval: Duration::ZERO`.

The `msg` of an operation is the code by default: the first run of 4–8 digits, with spaces and dashes between digits removed (`123-456` → `123456`; the rule is public as `mock::extract_code`); a text without such a run comes back whole. `message_to_code: false` (or `full_message` in `wait_code`) gives the full text.

## Errors

`fail_next(code)` makes the next request answer `{"response": code}` — the client turns it into `Error::Api` (`is_no_number()` for `NO_NUMBER`) exactly as for the real server. `fail_next_on(endpoint, code)` fails only that endpoint. `fail_next_transport(status)` makes the next request fail with a non-JSON body and that HTTP status (`Error::Transport`); `fail_next_transport_on(endpoint, status)` for one endpoint.

```rust
use onlinesim_rs_api::Error;
use onlinesim_rs_api::mock::MockOnlineSim;

#[tokio::test]
async fn handles_a_missing_number_and_a_network_hiccup() {
    let mock = MockOnlineSim::default();
    let client = mock.client().unwrap();

    mock.fail_next_on("getNum.php", "NO_NUMBER");
    assert!(client.numbers().get("google", 44, &[]).await.unwrap_err().is_no_number());

    mock.fail_next_transport(503);
    let error = client.user().balance().await.unwrap_err();
    assert!(matches!(error, Error::Transport { status: Some(503), .. }));

    assert_eq!(client.user().balance().await.unwrap().balance, 100.0); // one-shot
}
```

The state itself produces the real error codes too: `WARNING_LOW_BALANCE` when the balance is short, `NO_NUMBER` when `reject` matches the next number, `UNDEFINED_COUNTRY` / `ERROR_NO_SERVICE` for unknown prices, `ERROR_WRONG_TZID` for an unknown or closed operation, `TRY_AGAIN_LATER` when extending a non-extendable rent, `NO_COUNTRY` / `UNDEFINED_DAYS` for unknown rent tariffs, `ERROR_WRONG_KEY` (HTTP 456) for a wrong key.

## Inspecting requests

`mock.requests()` lists every request received: `MockRequest { method, path, query, body }`, `path` relative to `/api/`, `query` a `BTreeMap` (with `lang`), `body` the parsed JSON of a POST.

```rust
use onlinesim_rs_api::Method;
use onlinesim_rs_api::mock::MockOnlineSim;

#[tokio::test]
async fn sends_reject_as_a_comma_list() {
    let mock = MockOnlineSim::default();
    mock.client().unwrap().numbers().get("google", 44, &["4470", "4471"]).await.unwrap();

    let request = &mock.requests()[0];
    assert_eq!((request.method, request.path.as_str()), (Method::Get, "getNum.php"));
    assert_eq!(request.query["reject"], "4470,4471");
    assert_eq!(request.body, None);
}
```

## Control API

| Member | Description |
|---|---|
| `MockOnlineSim::new(options)` / `default()` | An in-process mock. |
| `client()` / `client_blocking()` | A normal `Client` / `blocking::Client` wired to the mock. |
| `transport()` | The `MockTransport` (`Transport` + `BlockingTransport`, cheap to clone). |
| `builder()` / `base_url()` | A `ClientBuilder` with the mock's key and base URL / that base URL. |
| `add_sms(tzid, text)` / `add_sms_from(tzid, text, service)` | An SMS arrives now at a number operation or a rent (detected by `tzid`). Panics for an unknown or closed one. |
| `add_sms_after(tzid, text, polls)` / `add_sms_after_from(…, service)` | The SMS arrives after `polls` state requests for `tzid` (`numbers().state`/`state_one`, `rent().state`/`state_one`). |
| `fail_next(code)` / `fail_next_on(endpoint, code)` | The next request (to `endpoint` only, e.g. `"getNum.php"`) answers `{"response": code}`. |
| `fail_next_transport(status)` / `fail_next_transport_on(endpoint, status)` | The next request fails with a non-JSON body and that HTTP status. |
| `set_balance(balance, zbalance)` / `balance()` | Set the total balance and an extra frozen part / read the total balance, USD (it includes the part frozen by open operations, as `user().balance()` reports it). |
| `operations()` / `rents()` | Copies of the current number operations / open rents, for assertions. |
| `requests()` | Copies of the requests received. |
| `webhook_url()` | The webhook URL last set through the profile. |
| `reset()` | Back to the constructor's state. |

`set_balance` takes two values because its 0.2.0 signature did; the other SDKs' `setBalance(n)` is `set_balance(n, 0.0)`. `script_sms` and `fail_no_number` from 0.2.0 still work; `add_sms_after` and `fail_next_on` replace them.

## Over HTTP (feature `mock`)

With the `mock` feature, `MockOnlineSim::start().await` (or `start_with(options)`) also serves the same state on a local port (wiremock, needs a tokio runtime), for code that builds its own HTTP client from a base URL. `base_url()` is then `http://127.0.0.1:<port>/api/`, and `client()` / `client_blocking()` go through real HTTP. All helpers above work the same.

## What the mock does

- **Numbers**: `get` freezes the price; `close` after an SMS spends it, `close` without one refunds it; `ban` after an SMS answers `"0"` (an `Error::Api` with code `0`), like the server. Numbers are `+<country>9000000NN`, ids count from 1. `next` switches to the following SMS. The operation's `time` stays at 1200 s — there is no clock.
- **Rent**: `get` charges at once and keeps the `extension` flag; `extend` charges again; `close` removes the rent (no refund). SMS appear in `messages`, newest first. Rent numbers have no country code.
- **User**: `balance` (frozen part included), a fixed profile, `set_webhook_url` validates `http(s)` URLs (`VALIDATION_ERROR` otherwise), `webhook_logs` lists the deliveries the mock *recorded* when an SMS arrived while a webhook URL was set — it makes no HTTP call. `payment_history` is empty.
- **Free**: `free().list` returns two fixed countries with one number and a few SMS each.
- Not imitated: the server's 5 s lock after `setOperationOk` (`INTERVAL_CONCURRENT_REQUESTS_ERROR`) and anything else that needs real time.
