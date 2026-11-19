---
title: Free numbers
description: Public numbers and their SMS — client.free(). No API key needed.
navigation:
  order: 4
---

## list

Countries, the numbers of the selected country and the SMS of the selected number, in one call.

```rust
async fn list(&self, options: &FreeListOptions) -> Result<FreeList>
```

| `FreeListOptions` field | Type | Default | Description |
|---|---|---|---|
| `country` | `Option<u32>` | `44` (server) | Country code; an unknown one falls back to the first available. |
| `number` | `Option<String>` | first number | Number **without** the country code, as the keys of `numbers`. |
| `page` | `Option<u32>` | `1` | Page of `messages`. |
| `count` | `Option<u32>` | server default | SMS per page, 10–500. |

`FreeList`:

- `countries`: `Vec<FreeCountry>` — `{ country, country_text, country_original }`
- `numbers`: `HashMap<String, FreeNumber>` — `{ country, country_original, full_number, data_humans, is_archive }`, keyed by the short number
- `messages`: `FreeMessages` — `page` (a paginator of `{ id, text, in_number, my_number, code, created_at, data_humans }`), `number` and `country`; `code` is `-` when no code was found

```rust
use onlinesim_rs_api::FreeListOptions;

let uk = FreeListOptions { country: Some(44), ..FreeListOptions::default() };
let list = client.free().list(&uk).await?;
if let Some(short) = list.numbers.keys().next() {
    let options = FreeListOptions { number: Some(short.clone()), count: Some(20), ..uk };
    for sms in client.free().list(&options).await?.messages.page.data {
        println!("{} {} {}", sms.in_number, sms.code, sms.text);
    }
}
```
