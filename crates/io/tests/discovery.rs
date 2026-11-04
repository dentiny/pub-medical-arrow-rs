use futures::TryStreamExt;
use opendal::{services::Fs, Operator};
use pmc_io::discover_metadata;
use tempfile::TempDir;

#[tokio::test]
async fn discovers_nested_json_files_under_the_selected_prefix() {
    let root = TempDir::new().unwrap();
    let operator = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
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
}
