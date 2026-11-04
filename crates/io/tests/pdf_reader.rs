use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use arrow_array::LargeBinaryArray;
use futures::io::{AsyncRead, AsyncReadExt, Cursor};
use opendal::{services::Fs, Operator};
use pmc_arrow::{to_record_batch, PdfMetadata, PdfRecordOptions};
use pmc_io::{read_pdf, read_pdf_with_options, PdfReader, ReadError};

const PDF: &[u8] = include_bytes!("../../rs/tests/fixtures/tiny.pdf");
const SOURCE_URI: &str = "file:///fixtures/tiny.pdf";

fn metadata() -> PdfMetadata {
    let mut metadata = PdfMetadata::new("pmc00123", SOURCE_URI);
    metadata.title = Some("医学 αβ".into());
    metadata.raw_metadata_json = Some(" {\"unmapped\":1e9999} \n".into());
    metadata
}

fn operator() -> Operator {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../rs/tests/fixtures");
    Operator::new(Fs::default().root(root.to_str().unwrap())).unwrap()
}

#[tokio::test]
async fn filesystem_pdf_preserves_bytes_and_metadata_through_arrow_conversion() {
    let record = read_pdf(&operator(), "tiny.pdf", metadata()).await.unwrap();
    assert_eq!(record.pdf_bytes(), PDF);
    assert_eq!(record.pdf_size_bytes(), PDF.len() as u64);
    assert_eq!(record.metadata().pmcid, "PMC123");
    assert_eq!(record.metadata().source_uri, SOURCE_URI);
    assert_eq!(record.metadata().source_last_modified, None);
    assert_eq!(record.metadata().title, metadata().title);
    assert_eq!(
        record.metadata().raw_metadata_json,
        metadata().raw_metadata_json
    );
    assert_eq!(record.pdf_sha256(), None);
    let batch = to_record_batch(&[record]).unwrap();
    let bytes = batch
        .column_by_name("pdf_bytes")
        .unwrap()
        .as_any()
        .downcast_ref::<LargeBinaryArray>()
        .unwrap();
    assert_eq!(bytes.value(0), PDF);
}

#[tokio::test]
async fn borrowed_stream_reads_complete_pdf() {
    let mut input = Cursor::new(PDF);
    let record = PdfReader::new(&mut input, metadata()).read().await.unwrap();
    assert_eq!(record.pdf_bytes(), PDF);
    assert_eq!(record.pdf_sha256(), None);
    assert_eq!(input.position(), PDF.len() as u64);
}

#[tokio::test]
async fn storage_and_stream_readers_compute_sha256_only_when_requested() {
    let options = PdfRecordOptions {
        compute_pdf_sha256: true,
    };
    let stored = read_pdf_with_options(&operator(), "tiny.pdf", metadata(), options)
        .await
        .unwrap();
    let streamed = PdfReader::new(Cursor::new(PDF), metadata())
        .with_options(options)
        .read()
        .await
        .unwrap();
    for record in [stored, streamed] {
        let digest: String = record
            .pdf_sha256()
            .unwrap()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            digest,
            include_str!("../../rs/tests/fixtures/tiny.sha256").trim()
        );
    }
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
async fn interrupted_stream_does_not_turn_partial_bytes_into_a_record() {
    let input = Cursor::new(PDF).chain(FailingReader);
    match PdfReader::new(input, metadata()).read().await.unwrap_err() {
        ReadError::Read { source_uri, source } => {
            assert_eq!(source_uri, SOURCE_URI);
            assert_eq!(source.kind(), io::ErrorKind::ConnectionReset);
        }
        error => panic!("unexpected error: {error}"),
    }
}

#[tokio::test]
async fn missing_object_and_invalid_payload_keep_source_context() {
    let error = read_pdf(&operator(), "missing.pdf", metadata())
        .await
        .unwrap_err();
    match error {
        ReadError::Storage { source_uri, source } => {
            assert_eq!(source_uri, SOURCE_URI);
            assert_eq!(source.kind(), opendal::ErrorKind::NotFound);
        }
        error => panic!("unexpected error: {error}"),
    }
    match PdfReader::new(Cursor::new(b"<html>upstream error</html>"), metadata())
        .read()
        .await
        .unwrap_err()
    {
        ReadError::InvalidRecord { source_uri, source } => {
            assert_eq!(source_uri, SOURCE_URI);
            assert!(matches!(source, pmc_arrow::Error::InvalidPdfHeader));
        }
        error => panic!("unexpected error: {error}"),
    }
}
