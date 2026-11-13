//! The client against a running API, keyless endpoints only. Skipped unless
//! `PROBNODE_LIVE_BASE_URL` is set (for example `https://probnode.com`), so offline runs pass.
//!
//! ```sh
//! PROBNODE_LIVE_BASE_URL=https://probnode.com cargo test --test live
//! ```

use probnode::{Client, Error, Format, MarketQuery};

fn live() -> Option<Client> {
    let url = std::env::var("PROBNODE_LIVE_BASE_URL")
        .ok()
        .filter(|u| !u.is_empty())?;
    // No key on purpose: these are the public endpoints.
    Some(Client::builder().base_url(url).build().unwrap())
}

macro_rules! live_or_skip {
    () => {
        match live() {
            Some(c) => c,
            None => {
                eprintln!("skipped: PROBNODE_LIVE_BASE_URL is not set");
                return;
            }
        }
    };
}

#[tokio::test]
async fn live_datasets() {
    let pn = live_or_skip!();
    let ds = pn.datasets().await.unwrap();
    assert!(!ds.is_empty());
    let full = ds
        .iter()
        .find(|d| d.code == "book_full")
        .expect("book_full listed");
    assert!(full.formats.iter().any(|f| f == "evmin"));
    assert!(full.market_day_formats.iter().any(|f| f == "parquet"));
}

#[tokio::test]
async fn live_catalog() {
    let pn = live_or_skip!();
    let series = pn.series(None, Some("bitcoin")).await.unwrap();
    assert!(series.iter().all(|s| !s.slug.is_empty()));
    let markets = pn
        .markets(&MarketQuery {
            limit: Some(3),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(!markets.is_empty() && markets.len() <= 3);
    let token = markets
        .iter()
        .flat_map(|m| m.tokens.iter())
        .next()
        .expect("a market with tokens");
    let hours = pn.coverage(&token.token_id, None, None).await.unwrap();
    assert!(hours.iter().all(|h| h.token_id == token.token_id));
}

#[tokio::test]
async fn live_samples() {
    let pn = live_or_skip!();
    let samples = pn.samples().await.unwrap();
    if let Some(s) = samples.iter().min_by_key(|s| s.file.bytes) {
        if s.file.bytes < 20_000_000 {
            let dir = tempfile::tempdir().unwrap();
            let path = pn.download(s, dir.path()).await.unwrap();
            assert_eq!(std::fs::metadata(path).unwrap().len(), s.file.bytes);
        }
    }
}

#[tokio::test]
async fn live_files_need_a_key() {
    let pn = live_or_skip!();
    // Without a key the client refuses before sending; the API's own 401 is checked by a raw
    // request with a bad key.
    let err = pn
        .files(
            "trades",
            Format::Parquet,
            "2026-11-12T00:00:00Z",
            "2026-11-12T01:00:00Z",
        )
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, Error::NoApiKey));
    let bad = Client::builder()
        .base_url(pn.base_url())
        .api_key("pn_not_a_key")
        .build()
        .unwrap();
    let err = bad
        .files(
            "trades",
            Format::Parquet,
            "2026-11-12T00:00:00Z",
            "2026-11-12T01:00:00Z",
        )
        .send()
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(401), "{err}");
}
