use std::{
    collections::BTreeSet,
    io::Cursor,
    time::{SystemTime, UNIX_EPOCH},
};

use arrow_array::{LargeBinaryArray, StringArray};
use arrow_ipc::reader::FileReader;
use tokio::{fs, process::Command};

const PDF: &[u8] = include_bytes!("../../rs/tests/fixtures/tiny.pdf");

#[tokio::test]
async fn converts_a_mirror_into_an_arrow_file() {
    let root = std::env::temp_dir().join(format!(
        "pmc-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mirror = root.join("mirror");
    fs::create_dir_all(mirror.join("metadata")).await.unwrap();
    for id in 1..=3 {
        let path = format!("PMC{id}.1/article.pdf");
        fs::create_dir_all(mirror.join(format!("PMC{id}.1")))
            .await
            .unwrap();
        fs::write(mirror.join(&path), PDF).await.unwrap();
        let metadata = serde_json::json!({
            "pmcid": format!("PMC{id}"),
            "version": 1,
            "pdf_url": format!("s3://pmc-oa-opendata/{path}"),
        });
        fs::write(
            mirror.join(format!("metadata/PMC{id}.1.json")),
            metadata.to_string(),
        )
        .await
        .unwrap();
    }
    fs::write(mirror.join("metadata/no-pdf.json"), r#"{"pmcid":"PMC4"}"#)
        .await
        .unwrap();
    for (name, limits, expected_rows) in [
        ("rows", &["--batch-size", "2"][..], &[2usize, 1][..]),
        (
            "memory",
            &["--max-rows", "32", "--max-memory-bytes", "8192"][..],
            &[1usize, 1, 1][..],
        ),
    ] {
        let output_path = root.join(format!("dataset-{name}.arrow"));
        let output = Command::new(env!("CARGO_BIN_EXE_pmc-arrow"))
            .arg(&mirror)
            .arg(&output_path)
            .args(["--parallelism", "2"])
            .args(limits)
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let file =
            FileReader::try_new(Cursor::new(fs::read(&output_path).await.unwrap()), None).unwrap();
        assert_eq!(file.schema(), pmc_arrow::schema());
        let batches = file.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.num_rows())
                .collect::<Vec<_>>(),
            expected_rows
        );
        let mut ids = BTreeSet::new();
        for batch in batches {
            let pmcids = batch
                .column_by_name("pmcid")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let pdfs = batch
                .column_by_name("pdf_bytes")
                .unwrap()
                .as_any()
                .downcast_ref::<LargeBinaryArray>()
                .unwrap();
            for row in 0..batch.num_rows() {
                ids.insert(pmcids.value(row).to_owned());
                assert_eq!(pdfs.value(row), PDF);
            }
        }
        assert_eq!(
            ids,
            BTreeSet::from(["PMC1".into(), "PMC2".into(), "PMC3".into()])
        );
    }
    let oversized_output_path = root.join("oversized.arrow");
    let output = Command::new(env!("CARGO_BIN_EXE_pmc-arrow"))
        .arg(&mirror)
        .arg(&oversized_output_path)
        .args(["--max-memory-bytes", "1"])
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("max_memory_bytes"));
    assert!(!fs::try_exists(&oversized_output_path).await.unwrap());
    fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn rejects_zero_batch_limits_before_opening_the_source() {
    for (flag, canonical_flag) in [
        ("--max-rows", "--max-rows"),
        ("--batch-size", "--max-rows"),
        ("--max-memory-bytes", "--max-memory-bytes"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pmc-arrow"))
            .args(["unused-mirror", "unused-output.arrow", flag, "0"])
            .output()
            .await
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains(canonical_flag));
    }
}
