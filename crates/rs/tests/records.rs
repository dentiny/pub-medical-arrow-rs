use arrow_schema::{DataType, TimeUnit};
use pmc_arrow::{schema, Error, PdfMetadata, PdfRecord, SCHEMA_VERSION};

const PDF: &[u8] = include_bytes!("fixtures/tiny.pdf");
const RETRIEVED: i64 = 1_793_731_234_567_890;

fn metadata() -> PdfMetadata {
    PdfMetadata::new("PMC123", "s3://example/PMC123.2/PMC123.2.pdf", RETRIEVED)
}

fn record() -> PdfRecord {
    PdfRecord::try_new(PDF.to_vec(), metadata()).unwrap()
}

#[test]
fn fixed_schema_contract() {
    let timestamp = DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into()));
    let expected = [
        ("pmcid", DataType::Utf8, false),
        ("pmc_version", DataType::UInt32, true),
        ("pmid", DataType::Utf8, true),
        ("doi", DataType::Utf8, true),
        ("title", DataType::LargeUtf8, true),
        ("license", DataType::Utf8, true),
        ("source_uri", DataType::Utf8, false),
        ("source_last_modified", timestamp.clone(), true),
        ("retrieved_at", timestamp, false),
        ("pdf_sha256", DataType::FixedSizeBinary(32), false),
        ("pdf_size_bytes", DataType::UInt64, false),
        ("pdf_bytes", DataType::LargeBinary, false),
        ("raw_metadata_json", DataType::LargeUtf8, true),
    ];
    let schema = schema();
    assert_eq!(schema.fields().len(), expected.len());
    for (field, (name, data_type, nullable)) in schema.fields().iter().zip(expected) {
        assert_eq!(field.name(), name);
        assert_eq!(field.data_type(), &data_type);
        assert_eq!(field.is_nullable(), nullable);
    }
    assert_eq!(
        schema.metadata()["pmc_arrow.schema_version"],
        SCHEMA_VERSION
    );
    assert_eq!(schema.metadata()["pmc_arrow.row_kind"], "pdf_snapshot");
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
