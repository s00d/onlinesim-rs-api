---
title: Numbers
description: Order a number, read its SMS, close it — client.numbers().
navigation:
  order: 1
---

An operation (`tzid`) is one ordered number. Order it with `get`/`get_with_number`, read the SMS with `state_one` or `wait_code`, finish it with `close` (or `ban`). Countries are phone codes (`7`, `44`, …), services are slugs (`google`, `vkcom`, …) as returned by `tariffs`.

Signatures below are the async ones; the blocking client has the same methods without `async`.

## price

Price of one number, USD; `None` when the country has no price for the service.

```rust
async fn price(&self, service: &str, country: u32) -> Result<Option<f64>>
```

| Param | Type | Description |
|---|---|---|
| `service` | `&str` | Service slug. |
| `country` | `u32` | Country code. |

```rust
let usd = client.numbers().price("google", 44).await?;
```

Errors: `ApiError` `ERROR_NO_SERVICE`, `UNDEFINED_COUNTRY`.

## get

Order a number; returns the operation id. Freezes the price on the balance.

```rust
async fn get(&self, service: &str, country: u32, reject: &[&str]) -> Result<i64>
```

| Param | Type | Description |
|---|---|---|
| `service` | `&str` | Service slug. |
| `country` | `u32` | Country code. |
| `reject` | `&[&str]` | Number prefixes to skip, e.g. `&["4470"]`; `&[]` uses the profile list. |

```rust
let tzid = client.numbers().get("google", 44, &["4470"]).await?;
```

Errors: `ApiError` with `is_no_number()`; `ApiError` `WARNING_LOW_BALANCE`, `ERROR_NO_SERVICE`, `UNDEFINED_COUNTRY`, `EXCEEDED_CONCURRENT_OPERATIONS`, `INTERVAL_CONCURRENT_REQUESTS_ERROR`, `TRY_AGAIN_LATER`.

## get_with_number

Same as `get`, but returns the number right away.

```rust
async fn get_with_number(&self, service: &str, country: u32, reject: &[&str]) -> Result<NumberOrder>
```

`NumberOrder`: `{ tzid, number, country, time, service, title, guard_interval_remaining_seconds }` — `number` with `+`, `time` in seconds left.

```rust
let order = client.numbers().get_with_number("google", 44, &[]).await?;
println!("{} {}", order.tzid, order.number);
```

Errors: as `get`.

## state

All operations of the account; empty when there are none.

```rust
async fn state(&self, options: StateOptions) -> Result<Vec<Operation>>
```

| `StateOptions` field | Type | Default | Description |
|---|---|---|---|
| `message_to_code` | `bool` | `true` | `msg` is the code only; `false` — the full SMS text. |
| `order_by` | `OrderBy` | `Asc` | Order of operations (`Asc`, `Desc`). |
| `msg_list` | `bool` | `false` | `msg` is a list of all SMS (`OperationMsg::List`). |

`Operation`: `{ tzid, response, number, service, country, sum, time, msg, guard_interval_remaining_seconds, … }`.
`response` is the state (`Option<OperationStatus>`, `None` when no number could be issued): `TzInpool` (number being assigned), `TzNumWait` (waiting for SMS), `TzNumAnswer` (SMS arrived), `TzOverOk` (finished), `ErrorNoItems` (failed). `msg` is `None` until the first SMS; `msg.as_text()` gives the code. The server caches answers for a few seconds.

```rust
use onlinesim_rs_api::{OrderBy, StateOptions};

let options = StateOptions { order_by: OrderBy::Desc, ..StateOptions::default() };
for op in client.numbers().state(options).await? {
    println!("{} {:?} {:?}", op.tzid, op.response, op.msg);
}
```

## state_one

One operation.

```rust
async fn state_one(&self, tzid: i64, options: StateOneOptions) -> Result<Operation>
```

`StateOneOptions`: `message_to_code` (default `true`), `msg_list` (default `false`).

