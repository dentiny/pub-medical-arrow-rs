use std::collections::HashMap;

use pmc_arrow::PdfMetadata;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::value::RawValue;

use crate::MetadataReadError;

/// Original PMC article-version JSON, including articles without PDFs.
///
/// Article fields are extracted only when converting to `PdfMetadata`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmcMetadata {
    /// Complete source JSON, preserving whitespace and unknown fields verbatim.
    pub raw_metadata_json: String,
    source_uri: String,
}

impl PmcMetadata {
    /// Extract PDF metadata directly from the source JSON.
    ///
    /// Missing or null `pdf_url` returns `Ok(None)`. Invalid mapped field types
    /// return an error with the metadata source location.
    /// `source_last_modified` is signed microseconds since the Unix epoch in UTC.
    /// It describes the PDF, not the JSON object or inventory
    /// entry; pass `None` when unknown. No clock or stat is read.
    pub fn into_pdf_metadata(
        self,
        source_last_modified: Option<i64>,
    ) -> Result<Option<PdfMetadata>, MetadataReadError> {
        (|| -> serde_json::Result<Option<PdfMetadata>> {
            // Borrow raw values so unknown fields need no typed representation.
            let fields: HashMap<String, &RawValue> = serde_json::from_str(&self.raw_metadata_json)?;
            let Some(source_uri) = field(&fields, "pdf_url")? else {
                return Ok(None);
            };
            Ok(Some(PdfMetadata {
                pmcid: field(&fields, "pmcid")?,
                pmc_version: field(&fields, "version")?,
                pmid: field::<Option<Pmid>>(&fields, "pmid")?.map(|pmid| match pmid {
                    Pmid::Text(text) => text,
                    Pmid::Number(number) => number.to_string(),
                }),
                doi: field(&fields, "doi")?,
                title: field(&fields, "title")?,
                license: field(&fields, "license_code")?,
                source_uri,
                source_last_modified,
                raw_metadata_json: Some(self.raw_metadata_json),
            }))
        })()
        .map_err(|source| MetadataReadError::InvalidJson {
            source_uri: self.source_uri,
            source,
        })
    }
}

fn field<T: DeserializeOwned>(
    fields: &HashMap<String, &RawValue>,
    name: &str,
) -> serde_json::Result<T> {
    serde_json::from_str(fields.get(name).map_or("null", |value| value.get()))
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
    let raw: &RawValue = serde_json::from_str(&raw_metadata_json).map_err(|source| {
        MetadataReadError::InvalidJson {
            source_uri: source_uri.into(),
            source,
        }
    })?;
    if !raw.get().starts_with('{') {
        return Err(MetadataReadError::NotObject {
            source_uri: source_uri.into(),
        });
    }
    Ok(PmcMetadata {
        raw_metadata_json,
        source_uri: source_uri.into(),
    })
}
