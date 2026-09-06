//! Fetch account balance.
//!
//! ```bash
//! ONLINESIM_APIKEY=... cargo run --example get_balance
//! ```

use onlinesim_rs_api::Client;

#[tokio::main]
async fn main() -> onlinesim_rs_api::Result<()> {
    let apikey = std::env::var("ONLINESIM_APIKEY").expect("set ONLINESIM_APIKEY");
    let client = Client::new(apikey)?;
    let balance = client.user().balance().await?;
    println!(
        "balance={}, zbalance={}, income={:?}",
        balance.balance, balance.zbalance, balance.income
    );
    Ok(())
}
