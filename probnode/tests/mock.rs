//! The client against a mock API (wiremock): requests as the API expects them, answers parsed,
//! errors carried, downloads checked.

use probnode::{Client, Error, Format, Granularity, MarketQuery};
use serde_json::json;
use sha2::{Digest, Sha256};
use wiremock::matchers::{body_json, header, header_exists, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn keyless(server: &MockServer) -> Client {
    Client::builder().base_url(server.uri()).build().unwrap()
}

fn keyed(server: &MockServer) -> Client {
    Client::builder()
        .base_url(format!("{}/", server.uri()))
        .api_key("pn_test")
        .build()
        .unwrap()
}

fn file_json(object_path: &str, body: &[u8]) -> serde_json::Value {
    json!({
        "object_path": object_path, "dataset": "trades", "format": "parquet",
        "event_date": "2026-11-12", "hour": 10, "src": "all", "bytes": body.len(),
        "rows": 3, "sha256": sha(body), "granularity": "hour", "market": null, "era": "v4",
        "new_field": 7
    })
}

#[tokio::test]
async fn datasets_are_public_and_typed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/data/datasets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "code": "book_full", "title": "Full order book", "formats": ["evmin"],
            "market_day_formats": ["parquet", "csv"], "tier": "tape",
            "first_hour": "2026-11-12T06:00:00Z", "last_hour": null, "schema": "book-max",
            "aliases": ["book_full"], "level": "L2", "kind": "event", "blurb": null,
            "tags": [], "legacy": false, "something_new": true
        }])))
        .expect(1)
        .mount(&server)
        .await;
    let ds = keyless(&server).datasets().await.unwrap();
    assert_eq!(ds.len(), 1);
    assert_eq!(ds[0].code, "book_full");
    assert_eq!(ds[0].market_day_formats, vec!["parquet", "csv"]);
    assert_eq!(ds[0].last_hour, None);
    assert_eq!(ds[0].extra["something_new"], json!(true));
    let req = &server.received_requests().await.unwrap()[0];
    assert!(
        req.headers.get("authorization").is_none(),
        "no key, no header"
    );
}

#[tokio::test]
async fn the_key_is_sent_as_bearer() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/data/usage"))
        .and(header("authorization", "Bearer pn_test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plan": "pro", "datasets": ["trades"], "signed_urls_used_today": 3,
            "signed_urls_per_day": 1000, "requests_per_minute": 120,
            "resets_at": "2026-11-13T00:00:00Z", "freshness": "realtime",
            "freshness_cutoff": null
        })))
        .expect(1)
        .mount(&server)
        .await;
    let usage = keyed(&server).usage().await.unwrap();
    assert_eq!(usage.plan, "pro");
    assert_eq!(usage.signed_urls_per_day, 1000);
}

