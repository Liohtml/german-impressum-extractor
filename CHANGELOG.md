# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-05-02

### Added
- Initial release.
- `extract_all` — one-shot extraction of every supported field.
- Per-field extractors: `extract_emails`, `extract_phones`, `extract_address`,
  `extract_persons`, `extract_legal_form`, `extract_hr_number`, `extract_vat_id`,
  `extract_year_founded`.
- Optional `serde` feature.
