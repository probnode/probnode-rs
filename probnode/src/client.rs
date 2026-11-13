//! The asynchronous client.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::{Method, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::config::Config;
use crate::models::*;
use crate::{Error, Result};

/// The production API.
pub const DEFAULT_BASE_URL: &str = "https://probnode.com";
/// `POST /v1/data/urls` signs at most this many paths a request; [`Client::sign`] batches.
pub const SIGN_BATCH: usize = 100;
const USER_AGENT: &str = concat!("probnode-rs/", env!("CARGO_PKG_VERSION"));

/// A session against one ProbNode deployment.
///
/// Every method is one HTTP call, except [`download`](Client::download) (sign, then fetch) and
/// [`sign`](Client::sign) (one call per 100 paths). Public calls (datasets, the catalog,
/// coverage, samples) work without a key. The client is cheap to clone and safe to share across
/// tasks.
#[derive(Debug, Clone)]
pub struct Client {
    api_key: Option<String>,
    base_url: String,
    http: reqwest::Client,
    fetch: reqwest::Client,
}

/// Builds a [`Client`]. Nothing is read from the environment unless
/// [`from_env`](ClientBuilder::from_env) is called.
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    timeout: Duration,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        ClientBuilder {
            api_key: None,
            base_url: None,
            timeout: Duration::from_secs(60),
        }
    }
}

impl ClientBuilder {
    /// Fills what is not set yet from `PROBNODE_API_KEY` / `PROBNODE_BASE_URL`, then from the
    /// key `probnode login` saved (see [`config`](crate::config)).
    pub fn from_env(mut self) -> Result<Self> {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let saved = Config::load()?;
        if self.api_key.is_none() {
            self.api_key = env("PROBNODE_API_KEY").or(saved.api_key);
        }
        if self.base_url.is_none() {
            self.base_url = env("PROBNODE_BASE_URL").or(saved.base_url);
        }
        Ok(self)
    }

    /// The API key.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// The API base URL (default [`DEFAULT_BASE_URL`]).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// The timeout of each request, a whole download included (default 60 s; raise it for
    /// large files).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The client.
    pub fn build(self) -> Result<Client> {
        let base_url = self
            .base_url
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string();
        // API calls never follow redirects, so the key is never sent anywhere but the API.
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(self.timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        // Signed links point at the store, possibly another host; fetched without the key.
        let fetch = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(self.timeout)
            .build()?;
        Ok(Client {
            api_key: self.api_key.filter(|k| !k.is_empty()),
            base_url,
            http,
            fetch,
        })
    }
}

#[derive(Deserialize)]
struct FilesOut {
    files: Vec<DataFile>,
}

#[derive(Deserialize)]
struct UrlsOut {
    urls: Vec<SignedUrl>,
}

#[derive(Deserialize)]
struct LoginOut {
    key: String,
}

impl Client {
    /// A client with the key from `PROBNODE_API_KEY` or `probnode login`, against
    /// `PROBNODE_BASE_URL`, the saved base URL, or `https://probnode.com`.
    pub fn from_env() -> Result<Client> {
        ClientBuilder::default().from_env()?.build()
    }

    /// A client with this key, against `https://probnode.com`.
    pub fn new(api_key: impl Into<String>) -> Result<Client> {
        ClientBuilder::default().api_key(api_key).build()
    }

    /// A builder, for another base URL or timeout.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// The API base URL, without a trailing slash.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The key in use, if any.
    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    // -- plumbing ---------------------------------------------------------------------------

    fn request(&self, method: Method, path: &str, need_key: bool) -> Result<RequestBuilder> {
        let rb = self
            .http
            .request(method, format!("{}{}", self.base_url, path));
        match (&self.api_key, need_key) {
            (Some(k), _) => Ok(rb.bearer_auth(k)),
            (None, true) => Err(Error::NoApiKey),
            (None, false) => Ok(rb),
        }
    }

    async fn send(rb: RequestBuilder) -> Result<Response> {
        let response = rb.send().await?;
        check(response).await
    }

