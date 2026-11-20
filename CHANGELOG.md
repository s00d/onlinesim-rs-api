# Changelog

## [0.2.2]

Aligned with the regenerated backend API spec.

### Fixed

- Models (decoding no longer fails on real answers): `Operation.response` is `Option<OperationStatus>` (absent when no number could be issued); `Profile.number` is `Option<i64>` and `Profile.country` `Option<String>` (they were swapped); `PaymentOrder.currency`, `FreeCountry.country_text`/`country_original` and `FreeNumber.country_original` are `Option<String>` (the server sends `null`); `RentSms.code` stays a `String`, the integer `0` of a `notify` SMS becomes `"0"`; rent `sum` is read from a number or a decimal string.
- `user().payment_history()` no longer drops `paylist` when the server sends it as a JSON array (ids 0, 1, 2…); it is kept, keyed `"0"`, `"1"`, ….
- `numbers().wait_code` retries `ApiError` `TRY_AGAIN_LATER` (HTTP 429 rate limit), `ERROR_UNDEFINED` and `INTERVAL_CONCURRENT_REQUESTS_ERROR` until its timeout instead of returning at once.
- Error messages: `"0"` (ban not applied, the operation already has an SMS), `ACCOUNT_CLEANED`, `USER_NOT_ACTIVATED`, `ERROR_UNDEFINED`, `FORWARD_NOT_SUPPORTED` added; `ERROR_NO_KEY`, `NO_NUMBER_FOR_FORWARD`, `ACCOUNT_IDENTIFICATION_REQUIRED`, `API_DISABLED` (no longer sent) removed; `is_no_number()` is only `NO_NUMBER`; `ERROR_WRONG_KEY` reads "Wrong or missing API key".
- `MockOnlineSim` follows the backend: `setOperationRevise` answers `{"tzid":"N","response":"1"}` and `ERROR_WRONG_TZID` / `ERROR_NO_TZID`; `setOperationOk` tzid `0` → `ERROR_NO_TZID`; a missing key is HTTP 200 `ERROR_WRONG_KEY` (456 only for a wrong key, also on public endpoints); `getNum` with `number` adds `title`, `response_text`, `guard_interval_remaining_seconds`. Rent: `getRentNum` reserves the price (`status` 0) and the first non-`notify` SMS confirms and charges it (a `notify` SMS has the integer `code` `0`), unknown country → `UNDEFINED_COUNTRY`, `sum` a number in getRentNum/extendRentState and a string in getRentState, `guard_interval_remaining_seconds` with `number`; `extendRentState` refusals are all `TRY_AGAIN_LATER`; `closeRentNum` unknown tzid → `ERROR_NO_OPERATIONS`, releases the reservation of an unconfirmed rent. Webhook-log params: `code` `""` without a code, rent `number` in full digits, rent `time_left` in minutes, `time_start` per type. `profile.php` accepts any `scheme://…` URL. `getPaymentHistory` adds `forms`/`buttons`/`domains`; `getFreeList` lists 44 first, clamps `count` to 10..500, sends ISO-8601 `created_at` and `ignore`/`content`. `MockRent` has the new `initial_days` and `status` fields.
- Docs: rent reservation and `sum`, available = balance − max(zbalance, 0), `ban` answer `"0"`, webhook `number`/`time_left`/`time_start`, the `wait_code` retry policy, the mock's behaviour.

## [0.2.1]

### Added

- `mock::MockOnlineSim` is now an in-process fake of the OnlineSim API: `MockOnlineSim::new(MockOptions)` / `default()`, no server, no sockets, no runtime needed. `client()` / `client_blocking()` return normal clients wired to it; `transport()` is the `MockTransport` (`Transport` + `BlockingTransport`) for clients you build yourself; `builder()` a `ClientBuilder` with the mock's key. It answers every endpoint the SDK calls like the real backend (HTTP 456 for a wrong key, string/int `response`, decimal-string money, `[]` for empty maps) and keeps balance, frozen sums, operations, rents, webhook URL and webhook log. Deterministic: counter-based ids and numbers, fixed-epoch timestamps.
- Mock control API shared with the other OnlineSim SDKs: `add_sms` / `add_sms_from`, `add_sms_after` / `add_sms_after_from` (lets `wait_code` run with a zero interval), `fail_next_on(endpoint, code)`, `fail_next_transport(status)` / `fail_next_transport_on`, `balance()`, `operations()`, `rents()`, `requests()`, `reset()`, `MockOptions` (`api_key`, `balance`, `prices`, `rent_tariffs`, `guard_interval`), `mock::extract_code`, `MOCK_BASE_URL`. See the [testing guide](https://s00d.github.io/onlinesim-rs-api/testing).
- Feature `mock-transport`: the in-process mock without extra dependencies, for the async or the blocking client. Feature `mock` now means `mock-transport` plus `MockOnlineSim::start()` / `start_with(options)`, the same state served over local HTTP (wiremock).
- CI: `cargo package --list` check, clippy for every feature combination (`cargo hack --feature-powerset`), `cargo deny check`, a smoke job that packages the crate, unpacks the `.crate` into an empty project and runs the README "Testing" example; a `release.yml` workflow (crates.io trusted publishing).

### Changed

- The mock follows the shared mock spec: default prices `{7: {vkcom: 0.5, telegram: 1.2, google: 0.3}, 44: {google: 0.2, vkcom: 0.36}}` instead of 0.5 for anything, numbers `+<country>9000000NN` and tzids from 1, the key must match `api_key` (`mock-key`), no SMS arrives unless added (0.2.0 sent `123456` to unscripted orders), close without an SMS refunds the price, unknown paths are a non-JSON 404. `fail_next` queues instead of replacing. `set_balance(balance, zbalance)` keeps its signature; `zbalance` is now added to the prices frozen by open operations. `script_sms` and `fail_no_number` still work.

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
