use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use arrow_array::{LargeStringArray, StringArray};
use futures::io::{AsyncRead, AsyncReadExt, Cursor};
use opendal::{services::Fs, Operator};
use pmc_arrow::to_record_batch;
use pmc_io::{read_metadata, MetadataReadError, MetadataReader, PdfReader};

const JSON: &str = include_str!("fixtures/metadata.json");
const PDF: &[u8] = include_bytes!("../../rs/tests/fixtures/tiny.pdf");
const METADATA_URI: &str = "s3://pmc-oa-opendata/metadata/PMC123.2.json";
const PDF_URI: &str =
    "s3://pmc-oa-opendata/PMC123.2/article.pdf?md5=0123456789abcdef0123456789abcdef";

fn operator() -> Operator {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    Operator::new(Fs::default().root(root.to_str().unwrap())).unwrap()
}

#[tokio::test]
async fn filesystem_metadata_maps_into_pdf_and_arrow_without_losing_source_json() {
    let source = read_metadata(&operator(), "metadata.json").await.unwrap();
    assert_eq!(source.raw_metadata_json, JSON);
    let metadata = source
        .into_pdf_metadata(1_700_000_000_123_456, Some(1_699_999_000_000_000))
        .unwrap();
    assert_eq!(metadata.pmcid, "PMC123");
    assert_eq!(metadata.pmc_version, Some(2));
    assert_eq!(metadata.pmid.as_deref(), Some("36537061"));
    assert_eq!(metadata.doi.as_deref(), Some("10.0000/example"));
    assert_eq!(metadata.title.as_deref(), Some("Synthetic 医学 article"));
    assert_eq!(metadata.license.as_deref(), Some("CC BY"));
    assert_eq!(metadata.source_uri, PDF_URI);
    assert_eq!(metadata.retrieved_at, 1_700_000_000_123_456);
    assert_eq!(metadata.source_last_modified, Some(1_699_999_000_000_000));

    let record = PdfReader::new(Cursor::new(PDF), metadata)
        .read()
        .await
        .unwrap();
    let batch = to_record_batch(&[record]).unwrap();
    let raw = batch
        .column_by_name("raw_metadata_json")
        .unwrap()
        .as_any()
        .downcast_ref::<LargeStringArray>()
        .unwrap();
    assert_eq!(raw.value(0), JSON);
    let uri = batch
        .column_by_name("source_uri")
        .unwrap()
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert_eq!(uri.value(0), PDF_URI);
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
        assert_eq!(source.version, None);
        assert_eq!(source.pmid, None);
        assert_eq!(source.pdf_url, None);
        assert!(source.into_pdf_metadata(0, None).is_none());
    }
    let json = r#"{"pmcid":"PMC123","pmid":"00042","pdf_url":"s3://example/a.pdf"}"#;
    let source = MetadataReader::new(Cursor::new(json), METADATA_URI)
        .read()
        .await
        .unwrap();
    let metadata = source.into_pdf_metadata(0, None).unwrap();
    assert_eq!(metadata.pmid.as_deref(), Some("00042"));
    assert_eq!(metadata.pmc_version, None);
    assert_eq!(metadata.source_last_modified, None);
}

#[tokio::test]
async fn malformed_json_and_mapped_field_types_keep_source_context() {
    for json in [
        r#"{"pmcid":"PMC123""#,
        r#"{"pmcid":"PMC123"} {}"#,
        r#"{"pmcid":"PMC123","pmid":1.5}"#,
        r#"{"pmcid":"PMC123","version":"1"}"#,
        r#"{"pmcid":"PMC123","pdf_url":false}"#,
        r#"{"pmcid":"PMC123","pmcid":"PMC456"}"#,
        r#"{"title":"missing identity"}"#,
    ] {
        let error = MetadataReader::new(Cursor::new(json), METADATA_URI)
            .read()
            .await
            .unwrap_err();
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
