use std::time::{SystemTime, UNIX_EPOCH};

use opendal::{services::Fs, Operator};
use pmc_arrow::PdfRecordOptions;
use pmc_io::PmcReader;

const PDF: &[u8] = include_bytes!("../../rs/tests/fixtures/tiny.pdf");

async fn operator() -> Operator {
    let root = std::env::temp_dir().join(format!(
        "pmc-reader-{}-{}",
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
async fn reads_pdf_referenced_by_metadata() {
    let operator = operator().await;
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
    let reader = PmcReader::new(operator.clone());
    let record = reader.read("metadata/article.json").await.unwrap().unwrap();
    assert_eq!(record.pdf_sha256(), None);
    let hashed = reader
        .with_options(PdfRecordOptions {
            compute_pdf_sha256: true,
        })
        .read("metadata/article.json")
        .await
        .unwrap()
        .unwrap();
    let digest: String = hashed
        .pdf_sha256()
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest,
        include_str!("../../rs/tests/fixtures/tiny.sha256").trim()
    );
    assert_eq!(record.pdf_bytes(), PDF);
    assert_eq!(record.metadata().source_uri, uri);
    assert_eq!(
        record.metadata().raw_metadata_json.as_deref(),
        Some(json.as_str())
    );
    assert_eq!(record.metadata().source_last_modified, None);
    operator.delete_with("/").recursive(true).await.unwrap();
}
