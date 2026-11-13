//! What the API returns, as typed structs.
//!
//! Every struct keeps the fields it does not name in `extra`, so a field the API adds later is
//! never lost. Times are the API's ISO 8601 strings (UTC).

use std::fmt;

use serde::{Deserialize, Serialize};

/// Unknown fields, kept.
pub type Extra = serde_json::Map<String, serde_json::Value>;

/// A file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Format {
    /// Apache Parquet (zstd).
    Parquet,
    /// CSV, gzip-compressed (`.csv.gz`).
    Csv,
    /// Probe, the full-depth event format of the `book_full` hours (`.evmin`).
    Evmin,
    /// The legacy native book format.
    Lp2,
}

impl Format {
    /// The name the API uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Parquet => "parquet",
            Format::Csv => "csv",
            Format::Evmin => "evmin",
            Format::Lp2 => "lp2",
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How the files of a selection are cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Granularity {
    /// One file per UTC hour, every market in it (the default).
    #[default]
    Hour,
    /// One file per market and UTC day.
    MarketDay,
}

impl Granularity {
    /// The name the API uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Granularity::Hour => "hour",
            Granularity::MarketDay => "market_day",
        }
    }
}

/// The schemas `POST /v1/data/exports` builds for one market's UTC day (see the API's schema
/// reference). `book-max` is the default; `book_full` is accepted as its alias.
pub const MARKET_DAY_SCHEMAS: [&str; 15] = [
    "book-max",
    "book-10",
    "book-top",
    "book-top-1s",
    "book-top-1m",
    "trades",
    "trade-quotes",
    "pair",
    "fills",
    "bars-1s",
    "bars-1m",
    "bars-1h",
    "bars-1d",
    "market-stats",
    "market-events",
];

/// A dataset, its formats, its tier and the hours held (`GET /v1/data/datasets`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    /// The dataset code, as `files` takes it (`book_full`, `trades`, ...).
    pub code: String,
    /// A human title.
    pub title: String,
    /// Formats of its hourly files.
    #[serde(default)]
    pub formats: Vec<String>,
    /// Formats of its market-day files.
    #[serde(default)]
    pub market_day_formats: Vec<String>,
    /// The plan tier that includes it.
    pub tier: String,
    /// The first hour held.
    #[serde(default)]
    pub first_hour: Option<String>,
    /// The last hour held.
    #[serde(default)]
    pub last_hour: Option<String>,
    /// The schema code its rows follow.
    #[serde(default)]
    pub schema: Option<String>,
    /// Other codes that name it.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// `L1`, `L2`, ...
    #[serde(default)]
    pub level: Option<String>,
    /// `event`, `grid`, `bar`, ...
    #[serde(default)]
    pub kind: Option<String>,
    /// One sentence on what it holds.
    #[serde(default)]
    pub blurb: Option<String>,
    /// Short tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// True for a dataset kept for older files only.
    #[serde(default)]
    pub legacy: bool,
    /// Fields not named above.
    #[serde(flatten)]
    pub extra: Extra,
}

/// A series of markets (`GET /v1/catalog/series`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Series {
    /// The series slug.
    pub slug: String,
    /// Its title.
    pub title: String,
    /// The venue (`polymarket`).
    pub venue: String,
    /// Its category (`crypto`, `politics`, ...).
    pub category: String,
    /// Fields not named above.
    #[serde(flatten)]
    pub extra: Extra,
}

/// One outcome token of a market.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    /// The venue's token id.
    pub token_id: String,
    /// The outcome it pays on (`Yes`, `Up`, a team name, ...).
    pub outcome: String,
    /// Fields not named above.
    #[serde(flatten)]
    pub extra: Extra,
}

