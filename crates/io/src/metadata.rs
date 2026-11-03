use pmc_arrow::PdfMetadata;
use serde::Deserialize;

use crate::MetadataReadError;

/// Source metadata for one PMC article version, including versions without PDFs.
///
/// Fields use the PMC JSON names. Unknown fields remain in `raw_metadata_json`.
/// PMCID normalization and record validation occur when constructing a `PdfRecord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmcMetadata {
    pub pmcid: String,
    pub version: Option<u32>,
    /// Numeric source PMIDs become decimal strings; string PMIDs stay unchanged.
    pub pmid: Option<String>,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub license_code: Option<String>,
    /// Original PDF URL, including any checksum query parameter. Absent without a PDF.
    pub pdf_url: Option<String>,
    /// Complete source JSON, preserving whitespace and unknown fields verbatim.
    pub raw_metadata_json: String,
}

impl PmcMetadata {
    /// Map source fields to a PDF snapshot, returning `None` when `pdf_url` is absent.
    ///
    /// Both timestamps are signed microseconds since the Unix epoch in UTC.
    /// `source_last_modified` must describe the PDF object, not this JSON object
    /// or its inventory entry; pass `None` when unknown. No clock or stat is read.
    pub fn into_pdf_metadata(
        self,
        retrieved_at: i64,
        source_last_modified: Option<i64>,
    ) -> Option<PdfMetadata> {
        Some(PdfMetadata {
            pmcid: self.pmcid,
            pmc_version: self.version,
            pmid: self.pmid,
            doi: self.doi,
            title: self.title,
            license: self.license_code,
            source_uri: self.pdf_url?,
            source_last_modified,
            retrieved_at,
            raw_metadata_json: Some(self.raw_metadata_json),
        })
    }
}

#[derive(Deserialize)]
struct MetadataFields {
    pmcid: String,
    version: Option<u32>,
    pmid: Option<Pmid>,
    doi: Option<String>,
    title: Option<String>,
    license_code: Option<String>,
    pdf_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Pmid {
    Text(String),
    Number(u64),
}

pub(crate) fn parse_metadata(
    bytes: Vec<u8>,
    source_uri: &str,
) -> Result<PmcMetadata, MetadataReadError> {
    let raw_metadata_json =
        String::from_utf8(bytes).map_err(|source| MetadataReadError::InvalidEncoding {
            source_uri: source_uri.into(),
            source,
        })?;
    let json_error = |source| MetadataReadError::InvalidJson {
        source_uri: source_uri.into(),
        source,
    };
    // Validate the complete JSON without converting unknown numeric values to f64.
    let raw: &serde_json::value::RawValue =
        serde_json::from_str(&raw_metadata_json).map_err(&json_error)?;
    if !raw.get().starts_with('{') {
        return Err(MetadataReadError::NotObject {
            source_uri: source_uri.into(),
        });
    }
    let fields: MetadataFields = serde_json::from_str(raw.get()).map_err(json_error)?;
    Ok(PmcMetadata {
        pmcid: fields.pmcid,
        version: fields.version,
        pmid: fields.pmid.map(|pmid| match pmid {
            Pmid::Text(text) => text,
            Pmid::Number(number) => number.to_string(),
        }),
        doi: fields.doi,
        title: fields.title,
        license_code: fields.license_code,
        pdf_url: fields.pdf_url,
        raw_metadata_json,
    })
}
