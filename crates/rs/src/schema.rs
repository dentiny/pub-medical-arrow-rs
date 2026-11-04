use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};

/// Fixed 12-column schema for PMC PDF snapshots, including empty batches.
pub fn schema() -> SchemaRef {
    static SCHEMA: OnceLock<SchemaRef> = OnceLock::new();
    Arc::clone(SCHEMA.get_or_init(|| {
        let timestamp = DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into()));
        Arc::new(Schema::new_with_metadata(
            vec![
                Field::new("pmcid", DataType::Utf8, false),
                Field::new("pmc_version", DataType::UInt32, true),
                Field::new("pmid", DataType::Utf8, true),
                Field::new("doi", DataType::Utf8, true),
                Field::new("title", DataType::LargeUtf8, true),
                Field::new("license", DataType::Utf8, true),
                Field::new("source_uri", DataType::Utf8, false),
                Field::new("source_last_modified", timestamp, true),
                Field::new("pdf_sha256", DataType::FixedSizeBinary(32), true),
                Field::new("pdf_size_bytes", DataType::UInt64, false),
                Field::new("pdf_bytes", DataType::LargeBinary, false),
                Field::new("raw_metadata_json", DataType::LargeUtf8, true),
            ],
            HashMap::from([("pmc_arrow.row_kind".into(), "pdf_snapshot".into())]),
        ))
    }))
}
