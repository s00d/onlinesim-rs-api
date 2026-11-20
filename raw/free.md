# Free numbers

> Public numbers and their SMS — client.free(). No API key needed.

## list

Countries, the numbers of the selected country and the SMS of the selected number, in one call.

```rust
async fn list(&self, options: &FreeListOptions) -> Result<FreeList>
```

<table>
<thead>
  <tr>
    <th>
      <code>
        FreeListOptions
      </code>
      
       field
    </th>
    
    <th>
      Type
    </th>
    
    <th>
      Default
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
        country
      </code>
    </td>
    
    <td>
      <code>
        Option<u32>
      </code>
    </td>
    
    <td>
      <code>
        44
      </code>
      
       (server)
    </td>
    
    <td>
      Country code; an unknown one falls back to the first available.
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
      first number
    </td>
    
    <td>
      Number <strong>
        without
      </strong>
      
       the country code, as the keys of <code>
        numbers
      </code>
      
      .
    </td>
  </tr>
  
  <tr>
    <td>
      <code>
        page
      </code>
    </td>
    
    <td>
      <code>
        Option<u32>
      </code>
    </td>
    
    <td>
      <code>
        1
      </code>
    </td>
    
    <td>
      Page of <code>
        messages
      </code>
      
      .
    </td>
  </tr>
  
  <tr>
    <td>
      <code>
        count
      </code>
    </td>
    
    <td>
      <code>
        Option<u32>
      </code>
    </td>
    
    <td>
      server default
    </td>
    
    <td>
      SMS per page, 10–500.
    </td>
  </tr>
</tbody>
</table>

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
