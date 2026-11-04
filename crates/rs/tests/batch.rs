use pmc_arrow::{to_record_batch, PdfBatchBuilder, PdfBatchConfig, PdfMetadata, PdfRecord};

const PDF: &[u8] = include_bytes!("fixtures/tiny.pdf");

fn record(pmcid: &str) -> PdfRecord {
    PdfRecord::try_new(PDF.to_vec(), PdfMetadata::new(pmcid, "file:///fixture.pdf")).unwrap()
}

#[test]
fn emits_full_and_partial_batches_and_can_be_reused() {
    let records = [record("PMC123"), record("PMC456"), record("PMC123")];
    let mut builder = PdfBatchBuilder::new(PdfBatchConfig {
        max_rows: 2,
        ..PdfBatchConfig::default()
    })
    .unwrap();

    assert!(builder.append(records[0].clone()).unwrap().is_none());
    let full = builder.append(records[1].clone()).unwrap().unwrap();
    assert_eq!(full, to_record_batch(&records[..2]).unwrap());

    assert!(builder.append(records[2].clone()).unwrap().is_none());
    let tail = builder.flush().unwrap().unwrap();
    assert_eq!(tail, to_record_batch(&records[2..]).unwrap());
    assert!(builder.flush().unwrap().is_none());

    assert!(builder.append(records[0].clone()).unwrap().is_none());
    let reused = builder.append(records[0].clone()).unwrap().unwrap();
    assert_eq!(
        reused,
        to_record_batch(&[records[0].clone(), records[0].clone()]).unwrap()
    );
    assert!(builder.flush().unwrap().is_none());
}

fn small_record(pmcid: &str) -> PdfRecord {
    PdfRecord::try_new(b"%PDF-1.7\n".to_vec(), PdfMetadata::new(pmcid, "x")).unwrap()
}

#[test]
fn flushes_before_exceeding_memory_and_preserves_the_incoming_record() {
    let first = small_record("PMC1");
    let second = small_record("PMC2");
    // Each row has 14 payload bytes and 71 overhead bytes, plus 44 initial offsets.
    // One row estimates 129 bytes; two estimate 214, exceeding this 200-byte limit.
    let mut builder = PdfBatchBuilder::new(PdfBatchConfig {
        max_rows: 10,
        max_memory_bytes: 200,
    })
    .unwrap();
    assert!(builder.append(first.clone()).unwrap().is_none());
    let batch = builder.append(second.clone()).unwrap().unwrap();
    assert_eq!(batch, to_record_batch(&[first]).unwrap());
    assert_eq!(
        builder.flush().unwrap().unwrap(),
        to_record_batch(&[second.clone()]).unwrap()
    );
    assert!(builder.flush().unwrap().is_none());
    // The byte count is reset by flush, so a subsequent row fits again.
    assert!(builder.append(second).unwrap().is_none());
}

#[test]
fn emits_batches_at_exact_memory_and_single_row_limits() {
    for config in [
        PdfBatchConfig {
            max_rows: 10,
            max_memory_bytes: 129,
        },
        PdfBatchConfig {
            max_rows: 1,
            max_memory_bytes: usize::MAX,
        },
    ] {
        let mut builder = PdfBatchBuilder::new(config).unwrap();
        for pmcid in ["PMC1", "PMC2"] {
            let record = small_record(pmcid);
            assert_eq!(
                builder.append(record.clone()).unwrap().unwrap(),
                to_record_batch(&[record]).unwrap()
            );
            assert!(builder.flush().unwrap().is_none());
        }
    }
}

#[test]
fn keeps_an_exact_limit_incoming_row_when_returning_the_previous_batch() {
    let first = small_record("PMC1");
    let mut pdf = b"%PDF-1.7\n".to_vec();
    pdf.resize(180, b' ');
    let incoming = PdfRecord::try_new(pdf, PdfMetadata::new("PMC2", "x")).unwrap();
    // Incoming alone estimates 44 + 71 + 180 + 4 + 1 = 300 bytes.
    let mut builder = PdfBatchBuilder::new(PdfBatchConfig {
        max_rows: 10,
        max_memory_bytes: 300,
    })
    .unwrap();
    assert!(builder.append(first.clone()).unwrap().is_none());
    assert_eq!(
        builder.append(incoming.clone()).unwrap().unwrap(),
        to_record_batch(&[first]).unwrap()
    );
    assert_eq!(
        builder.flush().unwrap().unwrap(),
        to_record_batch(&[incoming]).unwrap()
    );
    assert!(builder.flush().unwrap().is_none());
}

#[test]
fn rejects_oversized_records_without_flushing_buffered_rows() {
    let first = small_record("PMC1");
    let mut pdf = b"%PDF-1.7\n".to_vec();
    pdf.resize(200, b' ');
    let oversized = PdfRecord::try_new(pdf, PdfMetadata::new("PMC2", "x")).unwrap();
    let mut builder = PdfBatchBuilder::new(PdfBatchConfig {
        max_rows: 10,
        max_memory_bytes: 200,
    })
    .unwrap();
    assert!(builder.append(first.clone()).unwrap().is_none());
    assert!(matches!(
        builder.append(oversized),
        Err(pmc_arrow::Error::RecordTooLarge {
            estimated_memory_bytes: 320,
            max_memory_bytes: 200,
        })
    ));
    assert_eq!(
        builder.flush().unwrap().unwrap(),
        to_record_batch(&[first]).unwrap()
    );
    assert!(builder.append(small_record("PMC3")).unwrap().is_none());
}

#[test]
fn counts_all_metadata_strings_toward_the_memory_limit() {
    for field in [
        "pmcid",
        "source_uri",
        "pmid",
        "doi",
        "title",
        "license",
        "raw_metadata_json",
    ] {
        let mut metadata = PdfMetadata::new("PMC1", "x");
        let text = "9".repeat(100);
        match field {
            "pmcid" => metadata.pmcid = text,
            "source_uri" => metadata.source_uri = text,
            "pmid" => metadata.pmid = Some(text),
            "doi" => metadata.doi = Some(text),
            "title" => metadata.title = Some(text),
            "license" => metadata.license = Some(text),
            "raw_metadata_json" => {
                metadata.raw_metadata_json = Some(format!("{{\"extra\":\"{text}\"}}"));
            }
            _ => unreachable!(),
        }
        let record = PdfRecord::try_new(b"%PDF-1.7\n".to_vec(), metadata).unwrap();
        let mut builder = PdfBatchBuilder::new(PdfBatchConfig {
            max_rows: 10,
            max_memory_bytes: 150,
        })
        .unwrap();
        assert!(
            matches!(
                builder.append(record),
                Err(pmc_arrow::Error::RecordTooLarge { .. })
            ),
            "{field} must contribute to the estimate"
        );
    }
}

#[test]
fn rejects_zero_limits() {
    assert!(matches!(
        PdfBatchBuilder::new(PdfBatchConfig {
            max_rows: 0,
            ..PdfBatchConfig::default()
        }),
        Err(pmc_arrow::Error::InvalidBatchSize)
    ));
    assert!(matches!(
        PdfBatchBuilder::new(PdfBatchConfig {
            max_memory_bytes: 0,
            ..PdfBatchConfig::default()
        }),
        Err(pmc_arrow::Error::InvalidBatchMemorySize)
    ));
}
