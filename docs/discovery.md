# Discover metadata paths

`discover_metadata(&operator, "metadata/").await` returns a stream of
`Result<String, PmcReadError>` containing operator-relative JSON file paths.
The operator must support recursive listing; backend configuration remains with
the caller. A different prefix can select a smaller subtree or a matching mirror.

Discovery uses OpenDAL's streaming lister, ignores directories and names without
the `.json` suffix, and preserves backend order. It does not read JSON contents,
collect all paths, sort them or remove duplicates. A listing error includes the
requested prefix, is emitted once and ends the stream. Dropping the stream stops
further listing; backend pagination and buffering remain backend-controlled.

Pass the path stream directly to `PmcReader::read_many`:

```rust
let paths = discover_metadata(&operator, "metadata/").await?;
let reader = PmcReader::new(operator);
let records = reader.read_many(paths, parallelism);
```

Records arrive as reads complete, without an input-order guarantee. Feed each
record into `PdfBatchBuilder::append`, consume returned batches, and call
`flush()` when the record stream ends to retrieve the partial final batch.
Listing does not select article versions or filter licenses.

The `ingest` example connects all three stages without collecting the entire
path or record stream:

```sh
cargo run --locked -p pmc-io --example ingest -- /path/to/pmc-mirror metadata/ 8 128
```

It prints batch summaries; it does not write a dataset.

This API lists live storage objects. Parsing PMC inventory reports is separate
work; it is not needed when the caller already has a path stream.
