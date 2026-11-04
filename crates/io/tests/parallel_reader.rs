use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{SystemTime, UNIX_EPOCH};

use futures::{pin_mut, stream, StreamExt, TryStreamExt};
use opendal::{services::Fs, Operator};
use pmc_io::PmcReader;

const PDF: &[u8] = include_bytes!("../../rs/tests/fixtures/tiny.pdf");

#[tokio::test]
async fn reads_ordered_records_with_bounded_prefetch_and_skips_articles_without_pdf() {
    let root = std::env::temp_dir().join(format!(
        "pmc-parallel-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    tokio::fs::create_dir_all(&root).await.unwrap();
    let operator = Operator::new(Fs::default().root(root.to_str().unwrap())).unwrap();
    for id in [3, 1, 2] {
        let json = serde_json::json!({
            "pmcid": format!("PMC{id}"),
            "pdf_url": format!("s3://pmc-oa-opendata/PMC{id}.1/article.pdf")
        })
        .to_string();
        operator
            .write(&format!("metadata/PMC{id}.1.json"), json)
            .await
            .unwrap();
        operator
            .write(&format!("PMC{id}.1/article.pdf"), PDF)
            .await
            .unwrap();
    }
    operator
        .write("metadata/no-pdf.json", r#"{"pmcid":"PMC4"}"#)
        .await
        .unwrap();
    let requested = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&requested);
    let paths = stream::iter([
        "metadata/PMC3.1.json",
        "metadata/PMC1.1.json",
        "metadata/no-pdf.json",
        "metadata/PMC2.1.json",
    ])
    .map(move |path| {
        counted.fetch_add(1, Ordering::SeqCst);
        Ok(path.to_owned())
    });
    let reader = PmcReader::new(operator.clone());
    let records = reader.read_many(paths, 2);
    assert_eq!(requested.load(Ordering::SeqCst), 0);
    pin_mut!(records);
    let first = records.next().await.unwrap().unwrap();
    assert_eq!(first.metadata().pmcid, "PMC3");
    assert_eq!(first.pdf_bytes(), PDF);
    assert_eq!(requested.load(Ordering::SeqCst), 2);
    let remaining: Vec<_> = records.try_collect().await.unwrap();
    assert_eq!(
        remaining
            .iter()
            .map(|r| r.metadata().pmcid.as_str())
            .collect::<Vec<_>>(),
        ["PMC1", "PMC2"]
    );
    assert!(remaining.iter().all(|r| r.pdf_bytes() == PDF));
    assert_eq!(requested.load(Ordering::SeqCst), 4);
    operator.delete_with("/").recursive(true).await.unwrap();
}
