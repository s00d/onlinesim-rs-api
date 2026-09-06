[![Crates.io](https://img.shields.io/crates/v/onlinesim-rs-api?style=for-the-badge)](https://crates.io/crates/onlinesim-rs-api)
[![Documentation](https://img.shields.io/badge/docs-docs.rs-blue?style=for-the-badge)](https://docs.rs/onlinesim-rs-api)
[![GitHub issues](https://img.shields.io/github/issues/s00d/onlinesim-rs-api?style=for-the-badge)](https://github.com/s00d/onlinesim-rs-api/issues)
[![GitHub stars](https://img.shields.io/github/stars/s00d/onlinesim-rs-api?style=for-the-badge)](https://github.com/s00d/onlinesim-rs-api/stargazers)
[![Donate](https://img.shields.io/badge/Donate-Donationalerts-ff4081?style=for-the-badge)](https://www.donationalerts.com/r/s00d88)

# onlinesim-rs-api

Rust client for [OnlineSim](https://onlinesim.io) SMS API with full serde typing, async and blocking clients, and a built-in mock server for tests.

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `async` | yes | Async `Client` (`reqwest` + `tokio`) |
| `blocking` | no | Sync `blocking::Client` |
| `mock` | no | Built-in `MockOnlineSim` HTTP mock for tests / local integration |

## Install

```toml
[dependencies]
onlinesim-rs-api = "0.1"

[dev-dependencies]
onlinesim-rs-api = { version = "0.1", features = ["mock"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Quick start (async)

```rust
use onlinesim_rs_api::Client;

#[tokio::main]
async fn main() -> onlinesim_rs_api::Result<()> {
    let client = Client::new("your-apikey")?;
    let balance = client.user().balance().await?;
    println!("balance = {}", balance.balance);

    let tzid = client.numbers().get("telegram").await?;
    println!("tzid = {tzid}");
    Ok(())
}
```

## Built-in mocks (no real numbers)

```rust
use onlinesim_rs_api::mock::{MockOnlineSim, SmsScript};
use onlinesim_rs_api::WaitCodeOptions;

#[tokio::main]
async fn main() -> onlinesim_rs_api::Result<()> {
    let mock = MockOnlineSim::start().await;
    mock.script_sms(SmsScript {
        code: "123456".into(),
        polls_before_code: 1,
        ..SmsScript::default()
    });

    let client = mock.client()?;
    let ordered = client.numbers().get_with_number("telegram").await?;
    let code = client
        .numbers()
        .wait_code(
            ordered.tzid,
            WaitCodeOptions {
                interval_secs: 0, // no sleep in tests
                ..WaitCodeOptions::default()
            },
        )
        .await?;
    assert_eq!(code, "123456");
    Ok(())
}
```

See module docs: [`onlinesim_rs_api::mock`](https://docs.rs/onlinesim-rs-api/latest/onlinesim_rs_api/mock/) and example:

```bash
cargo run --example mock_sms_flow --features mock
```

### Mock helpers

| Method | Purpose |
|--------|---------|
| `script_sms(SmsScript)` | Queue number + SMS code delivery |
| `fail_no_number(service)` | Force `NO_NUMBER` on `getNum` |
| `set_balance(...)` | Control `getBalance` |
| `client()` | Preconfigured `Client` pointed at the mock |

Covered endpoints include: `getBalance`, `getProfile`, `getNum`, `getState`, `setOperationOk` / `Revise`, `getPrice`, `getNumbersStats`, free-list endpoints, rent get/state/close/tariffs.

## API modules

| Method | Module |
|--------|--------|
| `client.numbers()` | Temporary SMS numbers |
| `client.rent()` | Long-term number rent |
| `client.user()` | Balance / profile / payments |
| `client.free()` | Public free numbers |

## Examples

```bash
ONLINESIM_APIKEY=... cargo run --example get_balance
ONLINESIM_APIKEY=... cargo run --example wait_sms
cargo run --example mock_sms_flow --features mock
```

## License

Apache License 2.0
