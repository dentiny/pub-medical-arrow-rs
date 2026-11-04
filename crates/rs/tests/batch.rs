use pmc_arrow::{to_record_batch, PdfBatchBuilder, PdfMetadata, PdfRecord};

const PDF: &[u8] = include_bytes!("fixtures/tiny.pdf");

fn record(pmcid: &str) -> PdfRecord {
    PdfRecord::try_new(PDF.to_vec(), PdfMetadata::new(pmcid, "file:///fixture.pdf")).unwrap()
}

#[test]
fn emits_full_and_partial_batches_and_can_be_reused() {
    let records = [record("PMC123"), record("PMC456"), record("PMC123")];
    let mut builder = PdfBatchBuilder::new(2).unwrap();

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
