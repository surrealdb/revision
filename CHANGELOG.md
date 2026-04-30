# Changelog

## 0.18.0

### Added

- Optional feature **`skip`**: traits `SkipRevisioned` / `SkipCheckRevisioned`, free helpers and `revision-derive` support for skipping encoded values (`skip = false` on `#[revisioned(...)]` to opt out).
- `SkipRevisioned::skip_revisioned_slice`, used by `skip_slice` so in-memory payloads can skip bulk byte runs via `SliceReader::consume`; derived types mirror per-field skips on the slice path; `skip_reader` / `skip_check_reader` aliases; regression tests including large UTF-8 strings and nested composites; extended skip benchmarks.
