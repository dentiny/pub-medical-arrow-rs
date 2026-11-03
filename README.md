# PMC Arrow

Rust workspace for ingesting PubMed Central PDFs into Arrow datasets.

| Directory | Crate | Responsibility |
| --- | --- | --- |
| `crates/rs` | `pmc-arrow` | PDF snapshots and Arrow conversion |
| `crates/io` | `pmc-io` | Source ingestion and dataset output |
| `crates/cli` | `pmc-cli` | Command-line orchestration |

The core crate provides `PdfMetadata`, immutable validated `PdfRecord` values,
and the fixed 13-column `schema()`. `PdfRecord::try_new` preserves PDF bytes,
computes their SHA-256 and size, and retains raw metadata JSON verbatim.

PMCID normalization, source/version validation, JSON-object validation and a
lightweight PDF header check run during construction. Header checks do not
establish document integrity. Missing optional fields remain absent.

Arrow batch conversion follows in the next change. I/O and CLI remain
placeholders; the CLI exits with an explicit not-implemented message.

Dependency declarations match `atif-arrow`: Arrow `58` and OpenDAL `0.59.3`, with
OpenDAL default features disabled and `executors-tokio` / `http-transport-reqwest`
enabled. The lockfile aligns the Arrow family at `58.4.0` and OpenDAL, its core,
and its HTTP transport at `0.59.4`. Use `--locked` to retain these versions.

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```
