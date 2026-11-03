# PMC Arrow

Rust workspace for ingesting PubMed Central PDFs into Arrow datasets.

| Directory | Crate | Responsibility |
| --- | --- | --- |
| `crates/rs` | `pmc-arrow` | PDF snapshots and Arrow conversion |
| `crates/io` | `pmc-io` | Source ingestion and dataset output |
| `crates/cli` | `pmc-cli` | Command-line orchestration |

This change establishes buildable scaffolding. The library crates are placeholders;
the CLI exits with an explicit not-implemented message.

Dependency declarations match `atif-arrow`: Arrow `58` and OpenDAL `0.59.3`, with
OpenDAL default features disabled and `executors-tokio` / `http-transport-reqwest`
enabled. The lockfile aligns the Arrow family at `58.4.0` and OpenDAL, its core,
and its HTTP transport at `0.59.4`. Use `--locked` to retain these versions.

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```
