//! Order a number and wait for an SMS code.
//!
//! For local testing without real numbers use `mock_sms_flow` example instead.
//!
//! ```bash
//! ONLINESIM_APIKEY=... SERVICE=telegram cargo run --example wait_sms
//! ```

use onlinesim_rs_api::{Client, GetNumberParams, WaitCodeOptions};

#[tokio::main]
async fn main() -> onlinesim_rs_api::Result<()> {
    let apikey = std::env::var("ONLINESIM_APIKEY").expect("set ONLINESIM_APIKEY");
    let service = std::env::var("SERVICE").unwrap_or_else(|_| "telegram".into());
    let country: i64 = std::env::var("COUNTRY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);

    let client = Client::new(apikey)?;
    let ordered = client
        .numbers()
        .get_with_number_params(GetNumberParams::new(&service).country(country))
        .await?;
    println!("tzid={}, number={}", ordered.tzid, ordered.number);

    let code = client
        .numbers()
        .wait_code(
            ordered.tzid,
            WaitCodeOptions {
                interval_secs: 5,
                max_attempts: 24,
                ..WaitCodeOptions::default()
            },
        )
        .await?;
    println!("code={code}");
    Ok(())
}
