use arrow_array::{
    Array, FixedSizeBinaryArray, LargeBinaryArray, LargeStringArray, RecordBatch, StringArray,
    TimestampMicrosecondArray, UInt32Array, UInt64Array,
};
use pmc_arrow::{schema, to_record_batch, Error, PdfMetadata, PdfRecord};

const PDF: &[u8] = include_bytes!("fixtures/tiny.pdf");
const LAST_MODIFIED: i64 = 1_793_731_234_567_889;

fn metadata() -> PdfMetadata {
    PdfMetadata::new("PMC123", "s3://example/PMC123.2/PMC123.2.pdf")
}

fn record() -> PdfRecord {
    PdfRecord::try_new(PDF.to_vec(), metadata()).unwrap()
}

fn column<'a, T: Array + 'static>(batch: &'a RecordBatch, name: &str) -> &'a T {
    batch
        .column_by_name(name)
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap()
}

#[test]
fn empty_batch_retains_schema() {
    let batch = to_record_batch(&[]).unwrap();
    assert_eq!(batch.schema(), schema());
    assert_eq!(batch.num_rows(), 0);
    assert_eq!(batch.num_columns(), 12);
}

#[test]
fn preserves_all_values_nulls_binary_and_microseconds() {
    let raw = " \n{\"unmapped\": [true, null], \"n\": 123456789012345678901234567890, \"title\": \"original\"}\n";
    let mut full = metadata();
    full.pmc_version = Some(2);
    full.pmid = Some("000123".into());
    full.doi = Some("10.1234/MixedCase".into());
    full.title = Some("免疫研究 — αβ 🧬".into());
    full.license = Some("CC BY 4.0".into());
    full.source_last_modified = Some(LAST_MODIFIED);
    full.raw_metadata_json = Some(raw.into());
    let complete = PdfRecord::try_new(PDF.to_vec(), full.clone()).unwrap();
    let batch = to_record_batch(&[complete, record()]).unwrap();
    assert_eq!(batch.schema(), schema());
    assert_eq!(batch.num_rows(), 2);
    assert_eq!(column::<StringArray>(&batch, "pmcid").value(0), "PMC123");
    assert_eq!(column::<UInt32Array>(&batch, "pmc_version").value(0), 2);
    for (name, expected) in [
        ("pmid", full.pmid.as_deref().unwrap()),
        ("doi", full.doi.as_deref().unwrap()),
        ("license", full.license.as_deref().unwrap()),
        ("source_uri", full.source_uri.as_str()),
    ] {
        assert_eq!(column::<StringArray>(&batch, name).value(0), expected);
    }
    assert_eq!(
        column::<LargeStringArray>(&batch, "title").value(0),
        full.title.unwrap()
    );
    assert_eq!(
        column::<LargeStringArray>(&batch, "raw_metadata_json").value(0),
        raw
    );
    assert_eq!(
        column::<TimestampMicrosecondArray>(&batch, "source_last_modified").value(0),
        LAST_MODIFIED
    );
    for name in [
        "pmc_version",
        "pmid",
        "doi",
        "title",
        "license",
        "source_last_modified",
        "raw_metadata_json",
    ] {
        assert!(batch.column_by_name(name).unwrap().is_null(1), "{name}");
    }
    assert!(
        std::str::from_utf8(column::<LargeBinaryArray>(&batch, "pdf_bytes").value(0)).is_err(),
        "fixture must exercise non-UTF8 bytes"
    );
    for row in 0..2 {
        assert_eq!(
            column::<LargeBinaryArray>(&batch, "pdf_bytes").value(row),
            PDF
        );
        assert_eq!(
            column::<UInt64Array>(&batch, "pdf_size_bytes").value(row),
            PDF.len() as u64
        );
        for (field, array) in batch.schema().fields().iter().zip(batch.columns()) {
            if !field.is_nullable() {
                assert!(!array.is_null(row), "{}", field.name());
            }
        }
    }
    assert_eq!(
        column::<FixedSizeBinaryArray>(&batch, "pdf_sha256").value(0),
        record().pdf_sha256()
    );
}

#[test]
fn digest_matches_independent_fixture_checksum() {
    // Generated independently with Python hashlib, stored alongside the fixture.
    let expected = include_str!("fixtures/tiny.sha256").trim();
    let record = record();
    let actual: String = record
        .pdf_sha256()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(record.pdf_size_bytes(), PDF.len() as u64);
    assert_eq!(record.pdf_bytes(), PDF);
}

#[test]
fn pmcid_normalization_is_deterministic_and_not_integer_bounded() {
    for input in ["PMC123", "pmc123", "PmC00123", "123", " 000123\n"] {
        let mut meta = metadata();
        meta.pmcid = input.into();
        assert_eq!(
            PdfRecord::try_new(PDF.to_vec(), meta)
                .unwrap()
                .metadata()
                .pmcid,
            "PMC123"
        );
    }
    let input = "PMC123456789012345678901234567890";
    let mut meta = metadata();
    meta.pmcid = input.into();
    assert_eq!(
        PdfRecord::try_new(PDF.to_vec(), meta)
            .unwrap()
            .metadata()
            .pmcid,
        input
    );
}

#[test]
fn rejects_invalid_identifiers_without_panicking_on_unicode() {
    for input in [
        "",
        " ",
        "PMC",
        "PMC0",
        "000",
        "PMC-1",
        "PMC1.2",
        "PMC 123",
        "PMC１２３",
        "🧬",
        "é1",
        "123abc",
    ] {
        let mut meta = metadata();
        meta.pmcid = input.into();
        assert!(
            matches!(
                PdfRecord::try_new(PDF.to_vec(), meta),
                Err(Error::InvalidPmcid(_))
            ),
            "{input:?}"
        );
    }
}

#[test]
fn validates_pdf_header_without_claiming_full_pdf_validation() {
    assert!(matches!(
        PdfRecord::try_new(vec![], metadata()),
        Err(Error::EmptyPdf)
    ));
    for input in [
        b"<html>404</html>".as_slice(),
        b"%PDF-",
        b"%PDF-1.7",
        b"%PDF-a.b\n",
        b"%PDF-1.7x",
        b" %PDF-1.7\n",
    ] {
        assert!(matches!(
            PdfRecord::try_new(input.to_vec(), metadata()),
            Err(Error::InvalidPdfHeader)
        ));
    }
    for input in [b"%PDF-1.7\n".as_slice(), b"%PDF-2.0\r\n", b"%PDF-1.4\r"] {
        assert!(PdfRecord::try_new(input.to_vec(), metadata()).is_ok());
    }
}

#[test]
fn validates_json_but_retains_original_bytes_and_unknown_fields() {
    for json in ["", "{", "{\"bad\":NaN}", "{} trailing", "{}{}"] {
        let mut meta = metadata();
        meta.raw_metadata_json = Some(json.into());
        assert!(
            matches!(
                PdfRecord::try_new(PDF.to_vec(), meta),
                Err(Error::InvalidMetadataJson(_))
            ),
            "{json}"
        );
    }
    for json in ["null", "[]", "42", "\"string\"", "true"] {
        let mut meta = metadata();
        meta.raw_metadata_json = Some(json.into());
        assert!(
            matches!(
                PdfRecord::try_new(PDF.to_vec(), meta),
                Err(Error::MetadataNotObject)
            ),
            "{json}"
        );
    }
    let json = " \r\n{\"duplicate\":1,\"duplicate\":2,\"huge\":1e9999} \n";
    let mut meta = metadata();
    meta.raw_metadata_json = Some(json.into());
    assert_eq!(
        PdfRecord::try_new(PDF.to_vec(), meta)
            .unwrap()
            .metadata()
            .raw_metadata_json
            .as_deref(),
        Some(json)
    );
}

#[test]
fn keeps_empty_strings_distinct_from_null_and_does_not_invent_versions() {
    let mut meta = metadata();
    meta.title = Some(String::new());
    meta.raw_metadata_json = Some("{}".into());
    let batch = to_record_batch(&[PdfRecord::try_new(PDF.to_vec(), meta).unwrap()]).unwrap();
    let title = column::<LargeStringArray>(&batch, "title");
    assert!(!title.is_null(0));
    assert_eq!(title.value(0), "");
    assert!(column::<UInt32Array>(&batch, "pmc_version").is_null(0));
}

#[test]
fn preserves_order_duplicates_and_content_revisions() {
    let first = record();
    let mut updated_pdf = PDF.to_vec();
    updated_pdf.extend_from_slice(b"\n% subsequent revision\n");
    let updated = PdfRecord::try_new(updated_pdf, metadata()).unwrap();
    assert_ne!(first.pdf_sha256(), updated.pdf_sha256());
    let mut other_metadata = metadata();
    other_metadata.pmcid = "PMC456".into();
    let other = PdfRecord::try_new(PDF.to_vec(), other_metadata).unwrap();
    assert_eq!(first.pdf_sha256(), other.pdf_sha256());
    let batch = to_record_batch(&[updated.clone(), other, first.clone(), first]).unwrap();
    assert_eq!(batch.num_rows(), 4);
    assert_eq!(
        column::<StringArray>(&batch, "pmcid")
            .iter()
            .collect::<Vec<_>>(),
        vec![
            Some("PMC123"),
            Some("PMC456"),
            Some("PMC123"),
            Some("PMC123")
        ]
    );
    assert_eq!(
        column::<LargeBinaryArray>(&batch, "pdf_bytes").value(0),
        updated.pdf_bytes()
    );
    assert_eq!(
        column::<LargeBinaryArray>(&batch, "pdf_bytes").value(3),
        PDF
    );
}

#[test]
fn preserves_signed_utc_microsecond_timestamps() {
    let mut meta = metadata();
    meta.source_last_modified = Some(-1);
    let batch = to_record_batch(&[PdfRecord::try_new(PDF.to_vec(), meta).unwrap()]).unwrap();
    assert_eq!(
        column::<TimestampMicrosecondArray>(&batch, "source_last_modified").value(0),
        -1
    );
}

#[test]
fn arrow_ipc_round_trip_preserves_schema_and_every_column() {
    use arrow_ipc::{reader::StreamReader, writer::StreamWriter};
    let mut meta = metadata();
    meta.pmc_version = Some(3);
    meta.pmid = Some("456".into());
    meta.doi = Some("10.1234/Example".into());
    meta.title = Some("医学 — αβ".into());
    meta.license = Some("CC-BY-4.0".into());
    meta.source_last_modified = Some(-1);
    meta.raw_metadata_json = Some(" {\"unknown\":1e9999} \n".into());
    let full = PdfRecord::try_new(PDF.to_vec(), meta).unwrap();
    for batch in [
        to_record_batch(&[]).unwrap(),
        to_record_batch(&[record(), full]).unwrap(),
    ] {
        let mut bytes = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut bytes, &batch.schema()).unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }
        let mut reader = StreamReader::try_new(std::io::Cursor::new(bytes), None).unwrap();
        let restored = reader.next().unwrap().unwrap();
        assert_eq!(restored, batch);
        assert!(reader.next().is_none());
    }
}
