//! Opening a downloaded Probe file (feature `probe`).
//!
//! The `book_full` dataset's hourly files are Probe (`.evmin`): every change of every level of
//! every token, every trade with its transaction hash, both clocks. [`open`] hands one to the
//! [`probe_format`] reader.
//!
//! ```no_run
//! # async fn run() -> probnode::Result<()> {
//! use probnode::{Client, Format};
//!
//! let pn = Client::from_env()?;
//! let hours = pn
//!     .files("book_full", Format::Evmin, "2026-11-12T10:00:00Z", "2026-11-12T11:00:00Z")
//!     .send()
//!     .await?;
//! for path in pn.download_all(&hours, "data/").await? {
//!     let f = probnode::probe::open(&path)?;
//!     println!("{}: {} with {} markets", path.display(), f.version(), f.markets().len());
//! }
//! # Ok(()) }
//! ```

use std::path::Path;

pub use probe_format;

use crate::Result;

/// Opens a Probe file (memory-mapped).
pub fn open(path: impl AsRef<Path>) -> Result<probe_format::File> {
    Ok(probe_format::File::open(path.as_ref())?)
}