#[tokio::test]
async fn a_keyed_call_without_a_key_sends_nothing() {
    let server = MockServer::start().await;
    let err = keyless(&server).me().await.unwrap_err();
    assert!(matches!(err, Error::NoApiKey));
    assert_eq!(err.status(), Some(401));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn errors_carry_status_and_the_api_sentence() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/data/exports"))
        .respond_with(
            ResponseTemplate::new(402)
                .set_body_json(json!({"detail": "your plan does not include book-max"})),
        )
        .mount(&server)
        .await;
    Mock::given(path("/v1/catalog/coverage"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({"detail": [
            {"type": "missing", "loc": ["query", "token_id"], "msg": "Field required"}
        ]})))
        .mount(&server)
        .await;
    Mock::given(path("/v1/data/datasets"))
        .respond_with(ResponseTemplate::new(503).set_body_string("<html>down</html>"))
        .mount(&server)
        .await;
    let pn = keyed(&server);

    let err = pn.export_book("m", "2026-11-12").send().await.unwrap_err();
    match &err {
        Error::Api { status, detail } => {
            assert_eq!(*status, 402);
            assert_eq!(detail, "your plan does not include book-max");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(err.to_string(), "402: your plan does not include book-max");

    let err = pn.coverage("", None, None).await.unwrap_err();
    assert_eq!(err.status(), Some(422));
    assert!(err.to_string().contains("Field required"), "{err}");

    let err = pn.datasets().await.unwrap_err();
    assert_eq!(err.to_string(), "503: Service Unavailable");
}

#[tokio::test]
async fn catalog_queries() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/catalog/series"))
        .and(query_param("category", "crypto"))
        .and(query_param("q", "bitcoin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"slug": "bitcoin-above", "title": "Bitcoin above", "venue": "polymarket", "category": "crypto"}
        ])))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/catalog/markets"))
        .and(query_param("series", "bitcoin-above"))
        .and(query_param("limit", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "slug": "btc-1", "series_slug": "bitcoin-above", "start_ts": "2026-11-12T00:00:00Z",
            "end_ts": "2026-11-13T00:00:00Z", "resolution": null,
            "tokens": [{"token_id": "1", "outcome": "Yes"}, {"token_id": "2", "outcome": "No"}]
        }])))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/catalog/coverage"))
        .and(query_param("token_id", "1"))
        .and(query_param("start", "2026-11-12T00:00:00Z"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "token_id": "1", "hour_start": "2026-11-12T00:00:00Z", "recorded_ms": 3600000,
            "gap_count": 0, "uptime_pct": 100.0, "implied": false, "derived_ms": 0, "era": "v4",
            "era_split_ms": null, "clock_source": "chrony", "clock_stratum": 2
        }])))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/samples"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"url": "https://example.invalid/s.parquet", "object_path": "s.parquet",
             "dataset": "trades", "format": "parquet", "event_date": "2026-11-12", "hour": 0,
             "src": "all", "bytes": 1, "rows": 1, "sha256": "00", "granularity": "market_day",
             "market": "btc-1", "era": "v4"}
        ])))
        .mount(&server)
        .await;
    let pn = keyless(&server);
    let series = pn.series(Some("crypto"), Some("bitcoin")).await.unwrap();
    assert_eq!(series[0].slug, "bitcoin-above");
    let query = MarketQuery {
        series: Some("bitcoin-above".into()),
        limit: Some(2),
        ..Default::default()
    };
    let markets = pn.markets(&query).await.unwrap();
    assert_eq!(markets[0].tokens[1].outcome, "No");
    let cov = pn
        .coverage("1", Some("2026-11-12T00:00:00Z"), None)
        .await
        .unwrap();
    assert_eq!(cov[0].recorded_ms, 3_600_000);
    assert_eq!(cov[0].clock_stratum, Some(2));
    let samples = pn.samples().await.unwrap();
    assert_eq!(samples[0].file.market.as_deref(), Some("btc-1"));
    assert_eq!(samples[0].url, "https://example.invalid/s.parquet");
}

#[tokio::test]
async fn files_send_every_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/data/files"))
        .and(query_param("dataset", "trades"))
        .and(query_param("format", "csv"))
        .and(query_param("start", "2026-11-12T00:00:00Z"))
        .and(query_param("end", "2026-11-13T00:00:00Z"))
        .and(query_param("granularity", "market_day"))
        .and(header_exists("authorization"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "files": [file_json("trades/a.csv.gz", b"x")], "total_files": 1, "total_bytes": 1
        })))
        .expect(1)
        .mount(&server)
        .await;
    let files = keyed(&server)
        .files(
            "trades",
            Format::Csv,
            "2026-11-12T00:00:00Z",
            "2026-11-13T00:00:00Z",
        )
        .granularity(Granularity::MarketDay)
        .market("m1")
        .market("m2")
        .series("s1")
        .send()
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].hour, 10);
    assert_eq!(files[0].extra["new_field"], json!(7));
    let req = &server.received_requests().await.unwrap()[0];
    let pairs: Vec<(String, String)> = req.url.query_pairs().into_owned().collect();
    let markets: Vec<&str> = pairs
        .iter()
        .filter(|(k, _)| k == "market")
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(markets, vec!["m1", "m2"]);
    assert!(pairs.contains(&("series".into(), "s1".into())));
}

#[tokio::test]
async fn sign_batches_by_one_hundred() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/data/urls"))
        .respond_with(|req: &Request| {
            let body: serde_json::Value = req.body_json().unwrap();
            let paths = body["paths"].as_array().unwrap();
            assert!(paths.len() <= 100);
            let urls: Vec<_> = paths
                .iter()
                .map(|p| {
                    json!({"object_path": p, "url": format!("https://x.invalid/{}", p.as_str().unwrap()),
                           "expires_at": "2026-11-12T10:15:00Z"})
                })
                .collect();
            ResponseTemplate::new(200)
                .set_body_json(json!({"urls": urls, "licence": "l", "licence_detail": "d"}))
        })
        .expect(3)
        .mount(&server)
        .await;
    let paths: Vec<String> = (0..250).map(|i| format!("p/{i}")).collect();
    let urls = keyed(&server).sign(&paths).await.unwrap();
    assert_eq!(urls.len(), 250);
    assert_eq!(urls["p/249"], "https://x.invalid/p/249");
}

