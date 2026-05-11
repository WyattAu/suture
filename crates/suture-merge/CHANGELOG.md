# Changelog

## [0.3.0]

### Added

- `merge_json()` — three-way semantic merge for JSON files
- `merge_yaml()` — three-way semantic merge for YAML files
- `merge_toml()` — three-way semantic merge for TOML files
- `merge_csv()` — three-way semantic merge for CSV files
- `merge_xml()` — three-way semantic merge for XML files
- `merge_markdown()` — three-way semantic merge for Markdown files
- Feature flags: `json`, `yaml`, `toml`, `csv`, `xml`, `markdown`
- `all` feature includes all format drivers
- `merge_auto()`, `diff()`, `format_diff()` — extension-based auto-detection
- `MergeResult` struct with `merged` content and `MergeStatus` (Clean/Conflict)
- `MergeError` enum: `UnsupportedFormat`, `ParseError`, `NoDriver`
- Comprehensive test suites: unit tests, full validation, hardening/stress tests
- Adversarial input, unicode, size stress, cross-driver consistency tests
- `#[non_exhaustive]` on all public types for API stability
