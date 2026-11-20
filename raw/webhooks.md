# Webhooks

> Receive SMS as HTTP callbacks and parse them with parse_webhook.

Set a URL with [`client.user().set_webhook_url`](/user#set_webhook_url). For each SMS the server POSTs a JSON body to it: `webhook_type` `receiving_sms` for numbers, `rent_sms` for rents. Delivery attempts are listed by [`client.user().webhook_logs`](/user#webhook_logs).

Parsing needs no client: it works with `default-features = false`.

## parse_webhook

Parse an inbound webhook body from its JSON text; `parse_webhook_value` takes an already parsed `serde_json::Value`.

```rust
fn parse_webhook(body: &str) -> Result<WebhookPayload>
fn parse_webhook_value(value: &serde_json::Value) -> Result<WebhookPayload>
```

Lenient: numeric fields may arrive as strings, missing or malformed fields become `None`, unknown fields are dropped. It fails (`Error::Decode`) only when the body is not a JSON object.

```rust
use onlinesim_rs_api::parse_webhook;
use onlinesim_rs_api::types::WebhookType;

// In your HTTP handler, with the request body as text:
fn on_webhook(body: &str) -> onlinesim_rs_api::Result<()> {
    let hook = parse_webhook(body)?;
    if hook.webhook_type == Some(WebhookType::ReceivingSms) {
        println!("{:?} {:?}", hook.operation_id, hook.code);
    }
    Ok(()) // answer HTTP 200
}

fn main() {
    on_webhook(r#"{"webhook_type":"receiving_sms","operation_id":"1520","code":"195895"}"#).unwrap();
}
```

## Payload fields

<table>
<thead>
  <tr>
    <th>
      Field
    </th>
    
    <th>
      Type
    </th>
    
    <th>
      Description
    </th>
  </tr>
</thead>

<tbody>
  <tr>
    <td>
      <code>
        webhook_type
      </code>
    </td>
    
    <td>
      <code>
        Option<WebhookType>
      </code>
    </td>
    
    <td>
      <code>
        ReceivingSms
      </code>
      
       or <code>
        RentSms
      </code>
      
      ; <code>
        None
      </code>
      
       for anything else.
    </td>
  </tr>
  
  <tr>
    <td>
      <code>
        operation_id
      </code>
    </td>
    
    <td>
      <code>
        Option<i64>
      </code>
    </td>
    
    <td>
      <code>
        tzid
      </code>
      
       of the operation or rent.
    </td>
  </tr>
  
  <tr>
    <td>
      <code>
        user_id
      </code>
    </td>
    
    <td>
      <code>
        Option<i64>
      </code>
    </td>
    
    <td>
      Account id.
    </td>
  </tr>
  
  <tr>
    <td>
      <code>
        country_code
      </code>
    </td>
    
    <td>
      <code>
        Option<i64>
      </code>
    </td>
    
    <td>
      Country code of the number.
    </td>
  </tr>
  
  <tr>
    <td>
      <code>
        number
      </code>
    </td>
    
    <td>
      <code>
        Option<String>
      </code>
    </td>
    
    <td>
      Numbers: <code>
        +
      </code>
      
       and the full number (<code>
        +79001234567
      </code>
      
      ), or <code>
        hidden
      </code>
      
      . Rent: the full number in digits without <code>
        +
      </code>
      
       (<code>
        79001234567
      </code>
      
      ).
    </td>
  </tr>
  
  <tr>
    <td>
      <code>
        sender
      </code>
    </td>
    
    <td>
      <code>
        Option<String>
      </code>
    </td>
    
    <td>
      Sender of the SMS, e.g. <code>
        VK
      </code>
      
      .
    </td>
  </tr>
  
  <tr>
    <td>
      <code>
        message
      </code>
    </td>
    
    <td>
      <code>
        Option<String>
      </code>
    </td>
    
    <td>
      Full SMS text.
    </td>
  </tr>
  
  <tr>
    <td>
      <code>
        code
      </code>
    </td>
    
    <td>
      <code>
        Option<String>
      </code>
    </td>
    
    <td>
      Code extracted from the SMS; empty when none was found.
    </td>
  </tr>
  
  <tr>
    <td>
      <code>
        time_start
      </code>
    </td>
    
    <td>
      <code>
        Option<String>
      </code>
    </td>
    
    <td>
      Numbers: when the operation was ordered. Rent: when the SMS arrived. <code>
        YYYY-MM-DD HH:MM:SS
      </code>
      
      .
    </td>
  </tr>
  
  <tr>
    <td>
      <code>
        time_left
      </code>
    </td>
    
    <td>
      <code>
        Option<i64>
      </code>
    </td>
    
    <td>
      Time left: seconds for numbers, minutes for rent.
    </td>
  </tr>
</tbody>
</table>
