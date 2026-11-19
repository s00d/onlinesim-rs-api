---
title: Rent
description: Rent a number for days and receive every SMS sent to it — client.rent().
navigation:
  order: 2
---

A rent keeps a number for `days` days. Its SMS come in `RentItem.messages`, newest first.

`RentItem`: `{ tzid, status, number, country, sum, days, hours, time, time_seconds, extension, extend, messages, start_time, stop_time, day_extend, … }` —
`sum` is the price paid in USD (`f64`; the server sends a decimal string), `time` minutes left, `time_seconds` seconds left,
`extension` `0` not extendable / `1` extendable / `2` auto-extension, `extend` maps days to the extension price (empty when not extendable),
`messages` is `Vec<RentSms>`: `{ id, service, text, code, created_at }`.

## get

Rent a number and charge the balance.

```rust
async fn get(&self, country: u32, days: u32, extension: bool) -> Result<RentItem>
```

| Param | Type | Description |
|---|---|---|
| `country` | `u32` | Country code. |
| `days` | `u32` | One of the tariff's `days` keys. |
| `extension` | `bool` | `true` is the server default; `false` makes the rent non-extendable for good. |

```rust
let rent = client.rent().get(7, 7, true).await?;
println!("{} {} {}", rent.tzid, rent.number, rent.sum);
```

Errors: `ApiError` with `is_no_number()`; `ApiError` `UNDEFINED_DAYS`, `UNDEFINED_COUNTRY`, `WARNING_LOW_BALANCE`, `INTERVAL_CONCURRENT_REQUESTS_ERROR`, `TRY_AGAIN_LATER`.

## state

All rents of the account.

```rust
async fn state(&self) -> Result<Vec<RentItem>>
```

```rust
for rent in client.rent().state().await? {
    println!("{} {:?}", rent.number, rent.messages.first().map(|m| &m.code));
}
```

## state_one

One rent.

```rust
async fn state_one(&self, tzid: i64) -> Result<RentItem>
```

```rust
let messages = client.rent().state_one(tzid).await?.messages;
```

Errors: `ApiError` `ERROR_NO_OPERATIONS` for an unknown tzid.

## extend

Extend a rent by `days` days and charge the balance. Only rents created with `extension: true`.

```rust
async fn extend(&self, tzid: i64, days: u32) -> Result<RentItem>
```

```rust
let rent = client.rent().extend(tzid, 3).await?;
println!("{}", rent.stop_time);
```

Errors: `ApiError` `TRY_AGAIN_LATER` (not extendable, unknown tzid or period, a repeat within 10 s), `WARNING_LOW_BALANCE`.

## close

Return the number before the period ends. A rent that is not confirmed yet and has no SMS can be closed only after 120 s; US numbers cannot be closed early.

```rust
async fn close(&self, tzid: i64) -> Result<()>
```

```rust
client.rent().close(tzid).await?;
```

Errors: `ApiError` `ERROR_NO_OPERATIONS`, `NO_COMPLETE_TZID`, `NO_CLOSE_RENT_US`, `TRY_AGAIN_LATER`.

## tariffs

Rent prices of every country, keyed by country code. No API key needed.

```rust
async fn tariffs(&self) -> Result<HashMap<String, RentTariff>>
```

`RentTariff`: `{ code, name, enabled, is_new, position, extend, confirm, days, count, ext_counts }` — `days` maps rent days to the price in USD, `count` to the numbers available.

```rust
let tariffs = client.rent().tariffs().await?;
```

## tariffs_one

Rent prices of one country.

```rust
async fn tariffs_one(&self, country: u32) -> Result<RentTariff>
```

```rust
let days = client.rent().tariffs_one(7).await?.days;
println!("{:?}", days.get("7")); // price of 7 days
```

Errors: `ApiError` `NO_COUNTRY` for a country without rent.
