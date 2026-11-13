<p align="center">
  <a href="https://probnode.com"><img src="https://raw.githubusercontent.com/probnode/probnode-rs/main/assets/probnode-logo.png" alt="ProbNode" width="96" height="96"></a>
</p>

<h1 align="center">probnode-rs</h1>

<p align="center"><b>The official Rust client for ProbNode: Polymarket order books, trades and bars as sha256-checked files.</b></p>

<p align="center">
  <a href="https://github.com/probnode/probnode-rs/actions/workflows/ci.yml"><img src="https://github.com/probnode/probnode-rs/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="https://github.com/probnode/probnode-rs/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License: Apache-2.0"></a>
  <a href="https://github.com/probnode/probnode-rs/blob/main/Cargo.toml"><img src="https://img.shields.io/badge/MSRV-1.85-blue.svg" alt="MSRV 1.85"></a>
  <!-- enable on first release:
  <a href="https://crates.io/crates/probnode"><img src="https://img.shields.io/crates/v/probnode.svg" alt="crates.io"></a>
  <a href="https://docs.rs/probnode"><img src="https://img.shields.io/docsrs/probnode" alt="docs.rs"></a>
  <a href="https://crates.io/crates/probnode"><img src="https://img.shields.io/crates/d/probnode.svg" alt="Downloads"></a>
  -->
</p>

## Overview

