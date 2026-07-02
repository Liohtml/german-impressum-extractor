# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `extract_addresses` + `Address`: return every postal address on the page
  (one per block, document order, deduped) for multi-location Impressum pages.
  `extract_address` (first address) and `extract_all` are unchanged.
- `extract_all_scored` / `extract_all_scored_html` + `Scored<T>` / `ScoredExtracted`:
  per-field heuristic confidence (`0.0..=1.0`) driven by format validators
  (IBAN mod-97, postcode range, phone/VAT/BIC structure) plus a document-label
  boost. Additive; `extract_all` is unchanged.
- Input normalization (Unicode NFC, invisible-char cleanup, HTML entity decoding) currently applies to `extract_all`, `extract_all_html`, and `extract_address`; the standalone per-field extractors (`extract_fax`, `extract_phones`, `extract_emails`, etc.) operate on raw input and will be normalization-hardened in a later change.
- New fields + extractors: `supervisory_authority` (Aufsichtsbehörde),
  `professional_chamber` (zuständige Kammer / Berufskammer), `de_mail`,
  `dispute_resolution_url` (EU OS-Plattform / ODR link), and `register_type`
  (HRA/HRB). Added to `Extracted` and as standalone `extract_*` functions.
- `extract_tax_number` — public Steuernummer extractor, matching the rest of the
  granular API (#31).
- `extract_hr_court` — public Handelsregister-court extractor (#26).
- MSRV verification job in CI (builds + tests on Rust 1.85) (#29).
- Regression tests for the char-boundary panics, TLD filtering, KGaA, HR-court,
  and Steuernummer abbreviations, plus broader coverage of the core extractors
  (#24).
- `extract_fax` + `Extracted::fax` — labeled Fax/Telefax numbers, removed from
  `phones` in `extract_all` (#10).
- `extract_iban` + `Extracted::iban` and `extract_bic` + `Extracted::bic` —
  German bank details (#9).
- "Verantwortlich" (§18 Abs. 2 MStV / §55 RStV) person role detection (#11).
- Integration test suite covering all addressed issues (`tests/regressions.rs`) (#6).
- GitHub Actions CI: fmt, clippy, tests (default + `serde`), doc tests (#7).
- Text normalization layer (Unicode NFC, invisible-char / whitespace cleanup,
  well-formed HTML entity decoding) applied to all input before extraction.
- Block-aware address extraction: postcode/city and street are taken from the
  same text block, preventing cross-entity mixing on multi-address pages.
- Optional `html` feature: `extract_all_html` and `html_to_impressum_text`
  parse raw HTML (via `html5gum`) into structured data. Default build unchanged.

### Fixed
- Standalone `extract_*` functions now normalize their input (Unicode, whitespace,
  HTML entities) like `extract_all`, so direct calls no longer diverge from the
  corresponding `extract_all` field on messy input.
- `extract_persons` rejects tokens containing digits and an expanded set of
  non-name noise nouns (team, kontakt, webdesign, …), reducing false persons.
- `extract_bic` and `truncate_at_sentence_end` no longer panic on multi-byte
  UTF-8 input (zero-width spaces, soft hyphens); byte offsets are snapped to a
  char boundary via an MSRV-safe `floor_char_boundary` helper (#13, #14, #25).
- `extract_emails` now validates the TLD against an allowlist of real top-level
  domains, dropping prose false positives like `th@matters.discover` (#15).
- `hr_court` no longer swallows the HR-number prefix on separator-free lines
  (`Amtsgericht Berlin HRB 12345` → `"Berlin"`, not `"Berlin HRB"`) (#26).
- KGaA legal forms are recognized: `GmbH & Co. KGaA` and `KGaA` are no longer
  misclassified as `GmbH`/none (#30).
- `TAX_NUMBER_RE` now matches the abbreviations `St.-Nr.`, `StNr.`,
  `Steuer-Nr.` and `St.Nr.` in addition to the full word (#32).
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
- Dropped the `once_cell` dependency in favor of `std::sync::LazyLock`
  (available at the 1.85 MSRV) (#27).
- Pinned all GitHub Actions in CI to commit SHAs (#16).
- `Cargo.toml` `documentation` now points at the repository README instead of a
  not-yet-live docs.rs URL (#28).
- README: replaced the unpublished crates.io install snippet with a git
  dependency and swapped the broken crates.io/docs.rs badges for a CI badge (#8).
- `Extracted` is now `#[non_exhaustive]` (construct it via `..Default::default()`
  or obtain it from `extract_all`). `ScoredExtracted` does not yet cover the five
  new fields.

## [0.1.0] - 2026-05-02

### Added
- Initial release.
- `extract_all` — one-shot extraction of every supported field.
- Per-field extractors: `extract_emails`, `extract_phones`, `extract_address`,
  `extract_persons`, `extract_legal_form`, `extract_hr_number`, `extract_vat_id`,
  `extract_year_founded`.
- Optional `serde` feature.
