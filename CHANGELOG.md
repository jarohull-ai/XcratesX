# Changelog

## Unreleased

- Named, persistent experiments shared across plugins: create, open, edit, build and inspect.
- Generated Cargo projects and lockfiles that can be shared and rebuilt with standard Cargo.
- Non-interactive experiment commands and configuration validation.
- English terminal UI and plugin messages; configurable network diagnostic targets.
- Preset-preserving reload with validation; removed obsolete fallback plugin entries.
- Crate search cancels superseded requests, bounds responses, checks HTTP status, and avoids
  holding the UI state lock during network reads.
- Offline end-to-end workflow checks in CI; persistence and configuration regression tests.

Earlier improvements introduced real Cargo builds/graphs, protocol limits, plugin stderr
logs, response history, and process-group cleanup.
