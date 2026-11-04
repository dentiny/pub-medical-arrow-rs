use percent_encoding::percent_decode_str;

use crate::PmcReadError;

/// Resolve a source PDF URL to its path in the PMC bucket or a matching mirror.
pub(crate) fn validate_pdf_path(source_uri: &str) -> Result<String, PmcReadError> {
    let invalid = || PmcReadError::InvalidPdfUrl {
        source_uri: source_uri.into(),
    };
    let path = source_uri
        .strip_prefix("s3://pmc-oa-opendata/")
        .ok_or_else(invalid)?;
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let path = percent_decode_str(path)
        .decode_utf8()
        .map_err(|_| invalid())?;
    // OpenDAL trims whitespace and collapses empty path segments; filesystem
    // mirrors also interpret dot segments. Reject paths that could change keys.
    if path.trim() != path
        || path.contains('\\')
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(invalid());
    }
    Ok(path.into_owned())
}
