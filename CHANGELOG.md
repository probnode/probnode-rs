# Changelog

The `probnode` crate follows semantic versioning. Until 1.0, a minor version may change the API.

## 0.1.0 - unreleased

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
- Feature `probe`: `probe::open` for the `book_full` dataset's Probe hours.
