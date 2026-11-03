# Metadata fixture

`metadata.json` is synthetic and uses the PMC article-version JSON field names
and types documented in the [official bucket README](https://pmc-oa-opendata.s3.amazonaws.com/README.txt),
checked on 2026-11-03. It does not identify a real article or PDF. The large
unknown number, duplicate unknown key and whitespace exercise verbatim JSON
preservation. The `md5` value is a placeholder, not the PDF fixture's digest.

The PDF used by the tests is `crates/rs/tests/fixtures/tiny.pdf`.
