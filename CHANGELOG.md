# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `extract_fax` + `Extracted::fax` — labeled Fax/Telefax numbers, removed from
  `phones` in `extract_all` (#10).
- `extract_iban` + `Extracted::iban` and `extract_bic` + `Extracted::bic` —
  German bank details (#9).
- "Verantwortlich" (§18 Abs. 2 MStV / §55 RStV) person role detection (#11).
- Integration test suite covering all addressed issues (`tests/regressions.rs`) (#6).
- GitHub Actions CI: fmt, clippy, tests (default + `serde`), doc tests (#7).

### Fixed
- `Person.role` was always `None`; the role is now detected from the matched
  keyword (#1).
- `strip_titles` corrupted names containing title substrings (e.g. "Herrmann"
  → "mann"); now uses whole-token matching (#2).
- `extract_emails` no longer emits code-fragment false positives such as
  `…@….css` or known junk domains (#3).
- `extract_persons` no longer returns German articles, prepositions, or role
  keywords as person names (#4).
- `extract_vat_id` now matches USt-IdNr. with internal grouping spaces
  (`DE 123 456 789`) and avoids mis-reading an IBAN prefix as a VAT ID (#5).

### Changed
- README: replaced the unpublished crates.io install snippet with a git
  dependency and swapped the broken crates.io/docs.rs badges for a CI badge (#8).

## [0.1.0] - 2026-05-02

### Added
- Initial release.
- `extract_all` — one-shot extraction of every supported field.
- Per-field extractors: `extract_emails`, `extract_phones`, `extract_address`,
  `extract_persons`, `extract_legal_form`, `extract_hr_number`, `extract_vat_id`,
  `extract_year_founded`.
- Optional `serde` feature.
