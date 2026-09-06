//! Demonstrate the built-in mock SMS flow (no real API key / numbers).
//!
//! ```bash
//! cargo run --example mock_sms_flow --features mock
//! ```

use onlinesim_rs_api::mock::{MockOnlineSim, SmsScript};
use onlinesim_rs_api::WaitCodeOptions;

#[tokio::main]
async fn main() -> onlinesim_rs_api::Result<()> {
    let mock = MockOnlineSim::start().await;
    mock.script_sms(SmsScript {
        service: "telegram".into(),
        code: "424242".into(),
        polls_before_code: 0,
        ..SmsScript::default()
    });

    let client = mock.client()?;
    let ordered = client.numbers().get_with_number("telegram").await?;
    println!("mock number={} tzid={}", ordered.number, ordered.tzid);

    let code = client
        .numbers()
        .wait_code(
            ordered.tzid,
            WaitCodeOptions {
                interval_secs: 0,
                ..WaitCodeOptions::default()
            },
        )
        .await?;
    println!("mock code={code}");
    Ok(())
}
