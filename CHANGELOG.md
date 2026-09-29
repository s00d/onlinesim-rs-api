# Changelog

## [0.1.4] - 2026-09-29

### Fixed
- `ERROR_NO_OPERATIONS` → empty `state` / wait_code (not a hard error)
- Strip `service_` prefix on `getNum`
- Rent `tariffsRent` empty `[]` / null handling
- `getPrice` null → `"0"`
- Client pacing: 1 rps, `setOperationOk` ≥5s, retry on `INTERVAL_CONCURRENT_REQUESTS_ERROR`
- Mock disables rate limiting

### Notes
- `DEFAULT_COUNTRY` stays **1** (USA). Country 7 (RU) must not be used as default.

## [0.1.3] - previous

See git history.
