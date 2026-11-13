//! The datasets, their formats and the hours held, and a few markets of a series. Needs no key.
//!
//! ```sh
//! cargo run --example datasets
//! cargo run --example datasets -- bitcoin-above      # markets of one series
//! ```

use probnode::{Client, MarketQuery};

#[tokio::main]
async fn main() -> probnode::Result<()> {
    let pn = Client::from_env()?;

    for d in pn.datasets().await? {
        let held = match (&d.first_hour, &d.last_hour) {
            (Some(a), Some(b)) => format!("{a} .. {b}"),
            _ => "nothing held".to_string(),
        };
        let formats = d
            .formats
            .iter()
            .chain(d.market_day_formats.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        println!("{:<14} {:<5} {:<22} {}", d.code, d.tier, formats, held);
    }

    let series = std::env::args().nth(1);
    let query = MarketQuery {
        series,
        limit: Some(5),
        ..MarketQuery::default()
    };
    println!();
    for m in pn.markets(&query).await? {
        let outcomes: Vec<&str> = m.tokens.iter().map(|t| t.outcome.as_str()).collect();
        println!("{}  {} .. {}  {:?}", m.slug, m.start_ts, m.end_ts, outcomes);
    }
    Ok(())
}
