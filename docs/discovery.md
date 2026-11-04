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

Pass each path to `PmcReader::read` to fetch its metadata and referenced PDF.
The same path stream can feed a concurrent reader without requiring all paths
in memory. Listing does not select article versions or filter licenses.

This API lists live storage objects. Parsing PMC inventory reports is separate
work; it is not needed when the caller already has a path stream.