/// A market (`GET /v1/catalog/markets`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Market {
    /// The market slug, as `files`, `export_book` and `timeseries` take it.
    pub slug: String,
    /// The series it belongs to.
    #[serde(default)]
    pub series_slug: Option<String>,
    /// When it opened.
    pub start_ts: String,
    /// When it closes or closed.
    pub end_ts: String,
    /// The winning outcome once resolved.
    #[serde(default)]
    pub resolution: Option<String>,
    /// Its outcome tokens.
    #[serde(default)]
    pub tokens: Vec<Token>,
    /// Fields not named above.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Filters for [`Client::markets`](crate::Client::markets). Every field is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarketQuery {
    /// Only this series.
    pub series: Option<String>,
    /// Only this category.
    pub category: Option<String>,
    /// A substring of the slug.
    pub q: Option<String>,
    /// `open` or `resolved`.
    pub status: Option<String>,
    /// Page size (API default 100, at most 1000).
    pub limit: Option<u32>,
    /// Rows to skip.
    pub offset: Option<u32>,
}

/// One token's hour of recording (`GET /v1/catalog/coverage`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageHour {
    /// The token.
    pub token_id: String,
    /// The hour's start.
    pub hour_start: String,
    /// Milliseconds of the hour recorded.
    pub recorded_ms: i64,
    /// Gaps in the hour.
    pub gap_count: i64,
    /// Share of the hour recorded, in percent.
    pub uptime_pct: f64,
    /// True when the hour is implied rather than recorded.
    #[serde(default)]
    pub implied: bool,
    /// Milliseconds derived rather than recorded.
    #[serde(default)]
    pub derived_ms: i64,
    /// The recorder era.
    #[serde(default)]
    pub era: Option<String>,
    /// Where an era change splits the hour, if one does.
    #[serde(default)]
    pub era_split_ms: Option<i64>,
    /// The recorder's clock discipline (`chrony`, ...).
    #[serde(default)]
    pub clock_source: Option<String>,
    /// NTP stratum of the clock.
    #[serde(default)]
    pub clock_stratum: Option<i64>,
    /// Share of the hour the clock was synchronised, in percent.
    #[serde(default)]
    pub clock_synced_pct: Option<f64>,
    /// Largest clock offset in the hour, microseconds.
    #[serde(default)]
    pub clock_offset_max_us: Option<i64>,
    /// Largest clock error bound in the hour, microseconds.
    #[serde(default)]
    pub clock_error_max_us: Option<i64>,
    /// Fields not named above.
    #[serde(flatten)]
    pub extra: Extra,
}

/// One file in the store, as `GET /v1/data/files` lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataFile {
    /// Its path in the store; also where [`Client::download`](crate::Client::download) puts it
    /// under the destination directory.
    pub object_path: String,
    /// The dataset code.
    pub dataset: String,
    /// The format.
    pub format: String,
    /// The UTC day (`YYYY-MM-DD`).
    pub event_date: String,
    /// The UTC hour (0 for a market-day file).
    #[serde(default)]
    pub hour: u32,
    /// The source.
    #[serde(default)]
    pub src: String,
    /// Size in bytes.
    pub bytes: u64,
    /// Rows (or events) in the file.
    #[serde(default)]
    pub rows: u64,
    /// The sha256 of the file, lowercase hex.
    pub sha256: String,
    /// `hour` or `market_day`.
    #[serde(default)]
    pub granularity: Option<String>,
    /// The market of a market-day file.
    #[serde(default)]
    pub market: Option<String>,
    /// The recorder era.
    #[serde(default)]
    pub era: Option<String>,
    /// Fields not named above.
    #[serde(flatten)]
    pub extra: Extra,
}

/// A free sample file, with its link (`GET /v1/samples`). Downloads without a key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// The link to the file.
    pub url: String,
    /// The file.
    #[serde(flatten)]
    pub file: DataFile,
}

/// A signed link to one object, valid for minutes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedUrl {
    /// The object's path in the store.
    pub object_path: String,
    /// The link. It carries its own authorisation: fetch it without the API key.
    pub url: String,
    /// When the link stops working.
    #[serde(default)]
    pub expires_at: Option<String>,
}

