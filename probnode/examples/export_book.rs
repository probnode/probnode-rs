//! One market's UTC day of a schema, made on demand, downloaded and sha256-checked.
//!
//! ```sh
//! export PROBNODE_API_KEY=pn_...                     # or run `probnode login` once
//! cargo run --example export_book -- will-x-happen 2026-11-12
//! cargo run --example export_book -- will-x-happen 2026-11-12 trades csv
//! ```

use probnode::{Client, Format};

#[tokio::main]
async fn main() -> probnode::Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(market), Some(day)) = (args.next(), args.next()) else {
        eprintln!("usage: export_book <market-slug> <YYYY-MM-DD> [schema] [parquet|csv]");
        std::process::exit(2);
    };
    let schema = args.next().unwrap_or_else(|| "book-max".to_string());
    let format = match args.next().as_deref() {
        Some("csv") => Format::Csv,
        _ => Format::Parquet,
    };

    let pn = Client::from_env()?;
    let day = pn
        .export_book(&market, &day)
        .schema(&schema)
        .format(format)
        .send()
        .await?;
    let path = pn.download(&day, "data/").await?;
    println!(
        "{} {}: {} rows, {} bytes{} -> {}",
        market,
        day.schema,
        day.rows,
        day.bytes,
        if day.built { " (built now)" } else { "" },
        path.display()
    );
    Ok(())
}
