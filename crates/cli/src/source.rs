use anyhow::{bail, Context, Result};
use opendal::{services, Operator};

pub async fn open(source: &str) -> Result<Operator> {
    match source {
        "s3://pmc-oa-opendata" | "s3://pmc-oa-opendata/" => {
            opendal::install_default();
            Ok(Operator::new(
                services::S3::default()
                    .bucket("pmc-oa-opendata")
                    .region("us-east-1")
                    .skip_signature(),
            )?)
        }
        source if source.contains("://") => {
            bail!("unsupported source {source:?}; use a local mirror directory or s3://pmc-oa-opendata")
        }
        source => {
            let root = tokio::fs::canonicalize(source)
                .await
                .with_context(|| format!("cannot open mirror directory {source:?}"))?;
            Ok(Operator::new(services::Fs::default().root(
                root.to_str().context("mirror directory must be UTF-8")?,
            ))?)
        }
    }
}
