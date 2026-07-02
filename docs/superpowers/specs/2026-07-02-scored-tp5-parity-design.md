# Follow-up: ScoredExtracted ↔ TP5 parity

**Status:** Approved design (2026-07-02) — autonomous follow-up (Spec→PR).
**Context:** TP5 added five fields to `Extracted` (`register_type`, `supervisory_authority`, `professional_chamber`, `de_mail`, `dispute_resolution_url`) but deliberately did NOT extend `ScoredExtracted` (TP2). This follow-up closes that gap so `extract_all_scored` covers every `extract_all` field.

## Scope
- Add the five fields to `ScoredExtracted` as `Option<Scored<String>>`.
- Populate them in `score::score_extracted` with heuristic confidences.
- Add `#[non_exhaustive]` to `ScoredExtracted` (matching `Extracted`), so future field additions are non-breaking.
- Update docs/CHANGELOG (drop the "ScoredExtracted does not yet cover the new fields" caveat).

## Confidence model
Consistent with TP2 (`clamp(base + label_bonus, 0, 1)`). These fields are label-/format-gated at extraction, so they carry intrinsic confidence:
- `register_type` (deterministic HRA/HRB from the HR number): `0.85 + bonus(LabelKind::Register)`.
- `supervisory_authority` (labeled "Aufsichtsbehörde"): flat `0.8` (no matching `LabelKind`; do not add a new one).
- `professional_chamber` (labeled; looser regex): flat `0.75`.
- `de_mail` (labeled + email-shaped): flat `0.9`.
- `dispute_resolution_url` (canonical ODR URL, very high precision): flat `0.97`.

All via the existing clamping `scored()` helper.

## Constraints
- Non-breaking for reads: existing `ScoredExtracted` fields and `extract_all_scored` values unchanged; all existing tests pass. Adding fields is a construction-level change, mitigated by `#[non_exhaustive]` (and `ScoredExtracted` is obtained from `extract_all_scored`, not externally constructed).
- MSRV 1.85; `#![forbid(unsafe_code)]`; no new dep; clippy `-D warnings` clean; every new public field documented.

## Testing
- `extract_all_scored` on a text containing the five fields populates all five `Scored` values, each with `.value` equal to the corresponding `extract_all` field and `confidence` in `0.0..=1.0`.
- serde round-trip of `ScoredExtracted` still passes (now with the new fields).
- Existing scoring tests unchanged.

## Success criteria
1. `ScoredExtracted` has the five new `Option<Scored<String>>` fields + `#[non_exhaustive]`.
2. `score_extracted` populates them; `.value`s equal the `extract_all` fields; confidences in range.
3. Existing tests unchanged; no new dep; Rust 1.85; clippy clean.