```rust
use onlinesim_rs_api::types::OperationStatus;

let op = client.numbers().state_one(tzid, Default::default()).await?;
if op.response == Some(OperationStatus::TzNumAnswer) {
    println!("{:?}", op.msg.as_ref().and_then(|m| m.as_text()));
}
```

Errors: `ApiError` `ERROR_NO_OPERATIONS` when the operation does not exist (any more).

## wait_code

Poll the operation until a code arrives, then close it and return the code.

```rust
async fn wait_code(&self, tzid: i64, options: WaitCodeOptions) -> Result<String>
```

| `WaitCodeOptions` field | Type | Default | Description |
|---|---|---|---|
| `interval` | `Duration` | 5 s | Poll interval. |
| `timeout` | `Duration` | 600 s | Give up after. |
| `keep_open` | `bool` | `false` | Call `next(tzid)` instead of `close(tzid)`, to wait for another SMS. |
| `full_message` | `bool` | `false` | Return the full SMS text instead of the code. |
| `since` | `Option<String>` | `None` | Ignore this value (the previous code). |

Transient errors are retried until the timeout: `Error::Transport`, and `ApiError` `TRY_AGAIN_LATER` (the HTTP 429 rate limit), `ERROR_UNDEFINED` and `INTERVAL_CONCURRENT_REQUESTS_ERROR`. Any other `ApiError` is returned at once. The async version sleeps on tokio's clock (pause it in tests: `#[tokio::test(start_paused = true)]`); the blocking `wait_code_with(tzid, options, &clock)` takes any `Clock`.

```rust
use onlinesim_rs_api::WaitCodeOptions;

let open = WaitCodeOptions { keep_open: true, ..WaitCodeOptions::default() };
let first = client.numbers().wait_code(tzid, open).await?;
let next = WaitCodeOptions { since: Some(first), ..WaitCodeOptions::default() };
let second = client.numbers().wait_code(tzid, next).await?;
```

Errors: `Error::Timeout`; `ApiError` with code `TZ_OVER_OK` or `ERROR_NO_ITEMS` when the operation ended without a new code; any other `ApiError` from `state_one`/`close`/`next`.

## next

Switch the operation to its next SMS and keep it open (for services that send several SMS).

```rust
async fn next(&self, tzid: i64) -> Result<()>
```

```rust
client.numbers().next(tzid).await?;
```

Errors: `ApiError` `ERROR_WRONG_TZID` (not an operation of your account), `ERROR_NO_TZID`.

## close

Finish the operation. An operation without SMS can be closed only after its guard interval (`guard_interval_remaining_seconds`); every attempt locks the tzid for 5 s.

```rust
async fn close(&self, tzid: i64) -> Result<()>
```

```rust
client.numbers().close(tzid).await?;
```

Errors: `ApiError` `NO_COMPLETE_TZID`, `INTERVAL_CONCURRENT_REQUESTS_ERROR`, `ERROR_WRONG_TZID`.

## ban

Finish the operation and report the number as bad. Only before any SMS arrived.

```rust
async fn ban(&self, tzid: i64) -> Result<()>
```

```rust
client.numbers().ban(tzid).await?;
```

Errors: as `close`; `ApiError` with code `"0"` when the operation already has an SMS — nothing is changed then.

## tariffs

Services, prices and stock of every country, keyed by country code. No API key needed. The answer is large (a few MB); prefer `tariffs_one`.

```rust
async fn tariffs(&self) -> Result<HashMap<String, CountryStat>>
```

`CountryStat`: `{ name, locale_name, position, code, other, is_new, enabled, services }`; `services` maps `service_<slug>` to `ServiceStat { count, popular, code, price, id, service, slug }` (`count` is `None` when the stock is unknown).

```rust
let all = client.numbers().tariffs().await?;
println!("{} countries", all.len());
```

## tariffs_one

Services, prices and stock of one country.

```rust
async fn tariffs_one(&self, country: u32) -> Result<CountryStat>
```

```rust
let uk = client.numbers().tariffs_one(44).await?;
for s in uk.services.values() {
    println!("{} {} {:?}", s.slug, s.price, s.count);
}
```

Errors: `ApiError` `NO_COUNTRY`.