`probnode` is the official Rust client for the [ProbNode](https://probnode.com) API. ProbNode
records Polymarket at full depth: every change of every price level of every outcome token, every
trade, and every on-chain fill. The client lists, signs and downloads that data as parquet, csv.gz
and [Probe](https://github.com/probnode/probe) files, checks every file against its sha256, and
builds one market's UTC day of any market-day schema on request.

The same API has official clients for [Python](https://github.com/probnode/probnode-sdk) and
[C++](https://github.com/probnode/probnode-cpp), with the same methods.

## Features

- **Async and blocking.** An async `Client` on tokio, and `blocking::Client` with the same
  methods behind the `blocking` feature.
- **Every endpoint, typed.** Account, catalog, coverage, files, signed links, market-day exports
  and time series, with typed models that keep every field they do not name in `extra`.
- **Downloads you can trust.** Each file is written to a `.part`, checked against the sha256 the
  API listed, and only then renamed; a mismatch removes it and returns `Error::Checksum`.
- **Fifteen market-day schemas.** From the full book (`book-max`) to top of book, trades, fills,
  bars and daily market statistics, as parquet or csv.gz.
- **Probe files.** With the `probe` feature, the `book_full` dataset's hourly Probe files open
  directly through [`probe-format`](https://crates.io/crates/probe-format).
- **Pure Rust TLS.** reqwest with rustls; nothing links OpenSSL.
- **Shares the login.** The key file `probnode login` writes is read by this client too.

## Installation

The first crates.io release of `probnode` is on the way. Until then, from this repository:

```toml
[dependencies]
probnode = { git = "https://github.com/probnode/probnode-rs", tag = "v0.1.0" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }   # the client is async
```

Once released:

```sh
cargo add probnode
cargo add tokio --features macros,rt-multi-thread
```

Features:

| feature | what it adds |
|---|---|
| `blocking` | `probnode::blocking::Client`, the same methods without async |
| `probe` | `probnode::probe::open`, to read the `book_full` dataset's Probe (`.evmin`) hours with [`probe-format`](https://crates.io/crates/probe-format) |

## Quick start

```rust
use probnode::{Client, Format};

#[tokio::main]
async fn main() -> probnode::Result<()> {
    let pn = Client::from_env()?; // PROBNODE_API_KEY, or the key `probnode login` saved

    // One market's UTC day of the full book: made on the first request, then found.
    let day = pn
        .export_book("will-x-happen", "2026-11-12")
        .schema("book-max")
        .format(Format::Parquet)
        .send()
        .await?;
    let path = pn.download(&day, "data/").await?; // sha256-checked; returns the PathBuf
    println!("{} rows in {}", day.rows, path.display());
    Ok(())
}
```

The catalog, coverage and samples need no key:

```rust
use probnode::Client;

#[tokio::main]
async fn main() -> probnode::Result<()> {
    let pn = Client::builder().build()?;
    for d in pn.datasets().await? {
        println!("{}", d.code);
    }
    Ok(())
}
```

## Usage

### The API, method by method

| method | call | key |
|---|---|---|
| `login(email, password)` | `POST /v1/auth/login`; the client uses the key from then on | no |
| `me()`, `usage()` | `GET /v1/me`, `GET /v1/data/usage` | yes |
| `datasets()` | `GET /v1/data/datasets`: every dataset, its formats, tier and hours held | no |
| `series(category, q)` | `GET /v1/catalog/series` | no |
| `markets(&MarketQuery)` | `GET /v1/catalog/markets` (series, category, q, status, limit, offset) | no |
| `coverage(token_id, start, end)` | `GET /v1/catalog/coverage`: the recorded hours of a token | no |
| `samples()` | `GET /v1/samples`: free files with their links | no |
| `files(dataset, format, start, end)` | `GET /v1/data/files`; `.granularity(..)`, `.market(..)`, `.series(..)`, then `.send()` | yes |
| `sign(&paths)` | `POST /v1/data/urls`, batched by 100 | yes |
| `download(&item, dest)` | sign if needed, fetch to `dest/<object_path>.part`, check the sha256, rename | yes* |
| `download_all(&files, dest)` | sign in batches of 100, then download each | yes |
| `export_book(market, day)` | `POST /v1/data/exports`; `.schema(..)`, `.format(..)`, then `.send()` | yes |
| `timeseries(..)`, `timeseries_ndjson(..)` | `GET /v1/timeseries`: one market over a window, parquet bytes or rows | yes |

\* A sample downloads without a key: its link is already signed.

`download` takes anything that is `Downloadable`: a `DataFile` from `files`, a `BookExport` from
`export_book`, or a `Sample`.

### Hourly files

```rust
use probnode::{Client, Format, Granularity};

async fn run() -> probnode::Result<()> {
    let pn = Client::from_env()?;
    let files = pn
        .files("trades", Format::Parquet, "2026-11-12T00:00:00Z", "2026-11-12T06:00:00Z")
        .market("will-x-happen") // repeatable; .series(..) too
        .send()
        .await?;
    for path in pn.download_all(&files, "data/").await? {
        println!("{}", path.display());
    }

    // One file per market and day instead of per hour.
    let days = pn
        .files("trades", Format::Csv, "2026-11-10T00:00:00Z", "2026-11-12T00:00:00Z")
        .granularity(Granularity::MarketDay)
        .series("bitcoin-above")
        .send()
        .await?;
    println!("{} market-day files", days.len());
    Ok(())
}
```

### Market-day schemas

`export_book` builds any of the fifteen market-day schemas (`probnode::MARKET_DAY_SCHEMAS`):
`book-max` (the default), `book-10`, `book-top`, `book-top-1s`, `book-top-1m`, `trades`,
`trade-quotes`, `pair`, `fills`, `bars-1s`, `bars-1m`, `bars-1h`, `bars-1d`, `market-stats` and
`market-events`, as `Format::Parquet` or `Format::Csv` (gzip). The day must have ended (UTC). Each
schema's columns are described in the [schema reference](https://probnode.com/docs/schemas).

### Probe files

With the `probe` feature, the `book_full` dataset's hourly Probe files open directly:

```rust
use probnode::{Client, Format};

async fn run() -> probnode::Result<()> {
    let pn = Client::from_env()?;
    let hours = pn
        .files("book_full", Format::Evmin, "2026-11-12T10:00:00Z", "2026-11-12T11:00:00Z")
        .send()
        .await?;
    for path in pn.download_all(&hours, "data/").await? {
        let f = probnode::probe::open(&path)?;
        println!("{} markets", f.markets().len());
    }
    Ok(())
}
```

### Blocking

```rust
use probnode::blocking::Client;

fn main() -> probnode::Result<()> {
    let pn = Client::from_env()?;
    for d in pn.datasets()? {
        println!("{}", d.code);
    }
    Ok(())
}
```

### The key

`Client::from_env()` takes the key from `PROBNODE_API_KEY`, else from the file `probnode login`
saves: `~/.config/probnode.json` (on Windows `%USERPROFILE%\.config\probnode.json`), or the path in
`PROBNODE_CONFIG`. The base URL comes from `PROBNODE_BASE_URL`, the same file, or
`https://probnode.com`. `Client::new(key)` and `Client::builder()` set them explicitly. To share a
key minted by `login` with the command line, save it with `probnode::Config`:

```rust
async fn run() -> probnode::Result<()> {
    let mut pn = probnode::Client::builder().build()?;
    let key = pn.login("you@example.com", "password").await?;
    probnode::Config { api_key: Some(key), base_url: Some(pn.base_url().into()), ..Default::default() }
        .save()?;
    Ok(())
}
```

Get a key at [probnode.com/login](https://probnode.com/login).

### Errors

Every call returns `probnode::Result<T>`. A refusal from the API is `Error::Api { status,
detail }`, with the HTTP status and the API's own sentence (`402: your plan does not include
book-max`); `err.status()` gives the status of any error that has one. A keyed call made without
a key is `Error::NoApiKey` and sends nothing.

### Examples

```sh
cargo run --example datasets                              # no key needed
cargo run --example export_book -- will-x-happen 2026-11-12 trades csv
```

## Documentation

- ProbNode documentation: [probnode.com/docs](https://probnode.com/docs)
- Schemas, columns and units: [probnode.com/docs/schemas](https://probnode.com/docs/schemas)
- API reference (OpenAPI 3.1): [probnode.com/openapi.json](https://probnode.com/openapi.json)
- Crate documentation: `cargo doc -p probnode --all-features --open` (on docs.rs with the first
  release)

## Versioning and compatibility

- The crate follows [Semantic Versioning](https://semver.org). Until 1.0, a minor version may
  change the API; every change is listed in the [CHANGELOG](https://github.com/probnode/probnode-rs/blob/main/CHANGELOG.md).
- Minimum supported Rust version: **1.85**. Raising it is a minor-version change.
- Tested on Linux, macOS and Windows.
- The client talks to the ProbNode API's `/v1`; fields the API adds later are kept in `extra`, so
  a newer API does not break an older client.

## Support

- Questions about your account, plan or the data: [hello@probnode.com](mailto:hello@probnode.com)
  or the [help centre](https://probnode.com/help).
- Bugs and feature requests for this client: [open an issue](https://github.com/probnode/probnode-rs/issues/new/choose).
- Security reports: privately, as the [security policy](https://github.com/probnode/probnode-rs/blob/main/SECURITY.md) says.

## Contributing

Pull requests are welcome; see [CONTRIBUTING.md](https://github.com/probnode/probnode-rs/blob/main/CONTRIBUTING.md).
This project follows the [Contributor Covenant](https://github.com/probnode/probnode-rs/blob/main/CODE_OF_CONDUCT.md).

## Other ProbNode libraries

| Repository | Language | What it is | Package |
|---|---|---|---|
| [probnode-sdk](https://github.com/probnode/probnode-sdk) | Python | The official Python client, the `probnode` command line and the MCP server | [`probnode-sdk`](https://pypi.org/project/probnode-sdk/), [`probnode-mcp`](https://pypi.org/project/probnode-mcp/) on PyPI |
| [probnode-cpp](https://github.com/probnode/probnode-cpp) | C++17 | The official C++ client, built with CMake | CMake `find_package` or FetchContent |
| [probe](https://github.com/probnode/probe) | Rust, Python | Probe, the lossless order-book file format: specification, library and CLI | [`probe-format`](https://crates.io/crates/probe-format), [`probe-cli`](https://crates.io/crates/probe-cli) on crates.io |

## License

Apache License 2.0. See [LICENSE](https://github.com/probnode/probnode-rs/blob/main/LICENSE) and
[NOTICE](https://github.com/probnode/probnode-rs/blob/main/NOTICE). Copyright 2026 ProbNode. The
license covers the code in this repository; the data you download is licensed under the terms of
your ProbNode plan.
