use std::{
    collections::BTreeSet,
    io::Cursor,
    time::{SystemTime, UNIX_EPOCH},
};

use arrow_array::{Array, FixedSizeBinaryArray, LargeBinaryArray, StringArray};
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
    for compute_sha256 in [false, true] {
        let output_path = root.join(format!("dataset-{compute_sha256}.arrow"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_pmc-arrow"));
        command
            .arg(&mirror)
            .arg(&output_path)
            .args(["--parallelism", "2", "--batch-size", "2"]);
        if compute_sha256 {
            command.arg("--compute-pdf-sha256");
        }
        let output = command.output().await.unwrap();
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
            [2, 1]
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
            let hashes = batch
                .column_by_name("pdf_sha256")
                .unwrap()
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .unwrap();
            for row in 0..batch.num_rows() {
                ids.insert(pmcids.value(row).to_owned());
                assert_eq!(pdfs.value(row), PDF);
                assert_eq!(hashes.is_null(row), !compute_sha256);
                if compute_sha256 {
                    let digest: String = hashes
                        .value(row)
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect();
                    assert_eq!(
                        digest,
                        include_str!("../../rs/tests/fixtures/tiny.sha256").trim()
                    );
                }
            }
        }
        assert_eq!(
            ids,
            BTreeSet::from(["PMC1".into(), "PMC2".into(), "PMC3".into()])
        );
    }
    fs::remove_dir_all(root).await.unwrap();
}
