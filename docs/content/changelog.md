---
title: Changelog
description: Release notes of onlinesim-rs-api.
navigation:
  order: 6
---

The full history is in [CHANGELOG.md](https://github.com/s00d/onlinesim-rs-api/blob/master/CHANGELOG.md).

## 0.2.1

- Fixed: packaging (`include` patterns in Cargo.toml anchored to the crate root). 0.2.0 was never published to crates.io.

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
