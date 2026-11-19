# Changelog

## [0.2.0]

A rewrite aligned with the shared OnlineSim SDK contract. The public API changed; see the
[docs](https://s00d.github.io/onlinesim-rs-api/).

### Removed

- `numbers().repeat` (getNumRepeat), `numbers().service` (getService), `numbers().service_number` (getServiceNumber): no backend route.
- The `service_` prefix stripping in `get` (pass the slug, `ServiceStat::slug`).
- `rent().port_reload`: deprecated no-op on the backend. The old free endpoints (`getFreeCountryList`, `getFreePhoneList`, `getFreeMessageList`): use `free().list`.
- The shared batch `wait_code` poller, client-side rate limiting and automatic `INTERVAL_CONCURRENT_REQUESTS_ERROR` retries (hidden real sleeps inside calls).
- `ClientBuilder::oauth`, `disable_rate_limit`, `Config`, `DEFAULT_COUNTRY`, `request_error_message`, `GetNumberParams`, `parse_webhook_json`.
- `mock`: state files (`state_path`, `ONLINESIM_MOCK_STATE`), the browser-like User-Agent, the redundant `homepage` in Cargo.toml.

### Changed

- One `ClientBuilder` (`api_key`, `base_url`, `lang`, `dev_id`, `timeout`) builds both clients: `build()` / `build_with(transport)` and `build_blocking()` / `build_blocking_with(transport)`. Async and blocking sections are generated from one declaration, so their methods match.
- The API key is sent as `Authorization: Bearer <key>`, never in the query string.
- Errors: `Error::Api(ApiError { code, message })` (with `is_no_number()`), `Error::Transport { message, status, body }`, `Error::Timeout`, `Error::Decode`, `Error::Config`; `Error::code()`.
- Methods follow the contract: country is an explicit argument (`price(service, country)`, `get(service, country, reject)`, `tariffs_one(country)`), `price` returns `Option<f64>`, `close`/`next`/`ban` return `()`, `state(StateOptions)`, `state_one(tzid, StateOneOptions)`, `rent().get(country, days, extension)`, `user().payment_history(page)`, `free().list(&FreeListOptions)`.
- `numbers().wait_code(tzid, WaitCodeOptions { interval, timeout, keep_open, full_message, since })` with `Duration`s: polls `state_one` for that one operation, closes (or `next` with `keep_open`) after the code.
- Wire types mirror the OpenAPI schemas: `Operation` (`OperationStatus`, `OperationMsg`), `NumberOrder`, `CountryStat`, `RentItem`, `RentTariff`, `Balance`, `Profile`, `PaymentHistory`, `WebhookLog`, `FreeList`, `Paginator<T>`.
- `parse_webhook(&str)` / `parse_webhook_value(&Value)` return a lenient `WebhookPayload` with `Option` fields.
- `mock::MockOnlineSim` speaks the current wire format (Bearer key, 456 `ERROR_WRONG_KEY`, rent, profile validation) and adds `fail_next`, `webhook_url`.
- Edition 2024, MSRV 1.85 (required by reqwest 0.13 and tokio). Without features the crate builds with only types, errors and webhook parsing (no reqwest).

### Fixed

- The async client had no request timeout and its HTTP client could not be injected: default 30 s per request, applied to injected `reqwest` clients too; any `Transport` / `BlockingTransport` can be passed.
- `error_for_status()` ran before the JSON was read, so HTTP 456 `ERROR_WRONG_KEY` became an HTTP error; the body is now checked before the status. Non-JSON bodies are `Error::Transport` with status and a body excerpt.
- `response: true` was treated as an error code; validation objects in `response` become `VALIDATION_ERROR` with the field messages; a server `message` that only echoes the code falls back to the readable message.
- `wait_code`: cross-talk of `full_message` between waiters, a shared attempt counter, dropped futures still closing numbers, one transient error failing all waiters, `last_code` unused, whole-second intervals, no terminal states. Now `TZ_OVER_OK` / `ERROR_NO_ITEMS` end it at once, transport errors are retried until the timeout, and the last sleep is cut to the deadline.
- `state_one_with` sent `true`/`false` and `repeat`; booleans are now `1`/`0`, `orderby` is lowercase, `reject` is a comma list, unset parameters are omitted, `number=1` is sent to getState, rent requests send `pagination=0` and `number=1`.
- Missing entries in the error message table; `lock().expect` panics in the mock.

### Added

- `Transport` / `BlockingTransport` traits, `HttpRequest` / `HttpResponse`, `Clock` / `SystemClock` (blocking `wait_code_with`).
- `user().set_webhook_url`, `clear_webhook_url`, `webhook_logs`.
- Offline test suite on a fake transport (one test per method, error mapping, `wait_code` on a paused / fake clock), an opt-in integration test, a docs-examples compile check, CI on Rust 1.85 and stable with fmt, clippy and docs.
- Documentation site (Nuxt Content) in `docs/`.

## [0.1.4] - 2026-09-29

### Fixed
- `ERROR_NO_OPERATIONS` → empty `state` / wait_code (not a hard error)
- Strip `service_` prefix on `getNum`
- Rent `tariffsRent` empty `[]` / null handling
- `getPrice` null → `"0"`
- Client pacing: 1 rps, `setOperationOk` ≥5s, retry on `INTERVAL_CONCURRENT_REQUESTS_ERROR`
- Mock disables rate limiting

## [0.1.3] - previous

See git history.
