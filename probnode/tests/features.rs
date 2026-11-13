//! The optional features: the blocking client and the Probe reader.

#[cfg(feature = "blocking")]
#[test]
fn blocking_client_lists_and_exports() {
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/data/datasets"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "code": "trades", "title": "Trades", "formats": ["parquet"],
                "market_day_formats": [], "tier": "free", "first_hour": null, "last_hour": null
            }])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/data/exports"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "urls": [{"object_path": "e/day.parquet",
                          "url": format!("{}/store/day", server.uri()), "expires_at": "x"}],
                "licence": "l", "licence_detail": "d", "schema": "book-top", "rows": 9,
                "bytes": 3,
                "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
                "built": false
            })))
            .mount(&server)
            .await;
        Mock::given(path("/store/day"))
            .respond_with(ResponseTemplate::new(200).set_body_string("abc"))
            .mount(&server)
            .await;
        server
    });

    let inner = probnode::Client::builder()
        .base_url(server.uri())
        .api_key("pn_test")
        .build()
        .unwrap();
    let pn = probnode::blocking::Client::from_async(inner).unwrap();
    assert_eq!(pn.datasets().unwrap()[0].code, "trades");
    let day = pn
        .export_book("m", "2026-11-12")
        .schema("book-top")
        .send()
        .unwrap();
    assert_eq!(day.rows, 9);
    let dir = tempfile::tempdir().unwrap();
    let path = pn.download(&day, dir.path()).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "abc");
}

#[cfg(feature = "probe")]
#[test]
fn probe_open_refuses_a_file_that_is_not_probe() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.evmin");
    std::fs::write(&p, b"not a probe file at all, just some bytes").unwrap();
    let err = probnode::probe::open(&p).unwrap_err();
    assert!(matches!(err, probnode::Error::Probe(_)), "{err:?}");
}
