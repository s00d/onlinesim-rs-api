---
title: onlinesim-rs-api
description: Typed async and blocking Rust client for the OnlineSim API — SMS numbers, rent, account and free numbers.
---

## Install

Rust 1.85 or newer. The async client runs on `reqwest` + `tokio`; the blocking one is behind a feature.

```toml
[dependencies]
onlinesim-rs-api = "0.2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

| Feature | Default | What |
|---|---|---|
| `async` | yes | `Client`: `async` methods. |
| `blocking` | no | `blocking::Client`: the same methods without `.await`. |
| `mock-transport` | no | `mock::MockOnlineSim`: an in-process fake OnlineSim API for your own tests, no extra dependencies ([Testing](/testing)). |
| `mock` | no | `mock-transport` plus `MockOnlineSim::start()`, the same fake over local HTTP (wiremock). |

Without features the crate has only the types, the errors and [`parse_webhook`](/webhooks) — enough for a webhook receiver.

## Quick start

Check the balance, order a number, wait for the SMS code.

```rust
use onlinesim_rs_api::{Client, WaitCodeOptions};

#[tokio::main]
async fn main() -> onlinesim_rs_api::Result<()> {
    let client = Client::new(std::env::var("ONLINESIM_KEY").unwrap_or_default())?;

    let balance = client.user().balance().await?;
    println!("available: {} USD", balance.balance - balance.zbalance);

    let order = client.numbers().get_with_number("google", 44, &[]).await?;
    println!("enter {} in the app", order.number);

    // Polls every 5 s for up to 10 min, then closes the operation.
    let code = client.numbers().wait_code(order.tzid, WaitCodeOptions::default()).await?;
    println!("code: {code}");
    Ok(())
}
```

Blocking: `onlinesim_rs_api::blocking::Client::new(key)?` and the same calls without `.await` (feature `blocking`).

## Client options

```rust
use std::time::Duration;
use onlinesim_rs_api::Client;

let client = Client::builder()
    .api_key("your-api-key")
    .lang("en")
    .timeout(Duration::from_secs(15))
    .build()?;
```

| Builder method | Type | Default | Description |
|---|---|---|---|
| `api_key` | `impl Into<String>` | — | Sent as `Authorization: Bearer <key>`, never in the URL. Not needed for tariffs and free numbers. |
| `base_url` | `impl Into<String>` | `https://onlinesim.host/api/` | API base; trailing slash optional. |
| `lang` | `impl Into<String>` | `en` | Language of server texts (`lang` query parameter). |
| `dev_id` | `impl Display` | — | Partner developer id (`dev_id`), sent only when set. |
| `timeout` | `Duration` | 30 s | Per-request timeout, also applied to an injected `reqwest` client. |

Finish with `build()` (async, default `reqwest::Client`), `build_with(transport)` (your `reqwest::Client` — proxy, TLS — or any `Transport`, e.g. a fake in tests), `build_blocking()` or `build_blocking_with(transport)`. Clients are cheap to clone.

Sections: [`client.numbers()`](/numbers), [`client.rent()`](/rent), [`client.user()`](/user), [`client.free()`](/free). Webhook bodies are parsed by the standalone [`parse_webhook`](/webhooks).

Returned structs keep the server's snake_case field names. Money that the server sends as a decimal string (`balance`, rent `sum`) becomes `f64`, numbers sent as strings are parsed, and maps that arrive as an empty PHP array `[]` become empty maps.

## Error handling

Every call returns `onlinesim_rs_api::Result<T>`; the error is the enum `Error`.

| Variant | When | Data |
|---|---|---|
| `Error::Api(ApiError)` | The API answered with an error code (also a wrong key, HTTP 456). `is_no_number()` for `NO_NUMBER` / `NO_NUMBER_FOR_FORWARD`. | `code`, `message` |
| `Error::Transport` | Network error, request timeout, an HTTP error without an API code, or a body that is not JSON. | `message`, `status` (`None` without a response), `body` (first 500 chars) |
| `Error::Timeout` | `wait_code` got no code within its `timeout`. | message |
| `Error::Decode` | The JSON does not have the expected shape. | message |
| `Error::Config` | The client could not be built (bad base URL). | message |

```rust
use onlinesim_rs_api::Error;

match client.numbers().get("google", 44, &[]).await {
    Ok(tzid) => println!("ordered {tzid}"),
    Err(e) if e.is_no_number() => println!("no numbers now, try another country"),
    Err(Error::Api(e)) => eprintln!("{} {}", e.code, e.message), // e.g. WARNING_LOW_BALANCE
    Err(Error::Transport { status, body, .. }) => eprintln!("network: {status:?} {body}"),
    Err(e) => return Err(e),
}
```

`message` is the server's text when it sends one, otherwise a short English description of the code (`error_message(code)`), otherwise the code itself. Profile validation failures become `ApiError` with code `VALIDATION_ERROR`. `Error::code()` gives the API code of any error.
