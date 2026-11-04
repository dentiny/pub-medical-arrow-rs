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
fn flushes_before_memory_limit_and_preserves_incoming_record() {
    let first = small_record("PMC1");
    let mut pdf = b"%PDF-1.7\n".to_vec();
    pdf.resize(180, b' ');
    let incoming = PdfRecord::try_new(pdf, PdfMetadata::new("PMC2", "x")).unwrap();
    let mut single_row_builder = PdfBatchBuilder::new(PdfBatchConfig {
        max_rows: 1,
        max_memory_bytes: usize::MAX,
    })
    .unwrap();
    let incoming_bytes = single_row_builder
        .append(incoming.clone())
        .unwrap()
        .unwrap()
        .get_array_memory_size();
    let mut builder = PdfBatchBuilder::new(PdfBatchConfig {
        max_rows: 10,
        max_memory_bytes: incoming_bytes,
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
