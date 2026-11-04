use crate::{Error, PdfBatchConfig, PdfRecord, Result};

pub(crate) fn validate_config(config: &PdfBatchConfig) -> Result<()> {
    if config.max_rows == 0 {
        return Err(Error::InvalidBatchSize);
    }
    if config.max_memory_bytes == 0 {
        return Err(Error::InvalidBatchMemorySize);
    }
    Ok(())
}

/// Validate Arrow offset and allocation capacities for a batch.
///
/// Returns the total PDF payload size to preallocate its LargeBinary buffer.
/// Allocation exhaustion is not caught.
pub(crate) fn validate_batch_capacity(records: &[PdfRecord]) -> Result<usize> {
    // Check offset capacities before invoking builders whose infallible append
    // methods may panic on offset overflow. Allocation exhaustion is not caught.
    let pdf_capacity = checked_total(
        records.iter().map(|r| r.pdf_bytes().len()),
        i64::MAX as u64,
        "pdf_bytes",
    )?;
    checked_total(
        records.iter().map(|r| r.metadata().pmcid.len()),
        i32::MAX as u64,
        "pmcid",
    )?;
    checked_total(
        records.iter().map(|r| r.metadata().source_uri.len()),
        i32::MAX as u64,
        "source_uri",
    )?;
    for (name, get) in [
        (
            "pmid",
            (|r: &PdfRecord| r.metadata().pmid.as_deref()) as fn(&PdfRecord) -> Option<&str>,
        ),
        ("doi", |r: &PdfRecord| r.metadata().doi.as_deref()),
        ("license", |r: &PdfRecord| r.metadata().license.as_deref()),
    ] {
        checked_total(
            records.iter().map(|r| get(r).map_or(0, str::len)),
            i32::MAX as u64,
            name,
        )?;
    }
    for (name, get) in [
        (
            "title",
            (|r: &PdfRecord| r.metadata().title.as_deref()) as fn(&PdfRecord) -> Option<&str>,
        ),
        ("raw_metadata_json", |r: &PdfRecord| {
            r.metadata().raw_metadata_json.as_deref()
        }),
    ] {
        checked_total(
            records.iter().map(|r| get(r).map_or(0, str::len)),
            i64::MAX as u64,
            name,
        )?;
    }
    checked_total([records.len(), 1], (isize::MAX as u64) / 8, "row offsets")?;
    Ok(pdf_capacity)
}

fn checked_total(
    lengths: impl IntoIterator<Item = usize>,
    limit: u64,
    field: &'static str,
) -> Result<usize> {
    lengths.into_iter().try_fold(0usize, |sum, length| {
        sum.checked_add(length)
            .filter(|&n| n as u64 <= limit && n <= isize::MAX as usize)
            .ok_or(Error::BatchTooLarge(field))
    })
}