    async fn json<T: DeserializeOwned>(rb: RequestBuilder) -> Result<T> {
        let bytes = Client::send(rb).await?.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    // -- account ----------------------------------------------------------------------------

    /// Mints a key with email and password; the client uses it from then on. Save it with
    /// [`Config::save`] to share it with the `probnode` command line.
    pub async fn login(&mut self, email: &str, password: &str) -> Result<String> {
        let body = serde_json::json!({ "email": email, "password": password });
        let out: LoginOut = Client::json(
            self.request(Method::POST, "/v1/auth/login", false)?
                .json(&body),
        )
        .await?;
        self.api_key = Some(out.key.clone());
        Ok(out.key)
    }

    /// The account.
    pub async fn me(&self) -> Result<User> {
        Client::json(self.request(Method::GET, "/v1/me", true)?).await
    }

    /// The plan's limits and today's use.
    pub async fn usage(&self) -> Result<Usage> {
        Client::json(self.request(Method::GET, "/v1/data/usage", true)?).await
    }

    // -- catalog ----------------------------------------------------------------------------

    /// Every dataset, its formats, its tier and the hours held. Public.
    pub async fn datasets(&self) -> Result<Vec<Dataset>> {
        Client::json(self.request(Method::GET, "/v1/data/datasets", false)?).await
    }

    /// Series, optionally of one category and matching a substring. Public.
    pub async fn series(&self, category: Option<&str>, q: Option<&str>) -> Result<Vec<Series>> {
        let mut params = Vec::new();
        push(&mut params, "category", category);
        push(&mut params, "q", q);
        Client::json(
            self.request(Method::GET, "/v1/catalog/series", false)?
                .query(&params),
        )
        .await
    }

    /// Markets, filtered. Public.
    pub async fn markets(&self, query: &MarketQuery) -> Result<Vec<Market>> {
        let mut params = Vec::new();
        push(&mut params, "series", query.series.as_deref());
        push(&mut params, "category", query.category.as_deref());
        push(&mut params, "q", query.q.as_deref());
        push(&mut params, "status", query.status.as_deref());
        let limit = query.limit.map(|v| v.to_string());
        let offset = query.offset.map(|v| v.to_string());
        push(&mut params, "limit", limit.as_deref());
        push(&mut params, "offset", offset.as_deref());
        Client::json(
            self.request(Method::GET, "/v1/catalog/markets", false)?
                .query(&params),
        )
        .await
    }

    /// The recorded hours of one token over an optional window. Public.
    pub async fn coverage(
        &self,
        token_id: &str,
        start: Option<&str>,
        end: Option<&str>,
    ) -> Result<Vec<CoverageHour>> {
        let mut params = vec![("token_id", token_id.to_string())];
        push(&mut params, "start", start);
        push(&mut params, "end", end);
        Client::json(
            self.request(Method::GET, "/v1/catalog/coverage", false)?
                .query(&params),
        )
        .await
    }

    /// Free sample files with their links. Public.
    pub async fn samples(&self) -> Result<Vec<Sample>> {
        Client::json(self.request(Method::GET, "/v1/samples", false)?).await
    }

    // -- files ------------------------------------------------------------------------------

    /// The files of one dataset and format over `[start, end)` (ISO times on the hour). Narrow
    /// with [`FilesRequest::market`] / [`FilesRequest::series`], cut per market and day with
    /// [`FilesRequest::granularity`], then [`send`](FilesRequest::send).
    pub fn files(
        &self,
        dataset: impl Into<String>,
        format: Format,
        start: impl Into<String>,
        end: impl Into<String>,
    ) -> FilesRequest<'_> {
        FilesRequest {
            client: self,
            dataset: dataset.into(),
            format,
            start: start.into(),
            end: end.into(),
            granularity: Granularity::Hour,
            markets: Vec::new(),
            series: Vec::new(),
        }
    }

    /// Signed links for these object paths: `object_path -> url`. One request per 100 paths.
    /// Links are valid for minutes; sign again any time.
    pub async fn sign<S: AsRef<str>>(&self, paths: &[S]) -> Result<HashMap<String, String>> {
        let mut urls = HashMap::with_capacity(paths.len());
        for chunk in paths.chunks(SIGN_BATCH) {
            let body = serde_json::json!({
                "paths": chunk.iter().map(|p| p.as_ref()).collect::<Vec<_>>()
            });
            let out: UrlsOut = Client::json(
                self.request(Method::POST, "/v1/data/urls", true)?
                    .json(&body),
            )
            .await?;
            urls.extend(out.urls.into_iter().map(|u| (u.object_path, u.url)));
        }
        Ok(urls)
    }

    /// Fetches one file to `dest/<object_path>` and returns that path. The bytes go to a
    /// `.part` file first, which is renamed only when complete and its sha256 matches the
    /// listing; on a mismatch it is removed and [`Error::Checksum`] returned.
    pub async fn download<D: Downloadable + ?Sized>(
        &self,
        item: &D,
        dest: impl AsRef<Path>,
    ) -> Result<PathBuf> {
        self.download_with(item, dest, None, true).await
    }

