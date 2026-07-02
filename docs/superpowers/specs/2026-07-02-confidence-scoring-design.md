# TP2: Confidence scoring + `extract_all_scored()`

**Status:** Approved design (2026-07-02) — autonomous continuation of the noise-reduction program (author approved running Spec→PR without per-gate acceptance).
**Sub-project:** TP2 of 5. Builds on TP1 (normalization + segmentation + `Candidate` substrate, merged in #39).

## Decisions carried from brainstorming
- Precision model: **best-guess + confidence score** per field.
- API integration: **additive / non-breaking**. `extract_all` is unchanged; a new `extract_all_scored()` returns scored results.
- HTML: mirror with `extract_all_scored_html` behind the existing `html` feature.
- Dependencies: none new.

## Scope of TP2
**In scope**
- New public types `Scored<T> { value: T, confidence: f32 }` and `ScoredExtracted` mirroring `Extracted`, with each field carrying a confidence in `0.0..=1.0`.
- A `score` module: real format **validators** (IBAN mod-97, German postcode range, phone `+49`/length, BIC/VAT structure, person completeness, legal-form membership) plus a light **document-label boost** using TP1's segment labels.
- `extract_all_scored(&str) -> ScoredExtracted` and `#[cfg(feature = "html")] extract_all_scored_html(&str) -> ScoredExtracted`.
- `Document::has_label(&self, LabelKind) -> bool` to expose the label signal (also makes TP1's `LabelKind` fully consumed).

**Out of scope (later)**
- Multiple ranked candidates per field / candidate lists (TP4 multi-entity).
- Per-value provenance spans / exact segment-to-value mapping (needs span-aware extractors; TP3/TP4). TP2 uses **document-level** label presence, not per-value mapping.
- Changing any existing extractor or `extract_all`.

## Architecture (DRY)
`extract_all_scored` reuses the existing extraction wholesale, then scores:

```
text ──► normalize ──► Document::parse ──► build_extracted (TP1, unchanged) ──► Extracted
                                   │                                              │
                                   └──────────────► score::score_extracted(Extracted, &Document) ──► ScoredExtracted
```

```rust
pub fn extract_all_scored(text: &str) -> ScoredExtracted {
    let doc = segment::Document::parse(normalize::normalize_text(text));
    score::score_extracted(build_extracted(&doc), &doc)
}
```

No extraction logic is duplicated: scoring is a pure post-processing pass over the already-extracted `Extracted` plus the `Document` (for label context).

## Confidence model
Per field: `confidence = clamp(validity_base + label_bonus, 0.0, 1.0)`.

- **validity_base** — from a format validator. Examples: IBAN mod-97 pass `0.95` / fail `0.55`; postcode in `01000..=99999` `0.9` / else `0.4`; phone `+49` & 7–15 digits `0.9` / else `0.6`; email (already TLD-filtered) `0.85`; VAT `DE`+9 digits `0.9`; BIC (already banking-context-gated) `0.9`; legal form (matched known set) `0.9`; year in `1700..=2100` `0.8`; person first+last `0.8` / last-only `0.5`; hr_number `0.85`; hr_court/tax/city/street conservative `0.7–0.85`.
- **label_bonus** — `+0.1` if the document contains a segment labeled with the field's matching `LabelKind` (e.g. an IBAN in a doc that has a `Bank`-labeled segment), else `0.0`. Clamped so total ≤ 1.0.

Scores are documented as **heuristic**, monotonic in the obvious way (a checksum-valid IBAN always scores higher than an invalid one; a labeled field scores ≥ its unlabeled counterpart), and stable/deterministic.

## Module layout
- `src/scored.rs` (new): `Scored<T>` and `ScoredExtracted` (+ serde-gated derives, mirroring `Extracted`). Public.
- `src/score.rs` (new): validators (`iban_mod97_valid`, …) + per-field `score_*` fns + `score_extracted`. `pub(crate)`.
- `src/segment.rs` (modify): add `Document::has_label`.
- `src/lib.rs` (modify): `mod scored; mod score;`, re-export `Scored`/`ScoredExtracted`, add `extract_all_scored` (+ `#[cfg(feature="html")] extract_all_scored_html`). `extract_all` untouched.

## Error handling
- Infallible/panic-free; no `unwrap` on parsed input. `f32` math only; `clamp(0.0, 1.0)`.
- `#![forbid(unsafe_code)]` stays; MSRV 1.85; no new deps.
- `ScoredExtracted`/`Scored<T>` derive `PartialEq` (not `Eq` — `f32`), `Debug`, `Clone`, `Default`.

## Testing
- Non-breaking: all existing tests pass unchanged; `extract_all` output identical.
- Validators: `iban_mod97_valid` accepts a known-good DE IBAN and rejects a digit-flipped one; postcode/phone/vat/bic/year boundary cases.
- Scoring monotonicity: valid IBAN `confidence` > invalid; a phone in a `Telefon:`-labeled doc scores higher than the same phone with no label; person with first+last > last-only.
- `extract_all_scored` end-to-end on the full-Impressum fixture: every populated field has `0.0 < confidence <= 1.0`, and `.value`s equal the corresponding `extract_all` values (parity with the unscored path).
- `#[cfg(feature="html")]` parity for `extract_all_scored_html`.
- serde round-trip of `ScoredExtracted` (under `--features serde`).
- CI: covered by existing `--all-features` / `html` / serde / MSRV jobs; add a `Test (html feature)` already exists — no new job required.

## Success criteria
1. `extract_all` and all existing tests unchanged (non-breaking).
2. `extract_all_scored(t).<field>.value` equals `extract_all(t).<field>` for every field (same extraction, only annotated).
3. Confidence is always in `0.0..=1.0`; validity + label monotonicity holds (tested).
4. No new dependency; builds + tests pass on Rust 1.85; clippy `-D warnings` clean.

## Open items for planning
- Exact base-confidence constants (encoded in the plan; tuned only for monotonicity, not calibrated against a dataset — that needs real fixtures, a later concern).
