use std::time::{SystemTime, UNIX_EPOCH};

use futures::TryStreamExt;
use opendal::{services::Fs, Operator};
use pmc_io::discover_metadata;

#[tokio::test]
async fn discovers_nested_json_files_under_the_selected_prefix() {
    let root = std::env::temp_dir().join(format!(
        "pmc-discovery-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    tokio::fs::create_dir_all(&root).await.unwrap();
    let operator = Operator::new(Fs::default().root(root.to_str().unwrap())).unwrap();
    for path in [
        "metadata/PMC123.1.json",
        "metadata/nested/PMC456.2.json",
        "metadata/notes.txt",
        "outside.json",
    ] {
        operator.write(path, "{}").await.unwrap();
    }
    operator
        .create_dir("metadata/directory.json/")
        .await
        .unwrap();

    let mut paths: Vec<_> = discover_metadata(&operator, "metadata/")
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();
    paths.sort();
    assert_eq!(
        paths,
        ["metadata/PMC123.1.json", "metadata/nested/PMC456.2.json"]
    );
    operator.delete_with("/").recursive(true).await.unwrap();
}