    /// [`download`](Client::download) with a link signed beforehand (`url`) and the sha256
    /// check optional (`verify`).
    pub async fn download_with<D: Downloadable + ?Sized>(
        &self,
        item: &D,
        dest: impl AsRef<Path>,
        url: Option<&str>,
        verify: bool,
    ) -> Result<PathBuf> {
        let object_path = item.object_path();
        let target = target_path(dest.as_ref(), object_path)?;
        let url = match url.or(item.signed_url()) {
            Some(u) => u.to_string(),
            None => self
                .sign(&[object_path])
                .await?
                .remove(object_path)
                .ok_or_else(|| Error::Invalid(format!("no signed link for {object_path}")))?,
        };
        if let Some(dir) = target.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let mut part_name = target.file_name().unwrap_or_default().to_os_string();
        part_name.push(".part");
        let part = target.with_file_name(part_name);

        let response = self.fetch.get(&url).send().await?;
        let status = response.status();
        if status.is_client_error() || status.is_server_error() {
            return Err(Error::Api {
                status: status.as_u16(),
                detail: format!("fetching {object_path} failed"),
            });
        }
        let mut hasher = Sha256::new();
        let written = async {
            let mut out = tokio::fs::File::create(&part).await?;
            let mut body = response.bytes_stream();
            while let Some(chunk) = body.next().await {
                let chunk = chunk?;
                hasher.update(&chunk);
                out.write_all(&chunk).await?;
            }
            out.flush().await?;
            out.sync_all().await?;
            Ok::<(), Error>(())
        }
        .await;
        if let Err(e) = written {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(e);
        }
        let actual = hex(&hasher.finalize());
        if verify && !actual.eq_ignore_ascii_case(item.sha256()) {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(Error::Checksum {
                object_path: object_path.to_string(),
                expected: item.sha256().to_string(),
                actual,
            });
        }
        tokio::fs::rename(&part, &target).await?;
        Ok(target)
    }

    /// Downloads every file, signing their links in batches of 100 first. Returns the paths in
    /// the order of `files`; stops at the first failure.
    pub async fn download_all(
        &self,
        files: &[DataFile],
        dest: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>> {
        let paths: Vec<&str> = files.iter().map(|f| f.object_path.as_str()).collect();
        let urls = self.sign(&paths).await?;
        let mut out = Vec::with_capacity(files.len());
        for f in files {
            let url = urls.get(&f.object_path).map(String::as_str);
            out.push(self.download_with(f, dest.as_ref(), url, true).await?);
        }
        Ok(out)
    }

    /// One market's UTC day of a schema (default `book-max`, the full book) as parquet or
    /// csv.gz, made on the first request and found by later ones. Set
    /// [`schema`](ExportRequest::schema) and [`format`](ExportRequest::format), then
    /// [`send`](ExportRequest::send); pass the answer to [`download`](Client::download). The
    /// day must have ended (UTC). Needs a plan that covers the schema.
    ///
    /// ```no_run
    /// # async fn run() -> probnode::Result<()> {
    /// use probnode::{Client, Format};
    /// let pn = Client::from_env()?;
    /// let day = pn.export_book("will-x-happen", "2026-11-12")
    ///     .schema("book-max").format(Format::Parquet).send().await?;
    /// let path = pn.download(&day, "data/").await?;
    /// println!("{} rows in {}", day.rows, path.display());
    /// # Ok(()) }
    /// ```
    pub fn export_book(
        &self,
        market: impl Into<String>,
        day: impl Into<String>,
    ) -> ExportRequest<'_> {
        ExportRequest {
            client: self,
            market: market.into(),
            day: day.into(),
            schema: "book-max".to_string(),
            format: Format::Parquet,
        }
    }

    // -- range query ------------------------------------------------------------------------

    /// Rows of one market over a window (at most 7 days, 200,000 rows) as parquet bytes.
    pub async fn timeseries(
        &self,
        dataset: &str,
        market: &str,
        start: &str,
        end: &str,
    ) -> Result<bytes::Bytes> {
        let rb = self.timeseries_request(dataset, market, start, end, "parquet")?;
        Ok(Client::send(rb).await?.bytes().await?)
    }

    /// [`timeseries`](Client::timeseries) as NDJSON, one JSON object per row.
    pub async fn timeseries_ndjson(
        &self,
        dataset: &str,
        market: &str,
        start: &str,
        end: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let rb = self.timeseries_request(dataset, market, start, end, "ndjson")?;
        let text = Client::send(rb).await?.text().await?;
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).map_err(Error::from))
            .collect()
    }

    fn timeseries_request(
        &self,
        dataset: &str,
        market: &str,
        start: &str,
        end: &str,
        format: &str,
    ) -> Result<RequestBuilder> {
        Ok(self.request(Method::GET, "/v1/timeseries", true)?.query(&[
            ("dataset", dataset),
            ("market", market),
            ("start", start),
            ("end", end),
            ("format", format),
        ]))
    }
}

/// A `GET /v1/data/files` request being built; see [`Client::files`].
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is sent"]
pub struct FilesRequest<'a> {
    client: &'a Client,
    dataset: String,
    format: Format,
    start: String,
    end: String,
    granularity: Granularity,
    markets: Vec<String>,
    series: Vec<String>,
}