async fn serve_file(server: &MockServer, object_path: &str, body: &'static [u8]) {
    let link = format!("{}/store/{object_path}", server.uri());
    Mock::given(method("POST"))
        .and(path("/v1/data/urls"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "urls": [{"object_path": object_path, "url": link, "expires_at": "2026-11-12T10:15:00Z"}],
            "licence": "l", "licence_detail": "d"
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/store/{object_path}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn download_checks_sha256_and_renames() {
    let server = MockServer::start().await;
    let body: &'static [u8] = b"PAR1 some parquet bytes PAR1";
    serve_file(&server, "trades/parquet/2026-11-12/10.parquet", body).await;
    let file: probnode::DataFile =
        serde_json::from_value(file_json("trades/parquet/2026-11-12/10.parquet", body)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let got = keyed(&server).download(&file, dir.path()).await.unwrap();
    assert_eq!(got, dir.path().join("trades/parquet/2026-11-12/10.parquet"));
    assert_eq!(std::fs::read(&got).unwrap(), body);
    assert!(!got.with_file_name("10.parquet.part").exists());

    // The signed link is fetched without the API key.
    let reqs = server.received_requests().await.unwrap();
    let fetch = reqs
        .iter()
        .find(|r| r.url.path().starts_with("/store/"))
        .unwrap();
    assert!(fetch.headers.get("authorization").is_none());
}

#[tokio::test]
async fn a_sha256_mismatch_leaves_nothing() {
    let server = MockServer::start().await;
    serve_file(&server, "t/bad.parquet", b"tampered").await;
    let file: probnode::DataFile =
        serde_json::from_value(file_json("t/bad.parquet", b"original")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let err = keyed(&server)
        .download(&file, dir.path())
        .await
        .unwrap_err();
    match err {
        Error::Checksum {
            object_path,
            expected,
            actual,
        } => {
            assert_eq!(object_path, "t/bad.parquet");
            assert_eq!(expected, sha(b"original"));
            assert_eq!(actual, sha(b"tampered"));
        }
        other => panic!("{other:?}"),
    }
    assert!(!dir.path().join("t/bad.parquet").exists());
    assert!(!dir.path().join("t/bad.parquet.part").exists());

    // verify=false keeps it.
    let got = keyed(&server)
        .download_with(&file, dir.path(), None, false)
        .await
        .unwrap();
    assert_eq!(std::fs::read(got).unwrap(), b"tampered");
}

#[tokio::test]
async fn a_failed_fetch_is_an_error_with_its_status() {
    let server = MockServer::start().await;
    Mock::given(path("/store/gone"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let file: probnode::DataFile = serde_json::from_value(file_json("gone", b"x")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let url = format!("{}/store/gone", server.uri());
    let err = keyed(&server)
        .download_with(&file, dir.path(), Some(&url), true)
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(403));
    assert!(!dir.path().join("gone").exists());
}

#[tokio::test]
async fn download_all_signs_once_for_many() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/data/urls"))
        .and(body_json(json!({"paths": ["a/1", "a/2"]})))
        .respond_with({
            let base = server.uri();
            ResponseTemplate::new(200).set_body_json(json!({
                "urls": [
                    {"object_path": "a/1", "url": format!("{base}/store/1"), "expires_at": "x"},
                    {"object_path": "a/2", "url": format!("{base}/store/2"), "expires_at": "x"}
                ],
                "licence": "l", "licence_detail": "d"
            }))
        })
        .expect(1)
        .mount(&server)
        .await;
    for (i, b) in [(1, "one"), (2, "two")] {
        Mock::given(path(format!("/store/{i}")))
            .respond_with(ResponseTemplate::new(200).set_body_string(b))
            .mount(&server)
            .await;
    }
    let files: Vec<probnode::DataFile> = [("a/1", b"one"), ("a/2", b"two")]
        .iter()
        .map(|(p, b)| serde_json::from_value(file_json(p, *b)).unwrap())
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let got = keyed(&server)
        .download_all(&files, dir.path())
        .await
        .unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(std::fs::read_to_string(&got[1]).unwrap(), "two");
}

#[tokio::test]
async fn export_book_builds_the_request_then_downloads() {
    let server = MockServer::start().await;
    let body: &'static [u8] = b"csv,gz,bytes";
    let object_path = "trades/market_day/2026-11-12/will-x-happen.csv.gz";
    Mock::given(method("POST"))
        .and(path("/v1/data/exports"))
        .and(body_json(json!({
            "market": "will-x-happen", "day": "2026-11-12", "schema": "trades", "format": "csv"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "urls": [{"object_path": object_path,
                      "url": format!("{}/store/{object_path}", server.uri()),
                      "expires_at": "2026-11-13T10:15:00Z"}],
            "licence": "l", "licence_detail": "d", "schema": "trades", "rows": 1234,
            "bytes": body.len(), "sha256": sha(body), "built": true
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path(format!("/store/{object_path}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(&server)
        .await;

    let pn = keyed(&server);
    let day = pn
        .export_book("will-x-happen", "2026-11-12")
        .schema("trades")
        .format(Format::Csv)
        .send()
        .await
        .unwrap();
    assert_eq!(day.rows, 1234u64);
    assert!(day.built);
    let dir = tempfile::tempdir().unwrap();
    let path = pn.download(&day, dir.path()).await.unwrap();
    assert_eq!(path, dir.path().join(object_path));
    assert_eq!(std::fs::read(path).unwrap(), body);
}

#[tokio::test]
async fn export_book_defaults_to_book_max_parquet() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/data/exports"))
        .and(body_json(json!({
            "market": "m", "day": "2026-11-12", "schema": "book-max", "format": "parquet"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "urls": [{"object_path": "x", "url": "https://x.invalid/x", "expires_at": "x"}],
            "licence": "l", "licence_detail": "d", "schema": "book-max", "rows": 1,
            "bytes": 1, "sha256": "00", "built": false
        })))
        .expect(1)
        .mount(&server)
        .await;
    let day = keyed(&server)
        .export_book("m", "2026-11-12")
        .send()
        .await
        .unwrap();
    assert_eq!(day.schema, "book-max");
    assert_eq!(day.object_path(), Some("x"));
}

#[tokio::test]
async fn login_mints_a_key_and_uses_it() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/login"))
        .and(body_json(
            json!({"email": "a@example.com", "password": "correct-horse"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"key": "pn_new", "prefix": "pn_new", "name": "cli"})),
        )
        .mount(&server)
        .await;
    Mock::given(path("/v1/me"))
        .and(header("authorization", "Bearer pn_new"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "6c1f", "email": "a@example.com", "plan": "free", "plan_expires_at": null,
            "is_admin": false, "email_verified": true, "created_at": "2026-11-01T00:00:00Z"
        })))
        .mount(&server)
        .await;
    let mut pn = keyless(&server);
    assert_eq!(
        pn.login("a@example.com", "correct-horse").await.unwrap(),
        "pn_new"
    );
    assert_eq!(pn.me().await.unwrap().plan, "free");
}

#[tokio::test]
async fn timeseries_parquet_and_ndjson() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/timeseries"))
        .and(query_param("format", "ndjson"))
        .and(query_param("market", "m"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("{\"ts\":1,\"px\":0.5}\n{\"ts\":2,\"px\":0.51}\n"),
        )
        .mount(&server)
        .await;
    Mock::given(path("/v1/timeseries"))
        .and(query_param("format", "parquet"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(&b"PAR1"[..]))
        .mount(&server)
        .await;
    let pn = keyed(&server);
    let rows = pn
        .timeseries_ndjson(
            "trades",
            "m",
            "2026-11-12T00:00:00Z",
            "2026-11-12T01:00:00Z",
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1]["px"], json!(0.51));
    let bytes = pn
        .timeseries(
            "trades",
            "m",
            "2026-11-12T00:00:00Z",
            "2026-11-12T01:00:00Z",
        )
        .await
        .unwrap();
    assert_eq!(&bytes[..], b"PAR1");
}

#[tokio::test]
async fn redirects_are_not_followed_with_the_key() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/me"))
        .respond_with(
            ResponseTemplate::new(307).insert_header("location", "https://elsewhere.invalid/"),
        )
        .mount(&server)
        .await;
    let err = keyed(&server).me().await.unwrap_err();
    // A 3xx is not an error status, so the body (empty) fails to parse: nothing was followed.
    assert!(matches!(err, Error::Json(_)), "{err:?}");
}

#[test]
fn config_round_trips_with_the_python_cli_file() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("sub").join("probnode.json");
    assert_eq!(
        probnode::Config::load_from(&p).unwrap(),
        probnode::Config::default()
    );
    // What `probnode login` (Python) writes.
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(
        &p,
        "{\n  \"api_key\": \"pn_saved\",\n  \"base_url\": \"https://probnode.com\"\n}",
    )
    .unwrap();
    let c = probnode::Config::load_from(&p).unwrap();
    assert_eq!(c.api_key.as_deref(), Some("pn_saved"));
    c.save_to(&p).unwrap();
    let again: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    assert_eq!(again["api_key"], "pn_saved");
    assert_eq!(again["base_url"], "https://probnode.com");
}

#[test]
fn every_market_day_schema_is_listed() {
    assert_eq!(probnode::MARKET_DAY_SCHEMAS.len(), 15);
    assert_eq!(probnode::MARKET_DAY_SCHEMAS[0], "book-max");
    assert_eq!(Format::Parquet.to_string(), "parquet");
}
