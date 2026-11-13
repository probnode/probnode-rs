//! The official Rust client for the [ProbNode](https://probnode.com) API: prediction-market
//! order books, trades, bars and market statistics, delivered as parquet, csv.gz and Probe files
//! with a sha256 for every file.
//!
//! ```no_run
//! use probnode::{Client, Format};
//!
//! # async fn run() -> probnode::Result<()> {
//! let pn = Client::from_env()?;             // PROBNODE_API_KEY, or the key `probnode login` saved
//!
//! // Public: the datasets and the hours held.
//! for d in pn.datasets().await? {
//!     println!("{} {:?} {:?}", d.code, d.formats, d.last_hour);
//! }
//!
//! // One market's UTC day of the full book, made on demand, downloaded and sha256-checked.
//! let day = pn
//!     .export_book("will-x-happen", "2026-11-12")
//!     .schema("book-max")
//!     .format(Format::Parquet)
//!     .send()
//!     .await?;
//! let path = pn.download(&day, "data/").await?;
//! println!("{} rows in {}", day.rows, path.display());
//!
//! // Hourly files of a dataset, narrowed to one market.
//! let files = pn
//!     .files("trades", Format::Parquet, "2026-11-12T00:00:00Z", "2026-11-12T06:00:00Z")
//!     .market("will-x-happen")
//!     .send()
//!     .await?;
//! pn.download_all(&files, "data/").await?;
//! # Ok(()) }
//! ```
//!
//! # The key
//!
//! [`Client::from_env`] takes the key from `PROBNODE_API_KEY`, else from the file `probnode
//! login` writes (`~/.config/probnode.json`, or `$PROBNODE_CONFIG`; see [`config`]), and the
//! base URL from `PROBNODE_BASE_URL`, the same file, or `https://probnode.com`. Public calls
//! work without a key.
//!
//! # Errors
//!
//! Every call returns [`Result`]. A refusal from the API is [`Error::Api`] with the HTTP status
//! and the API's own sentence; a download whose bytes do not match the listed sha256 is
//! [`Error::Checksum`], and nothing is left at the target path.
//!
//! # Features
//!
//! - `blocking`: [`blocking::Client`], the same methods without async.
//! - `probe`: [`probe::open`] for the `book_full` dataset's Probe (`.evmin`) hours.

#![warn(missing_docs)]

mod client;
pub mod config;
mod error;
pub mod models;

#[cfg(feature = "blocking")]
pub mod blocking;
#[cfg(feature = "probe")]
pub mod probe;

pub use client::{
    Client, ClientBuilder, ExportRequest, FilesRequest, DEFAULT_BASE_URL, SIGN_BATCH,
};
pub use config::Config;
pub use error::{Error, Result};
pub use models::{
    BookExport, CoverageHour, DataFile, Dataset, Downloadable, Format, Granularity, Market,
    MarketQuery, Sample, Series, SignedUrl, Token, Usage, User, MARKET_DAY_SCHEMAS,
};
