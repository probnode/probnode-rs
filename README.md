<p align="center">
  <a href="https://probnode.com"><img src="assets/probnode-logo.png" alt="ProbNode" width="96" height="96"></a>
</p>

<h1 align="center">probnode-rs</h1>

<p align="center">
  <a href="https://github.com/probnode/probnode-rs/actions/workflows/ci.yml"><img src="https://github.com/probnode/probnode-rs/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="Apache-2.0"></a>
</p>

The official Rust client for [ProbNode](https://probnode.com): prediction-market order books at
full depth, trades, top-of-book grids, bars and market statistics, delivered as parquet, csv.gz
and [Probe](https://github.com/probnode/probe) files with a sha256 for every file.

The same API has an official [Python client](https://github.com/probnode/probnode-sdk) and an
official [C++ client](https://github.com/probnode/probnode-cpp).

## Install

```sh
cargo add probnode
cargo add tokio --features macros,rt-multi-thread    # the client is async
```

Features:

| feature | what it adds |
|---|---|
| `blocking` | `probnode::blocking::Client`, the same methods without async |
| `probe` | `probnode::probe::open`, to read the `book_full` dataset's Probe (`.evmin`) hours with [`probe-format`](https://crates.io/crates/probe-format) |

TLS is rustls; nothing links OpenSSL.

## Quick start

```rust
use probnode::{Client, Format};

#[tokio::main]
async fn main() -> probnode::Result<()> {
    let pn = Client::from_env()?;              // PROBNODE_API_KEY, or the key `probnode login` saved

    // One market's UTC day of the full book: made on the first request, then found.
    let day = pn
        .export_book("will-x-happen", "2026-11-12")
        .schema("book-max")
        .format(Format::Parquet)
        .send()
        .await?;
    let path = pn.download(&day, "data/").await?;   // sha256-checked; returns the PathBuf
    println!("{} rows in {}", day.rows, path.display());
    Ok(())
}
```

## The API, method by method

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
`export_book`, or a `Sample`. The file is written to `<object_path>.part` and renamed only when
complete and its sha256 is the one the API listed; on a mismatch the partial file is removed and
`Error::Checksum` is returned.

### Hourly files

```rust
use probnode::{Client, Format, Granularity};

async fn run() -> probnode::Result<()> {
let pn = Client::from_env()?;
let files = pn
    .files("trades", Format::Parquet, "2026-11-12T00:00:00Z", "2026-11-12T06:00:00Z")
    .market("will-x-happen")                   // repeatable; .series(..) too
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
Ok(())
}
```

### Market-day schemas

`export_book` builds any of the fifteen market-day schemas (`probnode::MARKET_DAY_SCHEMAS`):
`book-max` (the default), `book-10`, `book-top`, `book-top-1s`, `book-top-1m`, `trades`,
`trade-quotes`, `pair`, `fills`, `bars-1s`, `bars-1m`, `bars-1h`, `bars-1d`, `market-stats` and
`market-events`, as `Format::Parquet` or `Format::Csv` (gzip). The day must have ended (UTC). Each
schema's columns are described in the [schema reference](https://probnode.com/docs).

### Probe files

With the `probe` feature, the `book_full` dataset's hourly Probe files open directly:

```rust
async fn run() -> probnode::Result<()> {
use probnode::{Client, Format};

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

## The key

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

## Errors

Every call returns `probnode::Result<T>`. A refusal from the API is `Error::Api { status,
detail }`, with the HTTP status and the API's own sentence (`402: your plan does not include
book-max`); `err.status()` gives the status of any error that has one. A keyed call made without
a key is `Error::NoApiKey` and sends nothing.

## Examples

```sh
cargo run --example datasets                              # no key needed
cargo run --example export_book -- will-x-happen 2026-11-12 trades csv
```

## Minimum supported Rust version

1.85.

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
