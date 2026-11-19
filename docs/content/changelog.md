---
title: Changelog
description: Release notes of onlinesim-rs-api.
navigation:
  order: 7
---

The full history is in [CHANGELOG.md](https://github.com/s00d/onlinesim-rs-api/blob/master/CHANGELOG.md).

## 0.2.1

- `MockOnlineSim`: an in-process fake of the API for your tests (feature `mock-transport`, no extra dependencies): `MockOnlineSim::new(options).client()` returns a normal client that never opens a socket; `transport()` for clients you build yourself. Helpers `add_sms`, `add_sms_after` (zero-interval `wait_code`), `fail_next` / `fail_next_on`, `fail_next_transport`, `set_balance` / `balance`, `operations`, `rents`, `requests`, `reset`. See [Testing](/testing).
- Feature `mock` = `mock-transport` + `MockOnlineSim::start()`, the same fake over local HTTP. Its defaults now follow the shared mock spec (prices, numbers, key check, no automatic SMS).

## 0.2.0

A rewrite with a new API; 0.1 code does not compile unchanged.

- One `ClientBuilder` for both clients; the async and blocking clients have the same sections and methods (`client.numbers()`, `client.rent()`, `client.user()`, `client.free()`).
- The key is sent as a Bearer header, never in the URL. Default 30 s request timeout; the transport is injectable (your `reqwest` client or any `Transport`).
- One error enum: `Error::Api(ApiError { code, message })` (`is_no_number()`), `Transport`, `Timeout`, `Decode`, `Config`. An HTTP 456 wrong key is an `ApiError`.
- `numbers().wait_code(tzid, WaitCodeOptions { interval, timeout, keep_open, full_message, since })`: a simple per-call poller with `Duration`s, terminal states and transient-error retries; the shared batch poller is gone.
- Methods take the country explicitly; booleans are sent as `1`/`0`, `orderby` lowercase, `reject` as a comma list.
- `rent().get(country, days, extension)` sends `extension` explicitly.
- New: `user().set_webhook_url` / `clear_webhook_url` / `webhook_logs`, `parse_webhook`, `parse_webhook_value`.
- Removed: `numbers().repeat`, `service`, `service_number`, `rent().port_reload`, the old free endpoints (use `free().list`), client-side rate limiting, `oauth`, mock state files.
- MSRV 1.85.
