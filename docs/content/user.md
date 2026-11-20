---
title: User
description: Balance, profile, payments and the webhook URL — client.user().
navigation:
  order: 3
---

## balance

Balance in USD.

```rust
async fn balance(&self) -> Result<Balance> // { balance: f64, zbalance: f64, income: Option<f64> }
```

`balance` includes the sum frozen by numbers and unconfirmed rents in progress (`zbalance`, which may be negative): available = `balance − max(zbalance, 0)`. `income` is the partner income. The server caches the answer for 60 s.

```rust
let b = client.user().balance().await?;
let available = b.balance - b.zbalance.max(0.0);
```

Errors: `ApiError` `ERROR_WRONG_KEY`, `ACCOUNT_BLOCKED`, `API_ACCESS_DISABLED`, `API_ACCESS_IP`.

## profile

The account profile.

```rust
async fn profile(&self) -> Result<Profile>
```

`Profile`: `{ id, name, username, email, locale, currency, number_country, number_reject, webhook_url, api_access, number, country, … }`; all but `id` are `Option`. `number` is the phone number as digits (`Option<i64>`), `country` its country (`Option<String>`).

```rust
let profile = client.user().profile().await?;
println!("{:?} {:?}", profile.email, profile.webhook_url);
```

## payment_history

Top-up orders (paginated, newest first) and the available payment methods. Pages start at 1.

```rust
async fn payment_history(&self, page: u32) -> Result<PaymentHistory>
```

`PaymentHistory`: `{ orders: Paginator<PaymentOrder>, paylist, domains, forms, currency, buttons }`. `PaymentOrder.sum` stays a decimal string (`"89.00"`); `currency` and `form_text` may be `None`; fields without a typed slot are in `PaymentOrder.extra`. `paylist` maps a group id to its payment methods; when the ids are 0, 1, 2… the server sends a JSON array, which is kept keyed `"0"`, `"1"`, ….

```rust
let orders = client.user().payment_history(1).await?.orders;
println!("{} orders, page {}/{}", orders.total, orders.current_page, orders.last_page);
```

## set_webhook_url

Set the URL that receives SMS webhooks (see [Webhooks](/webhooks)).

```rust
async fn set_webhook_url(&self, url: &str) -> Result<()>
```

```rust
client.user().set_webhook_url("https://example.com/onlinesim-hook").await?;
```

Errors: `ApiError` `VALIDATION_ERROR` for an invalid URL; `message` holds the server's field messages.

## clear_webhook_url

Remove the webhook URL, also from operations in progress.

```rust
async fn clear_webhook_url(&self) -> Result<()>
```

```rust
client.user().clear_webhook_url().await?;
```

## webhook_logs

Webhook delivery log, newest first, 10 per page. Pages start at 1.

```rust
async fn webhook_logs(&self, page: u32) -> Result<Paginator<WebhookLog>>
```

`WebhookLog`: `{ id, user_id, webhook_url, kind, params, status, error, created_at, updated_at }` — `kind` (the server's `type`) is `receiving_sms` or `rent_sms`, `status` is `success`, `fail` or `pending`, `params` the payload that was sent (`serde_json::Value`; [`parse_webhook_value`](/webhooks) normalises it).

```rust
let logs = client.user().webhook_logs(1).await?;
for log in &logs.data {
    println!("{} {} {:?}", log.created_at, log.status, log.error);
}
```