impl FilesRequest<'_> {
    /// `Hour` (the default) or `MarketDay`.
    pub fn granularity(mut self, granularity: Granularity) -> Self {
        self.granularity = granularity;
        self
    }

    /// Narrow to a market slug. Repeatable.
    pub fn market(mut self, slug: impl Into<String>) -> Self {
        self.markets.push(slug.into());
        self
    }

    /// Narrow to a series slug. Repeatable.
    pub fn series(mut self, slug: impl Into<String>) -> Self {
        self.series.push(slug.into());
        self
    }

    pub(crate) fn params(&self) -> Vec<(&'static str, String)> {
        let mut params = vec![
            ("dataset", self.dataset.clone()),
            ("format", self.format.as_str().to_string()),
            ("start", self.start.clone()),
            ("end", self.end.clone()),
        ];
        if self.granularity != Granularity::Hour {
            params.push(("granularity", self.granularity.as_str().to_string()));
        }
        params.extend(self.markets.iter().map(|m| ("market", m.clone())));
        params.extend(self.series.iter().map(|s| ("series", s.clone())));
        params
    }

    /// The files.
    pub async fn send(self) -> Result<Vec<DataFile>> {
        let rb = self
            .client
            .request(Method::GET, "/v1/data/files", true)?
            .query(&self.params());
        let out: FilesOut = Client::json(rb).await?;
        Ok(out.files)
    }
}

/// A `POST /v1/data/exports` request being built; see [`Client::export_book`].
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until it is sent"]
pub struct ExportRequest<'a> {
    client: &'a Client,
    market: String,
    day: String,
    schema: String,
    format: Format,
}

impl ExportRequest<'_> {
    /// The schema, one of [`MARKET_DAY_SCHEMAS`] (default `book-max`).
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = schema.into();
        self
    }

    /// `Format::Parquet` (the default) or `Format::Csv` (gzip-compressed).
    pub fn format(mut self, format: Format) -> Self {
        self.format = format;
        self
    }

    pub(crate) fn body(&self) -> serde_json::Value {
        serde_json::json!({
            "market": self.market,
            "day": self.day,
            "schema": self.schema,
            "format": self.format.as_str(),
        })
    }

    /// The export: its signed link, rows, bytes and sha256.
    pub async fn send(self) -> Result<BookExport> {
        let rb = self
            .client
            .request(Method::POST, "/v1/data/exports", true)?
            .json(&self.body());
        let out: BookExport = Client::json(rb).await?;
        if out.urls.len() != 1 {
            return Err(Error::Invalid(format!(
                "the export answered with {} links, expected one",
                out.urls.len()
            )));
        }
        Ok(out)
    }
}

fn push(params: &mut Vec<(&'static str, String)>, key: &'static str, value: Option<&str>) {
    if let Some(v) = value.filter(|v| !v.is_empty()) {
        params.push((key, v.to_string()));
    }
}

/// An error status into [`Error::Api`] with the API's `detail`.
async fn check(response: Response) -> Result<Response> {
    let status = response.status();
    if !(status.is_client_error() || status.is_server_error()) {
        return Ok(response);
    }
    let reason = status.canonical_reason().unwrap_or("error").to_string();
    let body = response.bytes().await.unwrap_or_default();
    let detail = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(serde_json::Value::Object(map)) => match map.get("detail") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Null) | None => reason,
            Some(other) => other.to_string(),
        },
        _ => reason,
    };
    Err(Error::Api {
        status: status.as_u16(),
        detail,
    })
}

/// `dest/<object_path>`, refusing a path that is absolute or climbs out of `dest`.
pub(crate) fn target_path(dest: &Path, object_path: &str) -> Result<PathBuf> {
    let rel = Path::new(object_path);
    let ok = !object_path.is_empty()
        && rel
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if !ok || object_path.contains('\\') {
        return Err(Error::Invalid(format!(
            "refusing object path {object_path:?}: not a relative path inside the destination"
        )));
    }
    Ok(dest.join(rel))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(DIGITS[(b >> 4) as usize] as char);
        s.push(DIGITS[(b & 15) as usize] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_paths_stay_inside_dest() {
        let dest = Path::new("data");
        assert!(target_path(dest, "book_full/evmin/2026-11-12/10.evmin").is_ok());
        for bad in ["", "../x", "a/../../x", "/etc/passwd", "a\\..\\x"] {
            assert!(target_path(dest, bad).is_err(), "{bad:?} accepted");
        }
    }

    #[test]
    fn hex_is_lowercase() {
        assert_eq!(hex(&[0, 0xab, 0x0f]), "00ab0f");
    }
}
