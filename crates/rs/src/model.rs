use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// A SHA-256 digest stored as 32 raw bytes.
pub type Sha256Digest = [u8; 32];

/// Caller-supplied metadata for one PDF snapshot.
///
/// Timestamps are signed microseconds since the Unix epoch in UTC. They are
/// supplied explicitly so conversions are deterministic. The optional PMC
/// version is a source version, not a content hash or a ranking of versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfMetadata {
    /// Positive decimal ID, optionally prefixed with PMC (case insensitive).
    pub pmcid: String,
    /// Unknown versions remain null; zero is rejected.
    pub pmc_version: Option<u32>,
    /// Source-supplied PubMed identifier, preserved as text.
    pub pmid: Option<String>,
    /// Source-supplied DOI, preserved without URL or case rewriting.
    pub doi: Option<String>,
    pub title: Option<String>,
    /// Source-supplied license label; this crate does not infer permissions.
    pub license: Option<String>,
    /// Original PDF location. Any nonblank value is accepted, including local paths.
    pub source_uri: String,
    /// PDF source object's last-modified time in signed microseconds since the
    /// Unix epoch (1970-01-01 00:00:00 UTC). `None` means the time is unknown.
    pub source_last_modified: Option<i64>,
    /// Caller-supplied PDF retrieval time in signed microseconds since the
    /// Unix epoch (1970-01-01 00:00:00 UTC).
    pub retrieved_at: i64,
    /// A valid JSON object, preserved verbatim (including whitespace and unknown fields).
    /// No provider-specific field mapping is performed in this crate.
    pub raw_metadata_json: Option<String>,
}

impl PdfMetadata {
    /// Create metadata with all optional fields absent.
    pub fn new(pmcid: impl Into<String>, source_uri: impl Into<String>, retrieved_at: i64) -> Self {
        Self {
            pmcid: pmcid.into(),
            pmc_version: None,
            pmid: None,
            doi: None,
            title: None,
            license: None,
            source_uri: source_uri.into(),
            source_last_modified: None,
            retrieved_at,
            raw_metadata_json: None,
        }
    }
}

/// A validated, immutable PDF snapshot with a computed SHA-256 digest.
///
/// Fields are private to prevent bytes, length and digest from diverging after
/// validation. Construction takes ownership of the PDF vector without copying.
/// Arrow conversion copies the bytes into a contiguous column buffer.
#[derive(Clone)]
pub struct PdfRecord {
    metadata: PdfMetadata,
    pdf_bytes: Vec<u8>,
    pdf_sha256: Sha256Digest,
}

impl std::fmt::Debug for PdfRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PdfRecord")
            .field("metadata", &self.metadata)
            .field("pdf_size_bytes", &self.pdf_size_bytes())
            .field("pdf_sha256", &self.pdf_sha256)
            .finish_non_exhaustive()
    }
}

impl PdfRecord {
    /// Validate inputs, canonicalize the PMCID, and hash the original PDF bytes.
    ///
    /// Header validation requires `%PDF-x.y` at byte zero followed by CR or LF.
    /// It rejects obvious HTML/error responses but does not parse objects, xref,
    /// encryption, signatures or EOF markers and cannot detect truncated PDFs.
    pub fn try_new(pdf_bytes: Vec<u8>, mut metadata: PdfMetadata) -> Result<Self> {
        metadata.pmcid = normalize_pmcid(&metadata.pmcid)?;
        if metadata.pmc_version == Some(0) {
            return Err(Error::InvalidVersion);
        }
        if metadata.source_uri.trim().is_empty() {
            return Err(Error::MissingSourceUri);
        }
        if let Some(json) = &metadata.raw_metadata_json {
            // RawValue validates syntax without changing numbers, whitespace or keys.
            let raw: &serde_json::value::RawValue = serde_json::from_str(json)?;
            if !raw.get().starts_with('{') {
                return Err(Error::MetadataNotObject);
            }
        }
        if pdf_bytes.is_empty() {
            return Err(Error::EmptyPdf);
        }
        if pdf_bytes.len() < 9
            || !pdf_bytes.starts_with(b"%PDF-")
            || !pdf_bytes[5].is_ascii_digit()
            || pdf_bytes[6] != b'.'
            || !pdf_bytes[7].is_ascii_digit()
            || !matches!(pdf_bytes[8], b'\r' | b'\n')
        {
            return Err(Error::InvalidPdfHeader);
        }
        let pdf_sha256 = Sha256::digest(&pdf_bytes).into();
        Ok(Self {
            metadata,
            pdf_bytes,
            pdf_sha256,
        })
    }

    pub fn metadata(&self) -> &PdfMetadata {
        &self.metadata
    }

    pub fn pdf_bytes(&self) -> &[u8] {
        &self.pdf_bytes
    }

    pub fn pdf_sha256(&self) -> &Sha256Digest {
        &self.pdf_sha256
    }

    pub fn pdf_size_bytes(&self) -> u64 {
        self.pdf_bytes.len() as u64
    }
}

fn normalize_pmcid(value: &str) -> Result<String> {
    let trimmed = value.trim();
    let digits = if trimmed
        .get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("PMC"))
    {
        &trimmed[3..]
    } else {
        trimmed
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidPmcid(value.to_owned()));
    }
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Err(Error::InvalidPmcid(value.to_owned()));
    }
    Ok(format!("PMC{digits}"))
}
