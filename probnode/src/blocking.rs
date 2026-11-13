//! A synchronous client (feature `blocking`): the same methods as [`crate::Client`], each one
//! returning when its call is done.
//!
//! ```no_run
//! use probnode::{blocking::Client, Format};
//!
//! # fn main() -> probnode::Result<()> {
//! let pn = Client::from_env()?;
//! let day = pn.export_book("will-x-happen", "2026-11-12").format(Format::Csv).send()?;
//! let path = pn.download(&day, "data/")?;
//! println!("{} rows in {}", day.rows, path.display());
//! # Ok(()) }
//! ```
//!
//! It runs the asynchronous client on a small runtime of its own, so it must not be used from
//! inside an async runtime (use [`crate::Client`] there).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::models::*;
use crate::Result;

/// A synchronous session against one ProbNode deployment. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Client {
    inner: crate::Client,
    rt: Arc<tokio::runtime::Runtime>,
}

impl Client {
    /// Wraps an asynchronous client.
    pub fn from_async(inner: crate::Client) -> Result<Client> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        Ok(Client {
            inner,
            rt: Arc::new(rt),
        })
    }

    /// See [`crate::Client::from_env`].
    pub fn from_env() -> Result<Client> {
        Client::from_async(crate::Client::from_env()?)
    }

    /// See [`crate::Client::new`].
    pub fn new(api_key: impl Into<String>) -> Result<Client> {
        Client::from_async(crate::Client::new(api_key)?)
    }

    /// The asynchronous client underneath.
    pub fn as_async(&self) -> &crate::Client {
        &self.inner
    }

    /// See [`crate::Client::login`].
    pub fn login(&mut self, email: &str, password: &str) -> Result<String> {
        let rt = self.rt.clone();
        rt.block_on(self.inner.login(email, password))
    }

    /// See [`crate::Client::me`].
    pub fn me(&self) -> Result<User> {
        self.rt.block_on(self.inner.me())
    }

    /// See [`crate::Client::usage`].
    pub fn usage(&self) -> Result<Usage> {
        self.rt.block_on(self.inner.usage())
    }

    /// See [`crate::Client::datasets`].
    pub fn datasets(&self) -> Result<Vec<Dataset>> {
        self.rt.block_on(self.inner.datasets())
    }

    /// See [`crate::Client::series`].
    pub fn series(&self, category: Option<&str>, q: Option<&str>) -> Result<Vec<Series>> {
        self.rt.block_on(self.inner.series(category, q))
    }

    /// See [`crate::Client::markets`].
    pub fn markets(&self, query: &MarketQuery) -> Result<Vec<Market>> {
        self.rt.block_on(self.inner.markets(query))
    }

    /// See [`crate::Client::coverage`].
    pub fn coverage(
        &self,
        token_id: &str,
        start: Option<&str>,
        end: Option<&str>,
    ) -> Result<Vec<CoverageHour>> {
        self.rt.block_on(self.inner.coverage(token_id, start, end))
    }

    /// See [`crate::Client::samples`].
    pub fn samples(&self) -> Result<Vec<Sample>> {
        self.rt.block_on(self.inner.samples())
    }

    /// See [`crate::Client::files`].
    pub fn files(
        &self,
        dataset: impl Into<String>,
        format: Format,
        start: impl Into<String>,
        end: impl Into<String>,
    ) -> FilesRequest<'_> {
        FilesRequest {
            inner: self.inner.files(dataset, format, start, end),
            rt: &self.rt,
        }
    }

    /// See [`crate::Client::sign`].
    pub fn sign<S: AsRef<str>>(&self, paths: &[S]) -> Result<HashMap<String, String>> {
        self.rt.block_on(self.inner.sign(paths))
    }

    /// See [`crate::Client::download`].
    pub fn download<D: Downloadable + ?Sized>(
        &self,
        item: &D,
        dest: impl AsRef<Path>,
    ) -> Result<PathBuf> {
        self.rt.block_on(self.inner.download(item, dest))
    }

    /// See [`crate::Client::download_with`].
    pub fn download_with<D: Downloadable + ?Sized>(
        &self,
        item: &D,
        dest: impl AsRef<Path>,
        url: Option<&str>,
        verify: bool,
    ) -> Result<PathBuf> {
        self.rt
            .block_on(self.inner.download_with(item, dest, url, verify))
    }

    /// See [`crate::Client::download_all`].
    pub fn download_all(&self, files: &[DataFile], dest: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
        self.rt.block_on(self.inner.download_all(files, dest))
    }

    /// See [`crate::Client::export_book`].
    pub fn export_book(
        &self,
        market: impl Into<String>,
        day: impl Into<String>,
    ) -> ExportRequest<'_> {
        ExportRequest {
            inner: self.inner.export_book(market, day),
            rt: &self.rt,
        }
    }

    /// See [`crate::Client::timeseries`].
    pub fn timeseries(
        &self,
        dataset: &str,
        market: &str,
        start: &str,
        end: &str,
    ) -> Result<bytes::Bytes> {
        self.rt
            .block_on(self.inner.timeseries(dataset, market, start, end))
    }

    /// See [`crate::Client::timeseries_ndjson`].
    pub fn timeseries_ndjson(
        &self,
        dataset: &str,
        market: &str,
        start: &str,
        end: &str,
    ) -> Result<Vec<serde_json::Value>> {
        self.rt
            .block_on(self.inner.timeseries_ndjson(dataset, market, start, end))
    }
}

/// A files request being built; see [`crate::FilesRequest`].
#[derive(Debug)]
#[must_use = "a request does nothing until it is sent"]
pub struct FilesRequest<'a> {
    inner: crate::FilesRequest<'a>,
    rt: &'a tokio::runtime::Runtime,
}

impl FilesRequest<'_> {
    /// See [`crate::FilesRequest::granularity`].
    pub fn granularity(mut self, granularity: Granularity) -> Self {
        self.inner = self.inner.granularity(granularity);
        self
    }

    /// See [`crate::FilesRequest::market`].
    pub fn market(mut self, slug: impl Into<String>) -> Self {
        self.inner = self.inner.market(slug);
        self
    }

    /// See [`crate::FilesRequest::series`].
    pub fn series(mut self, slug: impl Into<String>) -> Self {
        self.inner = self.inner.series(slug);
        self
    }

    /// The files.
    pub fn send(self) -> Result<Vec<DataFile>> {
        self.rt.block_on(self.inner.send())
    }
}

/// An export request being built; see [`crate::ExportRequest`].
#[derive(Debug)]
#[must_use = "a request does nothing until it is sent"]
pub struct ExportRequest<'a> {
    inner: crate::ExportRequest<'a>,
    rt: &'a tokio::runtime::Runtime,
}

impl ExportRequest<'_> {
    /// See [`crate::ExportRequest::schema`].
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.inner = self.inner.schema(schema);
        self
    }

    /// See [`crate::ExportRequest::format`].
    pub fn format(mut self, format: Format) -> Self {
        self.inner = self.inner.format(format);
        self
    }

    /// The export: its signed link, rows, bytes and sha256.
    pub fn send(self) -> Result<BookExport> {
        self.rt.block_on(self.inner.send())
    }
}
