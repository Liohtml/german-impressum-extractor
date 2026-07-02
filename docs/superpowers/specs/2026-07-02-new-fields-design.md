# TP5: New fields (authority, chamber, De-Mail, ODR link, register type)

**Status:** Approved design (2026-07-02) — autonomous continuation (Spec→PR without per-gate acceptance).
**Sub-project:** TP5 of 5 (final). Builds on TP1–TP4 (all merged).

## Scope
Add five new Impressum fields, each as a public standalone extractor **and** a field on `Extracted`:
1. **`supervisory_authority`** — "Aufsichtsbehörde" (regulated professions must name theirs). Label-based.
2. **`professional_chamber`** — "zuständige Kammer" / "Berufskammer". Label-based.
3. **`de_mail`** — a De-Mail address, when labeled "De-Mail:". Label-based, low-noise.
4. **`dispute_resolution_url`** — the EU Online-Dispute-Resolution (OS-Plattform) link (`https://ec.europa.eu/consumers/odr`), legally required for most B2C sites. Canonical-URL regex, high precision.
5. **`register_type`** — the Handelsregister section, `"HRA"` or `"HRB"`, derived from the HR number. Deterministic.

**Out of scope:** scoring the new fields in `ScoredExtracted` (TP2) — deferred; `ScoredExtracted` will not gain these fields in TP5 (noted as a known follow-up). No other behavior changes.

## Breaking-change note (accepted)
Adding fields to the public `Extracted` struct is a struct-**construction** breaking change (external `Extracted { .. }` literals need `..Default::default()`). This is acceptable because: the crate is `0.1.0` and unpublished; `Extracted` is normally obtained from `extract_all` and only *read* (reading new fields is non-breaking); and we add `#[non_exhaustive]` to `Extracted` so this is the **last** construction-level change (future field additions become non-breaking). `Extracted` still derives `Debug, Clone, Default, PartialEq, Eq`.

## Design
Each new `Option<String>` extractor follows the established **wrapper + core** pattern (TP3) so `extract_all` normalizes exactly once and no double-decoding occurs:

```rust
pub fn extract_supervisory_authority(text: &str) -> Option<String> {
    extract_supervisory_authority_core(&normalize::normalize_text(text))
}
fn extract_supervisory_authority_core(text: &str) -> Option<String> { /* regex, no normalize */ }
```

`build_extracted` calls the `*_core` variants on the already-normalized `doc.text()`.

Extractor logic:
- **supervisory_authority / professional_chamber / de_mail**: label regex capturing the value up to end of line (`[^\n]{…}`), trimmed. `de_mail` captures an email-shaped token after a "De-Mail" label (lowercased).
- **dispute_resolution_url**: match the canonical ODR URL `https?://(www\.)?ec\.europa\.eu/consumers/odr/?` (case-insensitive), return the matched URL.
- **register_type**: reuse `extract_hr_number_core`; map a leading `HRA`/`HRB` to `"HRA"`/`"HRB"`, else `None`.

## Constraints
- Infallible/panic-free; `#![forbid(unsafe_code)]`; MSRV 1.85; no new dependency.
- Every new public item (5 fns + 5 `Extracted` fields) documented (`#![warn(missing_docs)]` + clippy `-D warnings`).
- `build_extracted` must call the `*_core` variants (single-normalize invariant from TP3).

## Testing
- Non-breaking for *reads*: all existing tests pass unchanged; `extract_all` still returns the previous fields identically, plus the 5 new ones.
- Each extractor: a positive case (label/URL present → expected value) and a negative case (absent → `None`).
- `register_type`: "…HRB 12345…" → `Some("HRB")`; "…HRA 55…" → `Some("HRA")`; no HR → `None`.
- `dispute_resolution_url`: a page containing the ec.europa.eu/odr link → that URL; a page without → `None`.
- `extract_all` integration: a full Impressum with all five present populates all five fields, and existing fields are unchanged.
- `#[non_exhaustive]` present on `Extracted` (compile-level; a doc/test note).

## Success criteria
1. All existing tests pass unchanged; `extract_all` existing fields unchanged.
2. The 5 new standalone extractors and the 5 `Extracted` fields work per the tests.
3. `#[non_exhaustive]` added to `Extracted`.
4. No new dep; builds + tests on Rust 1.85; clippy `-D warnings` clean.
