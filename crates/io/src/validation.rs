use crate::ReadError;

/// Validate the payload limit and return the byte count including the EOF probe.
pub(crate) fn validate_limit(limit: usize) -> Result<u64, ReadError> {
    if limit >= isize::MAX as usize {
        return Err(ReadError::InvalidSizeLimit);
    }
    Ok(limit as u64 + 1)
}
