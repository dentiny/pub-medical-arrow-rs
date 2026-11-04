use futures::{stream, Stream, StreamExt};
use pmc_arrow::PdfRecord;

use crate::{PmcReadError, PmcReader};

impl PmcReader {
    /// Read article paths concurrently, yielding PDF records in input order.
    ///
    /// At most `parallelism` articles are pending; zero is treated as one.
    /// Articles without PDFs are skipped. The first input or read error is
    /// yielded in input order, then the stream ends and pending reads are dropped.
    /// Dropping the stream also drops pending reads; no tasks are spawned.
    /// Construction performs no I/O, and reads advance only while polled.
    ///
    /// ```no_run
    /// use futures::{pin_mut, stream, StreamExt};
    /// use pmc_io::{PmcReadError, PmcReader};
    ///
    /// async fn read_articles(reader: &PmcReader) -> Result<(), PmcReadError> {
    ///     let paths = stream::iter([
    ///         Ok("metadata/PMC123.1.json".to_owned()),
    ///         Ok("metadata/PMC456.1.json".to_owned()),
    ///     ]);
    ///     let records = reader.read_many(paths, 4);
    ///     pin_mut!(records);
    ///     while let Some(record) = records.next().await {
    ///         let record = record?;
    ///         // Consume each record before requesting more.
    ///         let _bytes = record.pdf_bytes();
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub fn read_many<'a, S>(
        &'a self,
        paths: S,
        parallelism: usize,
    ) -> impl Stream<Item = Result<PdfRecord, PmcReadError>> + 'a
    where
        S: Stream<Item = Result<String, PmcReadError>> + 'a,
    {
        let reads = paths
            .map(move |path| async move {
                let path = path?;
                self.read(&path).await
            })
            .buffered(parallelism.max(1));
        stream::try_unfold(Box::pin(reads), |mut reads| async move {
            while let Some(result) = reads.next().await {
                if let Some(record) = result? {
                    return Ok(Some((record, reads)));
                }
            }
            Ok(None)
        })
    }
}
