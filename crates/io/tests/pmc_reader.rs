use std::time::{SystemTime, UNIX_EPOCH};

use opendal::{services::Fs, Operator};
use pmc_io::{MetadataReadError, PmcReadError, PmcReader, ReadError};

const PDF: &[u8] = include_bytes!("../../rs/tests/fixtures/tiny.pdf");

async fn operator(name: &str) -> Operator {
    let root = std::env::temp_dir().join(format!(
        "pmc-reader-{}-{name}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    tokio::fs::create_dir_all(&root).await.unwrap();
    Operator::new(Fs::default().root(root.to_str().unwrap())).unwrap()
}

#[tokio::test]
async fn follows_pdf_url_and_reuses_reader_for_an_article_without_pdf() {
    let operator = operator("resolve").await;
    let uri = "s3://pmc-oa-opendata/PMC123.2/article%20%2B%2520.pdf?md5=example";
    let json = serde_json::json!({"pmcid":"PMC123", "version":2, "pdf_url":uri}).to_string();
    operator
        .write("metadata/article.json", json.clone())
        .await
        .unwrap();
    // Decodes exactly once; '+' is a filename character, not a form-space.
    operator
        .write("PMC123.2/article +%20.pdf", PDF)
        .await
        .unwrap();
    operator
        .write("metadata/no-pdf.json", r#"{"pmcid":"PMC456"}"#)
        .await
        .unwrap();
    let reader = PmcReader::new(operator.clone());
    let record = reader
        .read("metadata/article.json", 1_700_000_000_123_456)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.pdf_bytes(), PDF);
    assert_eq!(record.metadata().source_uri, uri);
    assert_eq!(
        record.metadata().raw_metadata_json.as_deref(),
        Some(json.as_str())
    );
    assert_eq!(record.metadata().retrieved_at, 1_700_000_000_123_456);
    assert_eq!(record.metadata().source_last_modified, None);
    assert!(reader
        .read("metadata/no-pdf.json", 0)
        .await
        .unwrap()
        .is_none());
    operator.delete_with("/").recursive(true).await.unwrap();
}

#[tokio::test]
async fn wrong_bucket_and_ambiguous_paths_are_not_read_from_the_mirror() {
    let operator = operator("paths").await;
    // These objects would otherwise be readable after dropping the wrong
    // bucket or normalizing the path.
    operator.write("article.pdf", PDF).await.unwrap();
    operator.write("PMC123/article.pdf", PDF).await.unwrap();
    let reader = PmcReader::new(operator.clone());
    for uri in [
        "s3://other-bucket/article.pdf",
        "s3://pmc-oa-opendata/PMC123/%2e%2e/article.pdf",
        "s3://pmc-oa-opendata/PMC123//article.pdf",
        "s3://pmc-oa-opendata/article.pdf%20",
    ] {
        let json = serde_json::json!({"pmcid":"PMC123", "pdf_url":uri}).to_string();
        operator.write("metadata/article.json", json).await.unwrap();
        let error = reader.read("metadata/article.json", 0).await.unwrap_err();
        assert!(matches!(error, PmcReadError::InvalidPdfUrl { source_uri } if source_uri == uri));
    }
    operator.delete_with("/").recursive(true).await.unwrap();
}

#[tokio::test]
async fn distinguishes_missing_metadata_from_a_missing_referenced_pdf() {
    let operator = operator("errors").await;
    let reader = PmcReader::new(operator.clone());
    let error = reader.read("metadata/missing.json", 0).await.unwrap_err();
    assert!(
        matches!(error, PmcReadError::Metadata(MetadataReadError::Storage { source_uri, source })
        if source_uri == "metadata/missing.json" && source.kind() == opendal::ErrorKind::NotFound)
    );
    let uri = "s3://pmc-oa-opendata/PMC123.1/missing.pdf?md5=example";
    let json = serde_json::json!({"pmcid":"PMC123", "pdf_url":uri}).to_string();
    operator.write("metadata/article.json", json).await.unwrap();
    let error = reader.read("metadata/article.json", 0).await.unwrap_err();
    assert!(
        matches!(error, PmcReadError::Pdf(ReadError::Storage { source_uri, source })
        if source_uri == uri && source.kind() == opendal::ErrorKind::NotFound)
    );
    operator.delete_with("/").recursive(true).await.unwrap();
}
