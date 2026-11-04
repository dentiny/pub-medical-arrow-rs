use futures::{future, Stream, TryStreamExt};
use opendal::Operator;

use crate::PmcReadError;

/// Discover metadata JSON object paths recursively under an operator-relative prefix.
///
/// Use `"metadata/"` for the PMC bucket's metadata directory. Only file entries
/// ending in `.json` are yielded, in backend order. JSON contents are not read.
/// The returned stream advances the OpenDAL lister on demand, without collecting
/// or sorting the entire listing. Dropping it stops further listing; a listing
/// error is yielded once and ends the stream.
///
/// ```no_run
/// use futures::{pin_mut, TryStreamExt};
/// use pmc_io::{discover_metadata, PmcReadError, PmcReader};
///
/// async fn read_prefix(operator: opendal::Operator) -> Result<(), PmcReadError> {
///     let paths = discover_metadata(&operator, "metadata/").await?;
///     let reader = PmcReader::new(operator);
///     let records = reader.read_many(paths, 8);
///     pin_mut!(records);
///     while let Some(record) = records.try_next().await? {
///         // Consume the completed record or append it to a PdfBatchBuilder.
///         let _bytes = record.pdf_bytes();
///     }
///     Ok(())
/// }
/// ```
pub async fn discover_metadata(
    operator: &Operator,
    prefix: &str,
) -> Result<impl Stream<Item = Result<String, PmcReadError>> + Send + 'static, PmcReadError> {
    let lister = operator
        .lister_with(prefix)
        .recursive(true)
        .await
        .map_err(|source| PmcReadError::Discovery {
            prefix: prefix.into(),
            source,
        })?;
    let prefix = prefix.to_owned();
    Ok(lister
        .map_err(move |source| PmcReadError::Discovery {
            prefix: prefix.clone(),
            source,
        })
        .try_filter_map(|entry| {
            let is_json = entry.metadata().is_file() && entry.path().ends_with(".json");
            future::ready(Ok(is_json.then(|| entry.path().to_owned())))
        }))
}
