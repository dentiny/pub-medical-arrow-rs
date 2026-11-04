use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::io::{AsyncRead, AsyncReadExt, Cursor};
use opendal::{services::Fs, Operator};
use pmc_arrow::PdfMetadata;
use pmc_io::{read_metadata, MetadataReadError, MetadataReader};

const JSON: &str = include_str!("fixtures/metadata.json");
const METADATA_URI: &str = "s3://pmc-oa-opendata/metadata/PMC123.2.json";

fn operator() -> Operator {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    Operator::new(Fs::default().root(root.to_str().unwrap())).unwrap()
}

#[tokio::test]
async fn filesystem_and_stream_map_every_pdf_metadata_field() {
    let sources = [
        read_metadata(&operator(), "metadata.json").await.unwrap(),
        MetadataReader::new(Cursor::new(JSON), METADATA_URI)
            .read()
            .await
            .unwrap(),
    ];
    for source in sources {
        assert_eq!(source.raw_metadata_json, JSON);
        let metadata = source.into_pdf_metadata().unwrap().unwrap();
        assert_eq!(
            metadata,
            PdfMetadata {
                pmcid: "PMC123".into(),
                pmc_version: Some(2),
                pmid: Some("36537061".into()),
                doi: Some("10.0000/example".into()),
                title: Some("Synthetic 医学 article".into()),
                license: Some("CC BY".into()),
                source_uri:
                    "s3://pmc-oa-opendata/PMC123.2/article.pdf?md5=0123456789abcdef0123456789abcdef"
                        .into(),
                raw_metadata_json: Some(JSON.into()),
            }
        );
    }
}

#[tokio::test]
async fn missing_and_null_optional_article_fields_remain_absent() {
    for json in [
        r#"{"pmcid":"PMC123","pdf_url":"s3://example/a.pdf"}"#,
        r#"{"pmcid":"PMC123","version":null,"pmid":null,"doi":null,"title":null,"license_code":null,"pdf_url":"s3://example/a.pdf"}"#,
    ] {
        let source = MetadataReader::new(Cursor::new(json), METADATA_URI)
            .read()
            .await
            .unwrap();
        assert_eq!(
            source.into_pdf_metadata().unwrap().unwrap(),
            PdfMetadata {
                raw_metadata_json: Some(json.into()),
                ..PdfMetadata::new("PMC123", "s3://example/a.pdf")
            }
        );
    }
}

#[tokio::test]
async fn stream_preserves_optional_fields_and_handles_articles_without_pdfs() {
    for json in [
        r#"{"pmcid":"PMC123"}"#,
        r#"{"pmcid":"PMC123","version":null,"pmid":null,"pdf_url":null}"#,
    ] {
        let mut input = Cursor::new(json);
        let source = MetadataReader::new(&mut input, METADATA_URI)
            .read()
            .await
            .unwrap();
        assert_eq!(input.position(), json.len() as u64);
        assert_eq!(source.raw_metadata_json, json);
        assert!(source.into_pdf_metadata().unwrap().is_none());
    }
    let json =
        r#"{"\u0070mcid":"PMC123","pmid":"42","pmid":"00042","pdf_url":"s3://example/a.pdf"}"#;
    let source = MetadataReader::new(Cursor::new(json), METADATA_URI)
        .read()
        .await
        .unwrap();
    let metadata = source.into_pdf_metadata().unwrap().unwrap();
    assert_eq!(metadata.pmid.as_deref(), Some("00042"));
    assert_eq!(metadata.pmc_version, None);
}

#[tokio::test]
async fn malformed_json_and_mapped_field_types_keep_source_context() {
    for json in [r#"{"pmcid":"PMC123""#, r#"{"pmcid":"PMC123"} {}"#] {
        let error = MetadataReader::new(Cursor::new(json), METADATA_URI)
            .read()
            .await
            .unwrap_err();
        assert!(
            matches!(error, MetadataReadError::InvalidJson { source_uri, .. } if source_uri == METADATA_URI),
            "input: {json}"
        );
    }
    // JSON reading succeeds; mapped field types are checked at conversion.
    for json in [
        r#"{"pmcid":"PMC123","pmid":1.5,"pdf_url":"s3://example/a.pdf"}"#,
        r#"{"pmcid":"PMC123","version":"1","pdf_url":"s3://example/a.pdf"}"#,
        r#"{"pmcid":"PMC123","pdf_url":false}"#,
        r#"{"title":"missing identity","pdf_url":"s3://example/a.pdf"}"#,
    ] {
        let source = MetadataReader::new(Cursor::new(json), METADATA_URI)
            .read()
            .await
            .unwrap();
        let error = source.into_pdf_metadata().unwrap_err();
        assert!(
            matches!(error, MetadataReadError::InvalidJson { source_uri, .. } if source_uri == METADATA_URI),
            "input: {json}"
        );
    }
    for json in ["[]", "null", "42"] {
        let error = MetadataReader::new(Cursor::new(json), METADATA_URI)
            .read()
            .await
            .unwrap_err();
        assert!(
            matches!(error, MetadataReadError::NotObject { source_uri } if source_uri == METADATA_URI)
        );
    }
    let error = MetadataReader::new(Cursor::new([0xff]), METADATA_URI)
        .read()
        .await
        .unwrap_err();
    assert!(
        matches!(error, MetadataReadError::InvalidEncoding { source_uri, .. } if source_uri == METADATA_URI)
    );
}

struct FailingReader;

impl AsyncRead for FailingReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "source interrupted",
        )))
    }
}

#[tokio::test]
async fn read_failures_do_not_return_partial_metadata() {
    let input = Cursor::new(JSON).chain(FailingReader);
    match MetadataReader::new(input, METADATA_URI)
        .read()
        .await
        .unwrap_err()
    {
        MetadataReadError::Read { source_uri, source } => {
            assert_eq!(source_uri, METADATA_URI);
            assert_eq!(source.kind(), io::ErrorKind::ConnectionReset);
        }
        error => panic!("unexpected error: {error}"),
    }
    match read_metadata(&operator(), "missing.json")
        .await
        .unwrap_err()
    {
        MetadataReadError::Storage { source_uri, source } => {
            assert_eq!(source_uri, "missing.json");
            assert_eq!(source.kind(), opendal::ErrorKind::NotFound);
        }
        error => panic!("unexpected error: {error}"),
    }
}
