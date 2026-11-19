//! Order a number and wait for its SMS code.
//!
//! `ONLINESIM_KEY=... cargo run --example receive_sms -- google 44`

use onlinesim_rs_api::{Client, WaitCodeOptions};

#[tokio::main]
async fn main() -> onlinesim_rs_api::Result<()> {
    let key = std::env::var("ONLINESIM_KEY").expect("set ONLINESIM_KEY");
    let mut args = std::env::args().skip(1);
    let service = args.next().unwrap_or_else(|| "google".into());
    let country = args.next().and_then(|c| c.parse().ok()).unwrap_or(44);

    let client = Client::new(key)?;
    let balance = client.user().balance().await?;
    println!("available: {:.2} USD", balance.balance - balance.zbalance);

    let order = client
        .numbers()
        .get_with_number(&service, country, &[])
        .await?;
    println!("enter {} in the app", order.number);

    let code = client
        .numbers()
        .wait_code(order.tzid, WaitCodeOptions::default())
        .await?;
    println!("code: {code}");
    Ok(())
}
