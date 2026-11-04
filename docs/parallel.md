# Concurrent article reads

`PmcReader::read_many(paths, parallelism)` accepts a stream of
`Result<String, PmcReadError>` containing operator-relative metadata paths and
returns a stream of PDF records. An existing list can be supplied with
`futures::stream::iter(paths.into_iter().map(Ok))`.

```rust
use futures::{pin_mut, stream, StreamExt};

let paths = stream::iter([
    Ok("metadata/PMC123.1.json".to_owned()),
    Ok("metadata/PMC456.1.json".to_owned()),
]);
let records = reader.read_many(paths, 4);
pin_mut!(records);
while let Some(record) = records.next().await {
    let record = record?;
    // Consume the record or append it to a batch.
}
```

Different articles are read concurrently; each article's metadata is read before
its PDF. Results retain input order, including duplicate paths. Articles with no
PDF URL are skipped. The first input or read error is emitted in input order,
then the stream ends and pending read futures are dropped.

Construction performs no I/O. Polling fills at most `parallelism` pending article
slots (zero means one), including completed records waiting for an earlier one.
A slow consumer therefore stops further scheduling. Dropping the stream drops
pending futures; this implementation does not spawn background tasks.

Each PDF is still materialized in memory, and callers also own records already
yielded to them. Concurrency bounds article count, not total bytes. SHA-256 and
record validation remain synchronous within each read; this API overlaps I/O
and does not introduce a CPU thread pool. Arrow conversion and batch sizes remain
caller-controlled.
