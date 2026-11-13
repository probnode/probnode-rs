# Changelog

All notable changes to the `probnode` crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Until 1.0, a minor version may change
the API.

## [Unreleased]

## [0.1.0] - 2026-11-13

First release of the official Rust client.

### Added

- `Client` (async, reqwest with rustls): `login`, `me`, `usage`; `datasets`, `series`, `markets`,
  `coverage`, `samples`; `files` (hourly or `Granularity::MarketDay`, narrowed by market and
  series); `sign` (batched by 100); `download` and `download_all` (a `.part` file, the sha256
  checked, then renamed); `export_book` (any of the fifteen market-day schemas, parquet or
  csv.gz); `timeseries` (parquet bytes) and `timeseries_ndjson`.
- Typed models, with every field they do not name kept in `extra`.
- `Error::Api` carries the HTTP status and the API's sentence; `Error::Checksum` names both
  digests.
- The key from an argument, `PROBNODE_API_KEY`, or the file `probnode login` saves
  (`~/.config/probnode.json`, shared with the Python command line).
- Feature `blocking`: `blocking::Client` with the same methods.
- Feature `probe`: `probe::open` for the `book_full` dataset's Probe hours, through
  `probe-format` 0.1 from crates.io.

[Unreleased]: https://github.com/probnode/probnode-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/probnode/probnode-rs/releases/tag/v0.1.0