/// One market's UTC day of a schema, made or found by `POST /v1/data/exports`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookExport {
    /// The schema the file holds (`book-max` for `book_full`).
    pub schema: String,
    /// Rows in the file.
    pub rows: u64,
    /// Size in bytes.
    pub bytes: u64,
    /// The sha256 of the file, lowercase hex.
    pub sha256: String,
    /// True when this request made the file; false when it existed.
    #[serde(default)]
    pub built: bool,
    /// The signed link to the file (one).
    pub urls: Vec<SignedUrl>,
    /// The licence the data is delivered under.
    #[serde(default)]
    pub licence: Option<String>,
    /// The licence's terms.
    #[serde(default)]
    pub licence_detail: Option<String>,
    /// Fields not named above.
    #[serde(flatten)]
    pub extra: Extra,
}

impl BookExport {
    /// The file's path in the store.
    pub fn object_path(&self) -> Option<&str> {
        self.urls.first().map(|u| u.object_path.as_str())
    }
}

/// The account (`GET /v1/me`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    /// The account id.
    pub id: String,
    /// Its email.
    pub email: String,
    /// The plan.
    pub plan: String,
    /// When a paid plan ends.
    #[serde(default)]
    pub plan_expires_at: Option<String>,
    /// True once the email is verified.
    #[serde(default)]
    pub email_verified: bool,
    /// When the account was made.
    pub created_at: String,
    /// Fields not named above.
    #[serde(flatten)]
    pub extra: Extra,
}

/// The plan's limits and today's use (`GET /v1/data/usage`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    /// The plan.
    pub plan: String,
    /// The datasets it includes.
    #[serde(default)]
    pub datasets: Vec<String>,
    /// Signed links issued today.
    pub signed_urls_used_today: u64,
    /// Signed links allowed a day.
    pub signed_urls_per_day: u64,
    /// Requests allowed a minute.
    pub requests_per_minute: u64,
    /// When the daily count resets.
    pub resets_at: String,
    /// How fresh the plan's data is (`realtime`, `delayed`, ...).
    pub freshness: String,
    /// The newest hour the plan can fetch, if the plan is delayed.
    #[serde(default)]
    pub freshness_cutoff: Option<String>,
    /// Fields not named above.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Something [`Client::download`](crate::Client::download) can fetch: a listed file, an export,
/// or a sample.
pub trait Downloadable {
    /// The object's path in the store; the file lands at `dest/<object_path>`.
    fn object_path(&self) -> &str;
    /// The sha256 the download is checked against.
    fn sha256(&self) -> &str;
    /// A link already issued for it, if any; otherwise the client signs one.
    fn signed_url(&self) -> Option<&str>;
}

impl Downloadable for DataFile {
    fn object_path(&self) -> &str {
        &self.object_path
    }
    fn sha256(&self) -> &str {
        &self.sha256
    }
    fn signed_url(&self) -> Option<&str> {
        None
    }
}

impl Downloadable for Sample {
    fn object_path(&self) -> &str {
        &self.file.object_path
    }
    fn sha256(&self) -> &str {
        &self.file.sha256
    }
    fn signed_url(&self) -> Option<&str> {
        Some(&self.url)
    }
}

impl Downloadable for BookExport {
    fn object_path(&self) -> &str {
        BookExport::object_path(self).unwrap_or("")
    }
    fn sha256(&self) -> &str {
        &self.sha256
    }
    fn signed_url(&self) -> Option<&str> {
        self.urls.first().map(|u| u.url.as_str())
    }
}

impl<T: Downloadable + ?Sized> Downloadable for &T {
    fn object_path(&self) -> &str {
        (**self).object_path()
    }
    fn sha256(&self) -> &str {
        (**self).sha256()
    }
    fn signed_url(&self) -> Option<&str> {
        (**self).signed_url()
    }
}
