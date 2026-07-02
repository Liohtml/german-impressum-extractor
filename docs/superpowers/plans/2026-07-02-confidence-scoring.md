# TP2 Confidence Scoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an additive `extract_all_scored()` that returns every `extract_all` field annotated with a heuristic confidence in `0.0..=1.0`, driven by real format validators plus a document-label boost.

**Architecture:** `extract_all_scored` builds the TP1 `Document`, calls the unchanged `build_extracted` to get an `Extracted`, then a pure `score::score_extracted(Extracted, &Document)` pass maps each field to `Scored<T>`. No extraction logic is duplicated and `extract_all` is untouched.

**Tech Stack:** Rust (edition 2024), existing deps only (`regex`, `unicode-normalization`; `html5gum` under the `html` feature). No new dependency.

## Global Constraints

- MSRV `rust-version = "1.85"`; no APIs newer than 1.85.
- `#![forbid(unsafe_code)]`; write no `unsafe`.
- Additive / non-breaking: `extract_all`, `extract_all_html`, `Extracted`, `Person`, and all existing public signatures are unchanged; all existing tests pass unchanged.
- No new dependency; default build unaffected.
- The crate has `#![warn(missing_docs)]` AND CI runs `cargo clippy --all-targets --all-features -- -D warnings` — so **every public item (including every struct field) MUST have a `///` doc comment**, or the clippy gate fails.
- Confidence is always in `0.0..=1.0` (centralize clamping).
- `Scored<T>`/`ScoredExtracted` derive `Debug, Clone, PartialEq` (NOT `Eq` — `f32`); `ScoredExtracted` also derives `Default`. serde derives are `#[cfg_attr(feature = "serde", ...)]`, mirroring `Extracted`.
- The interim `#[allow(dead_code)]` on `mod segment;` and `mod candidate;` stays (Segment.span/value_span and Candidate fields remain unread in TP2). Do not remove them unless clippy stays green.

---

## File Structure

- Create `src/scored.rs` — public `Scored<T>` and `ScoredExtracted`.
- Create `src/score.rs` — `pub(crate)` validators + `score_*` fns + `score_extracted`.
- Modify `src/segment.rs` — add `Document::has_label`.
- Modify `src/lib.rs` — `mod scored; mod score;`, re-export types, add `extract_all_scored` (+ gated `extract_all_scored_html`).
- Create `tests/scoring.rs` — integration tests.
- Modify `README.md`, `CHANGELOG.md`.

---

## Task 1: Scored types (`scored.rs`)

**Files:**
- Create: `src/scored.rs`
- Modify: `src/lib.rs` (add `mod scored;` and `pub use scored::{Scored, ScoredExtracted};` near the other `mod` declarations / re-exports)
- Test: unit test in `src/scored.rs`

**Interfaces:**
- Produces: `pub struct Scored<T> { pub value: T, pub confidence: f32 }`; `pub struct ScoredExtracted { ... }` (fields listed below).

- [ ] **Step 1: Declare module + re-export in lib.rs**

In `src/lib.rs`, next to the existing `mod normalize;` etc., add `mod scored;`. After the `pub struct Person { ... }` definition (or near the other public re-exports), add:

```rust
pub use scored::{Scored, ScoredExtracted};
```

- [ ] **Step 2: Write the failing test**

Create `src/scored.rs`:

```rust
//! Scored extraction results (TP2). [`crate::extract_all_scored`] returns a
//! [`ScoredExtracted`]; each field carries a heuristic confidence in
//! `0.0..=1.0` (higher means more likely correct).

use crate::Person;

/// A value paired with a heuristic confidence in `0.0..=1.0`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Scored<T> {
    /// The extracted value — identical to what the unscored `extract_all` returns.
    pub value: T,
    /// Heuristic confidence in `0.0..=1.0`; higher means more likely correct.
    pub confidence: f32,
}

/// Scored counterpart of [`crate::Extracted`]: the same fields, each annotated
/// with a confidence. Returned by [`crate::extract_all_scored`].
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScoredExtracted {
    /// Scored email addresses.
    pub emails: Vec<Scored<String>>,
    /// Scored phone numbers.
    pub phones: Vec<Scored<String>>,
    /// Scored fax number, if any.
    pub fax: Option<Scored<String>>,
    /// Scored postcode.
    pub postcode: Option<Scored<String>>,
    /// Scored city.
    pub city: Option<Scored<String>>,
    /// Scored street.
    pub street: Option<Scored<String>>,
    /// Scored Handelsregister number.
    pub hr_number: Option<Scored<String>>,
    /// Scored Handelsregister court.
    pub hr_court: Option<Scored<String>>,
    /// Scored USt-IdNr. (VAT ID).
    pub vat_id: Option<Scored<String>>,
    /// Scored Steuernummer.
    pub tax_number: Option<Scored<String>>,
    /// Scored IBAN.
    pub iban: Option<Scored<String>>,
    /// Scored BIC.
    pub bic: Option<Scored<String>>,
    /// Scored legal form.
    pub legal_form: Option<Scored<String>>,
    /// Scored founding year.
    pub year_founded: Option<Scored<i32>>,
    /// Scored persons.
    pub persons: Vec<Scored<Person>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scored_holds_value_and_confidence() {
        let s = Scored { value: "info@a.de".to_string(), confidence: 0.85 };
        assert_eq!(s.value, "info@a.de");
        assert!((s.confidence - 0.85).abs() < f32::EPSILON);
        let d = ScoredExtracted::default();
        assert!(d.emails.is_empty() && d.iban.is_none());
    }
}
```

- [ ] **Step 3: Run test to verify it fails / then passes**

Run: `cargo test --lib scored 2>&1 | tail -20`
Expected: after adding the module it compiles and the test PASSES. (If `pub use` triggers an "unused import" it will not — these are public re-exports.)

- [ ] **Step 4: Verify build + clippy + existing tests**

Run: `cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean (all public items documented).
Run: `cargo test --all-targets 2>&1 | tail -4` → existing tests still pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/scored.rs
git commit -m "feat(scored): Scored<T> and ScoredExtracted result types"
```

---

## Task 2: `Document::has_label` (`segment.rs`)

**Files:**
- Modify: `src/segment.rs` (add a method to `impl Document`; add a unit test)
- Test: unit test in `src/segment.rs`

**Interfaces:**
- Produces: `pub(crate) fn Document::has_label(&self, kind: LabelKind) -> bool`

- [ ] **Step 1: Write the failing test**

In `src/segment.rs`, inside the existing `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn has_label_detects_present_labels() {
        let doc = Document::parse("Telefon: 030 123\nMuster GmbH".to_string());
        assert!(doc.has_label(LabelKind::Phone));
        assert!(!doc.has_label(LabelKind::Bank));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib segment::tests::has_label 2>&1 | head -15`
Expected: FAIL — `no method named has_label`.

- [ ] **Step 3: Implement**

In `src/segment.rs`, inside `impl Document` (next to `text`/`block_texts`), add:

```rust
    /// True if any segment carries the given label.
    pub(crate) fn has_label(&self, kind: LabelKind) -> bool {
        self.segments.iter().any(|s| s.label == Some(kind))
    }
```

(`LabelKind` already derives `Copy, PartialEq`, so `s.label == Some(kind)` compiles.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib segment 2>&1 | tail -10`
Expected: PASS. (`has_label` is unused by non-test code until Task 3; the existing `#[allow(dead_code)]` on `mod segment;` keeps the clippy gate green.)

- [ ] **Step 5: Verify clippy + full suite**

Run: `cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.
Run: `cargo test --all-targets 2>&1 | tail -4` → pass.

- [ ] **Step 6: Commit**

```bash
git add src/segment.rs
git commit -m "feat(segment): Document::has_label label-presence query"
```

---

## Task 3: Scoring engine + wiring (`score.rs`, `lib.rs`)

**Files:**
- Create: `src/score.rs`
- Modify: `src/lib.rs` (`mod score;`, `extract_all_scored`, `#[cfg(feature="html")] extract_all_scored_html`)
- Test: unit tests in `src/score.rs`

**Interfaces:**
- Consumes: `crate::{Extracted, Person}`, `crate::segment::{Document, LabelKind}`, `crate::scored::{Scored, ScoredExtracted}`, `crate::build_extracted`, `crate::normalize::normalize_text`, (html) `crate::html::html_to_impressum_text`.
- Produces: `pub(crate) fn score::score_extracted(base: Extracted, doc: &Document) -> ScoredExtracted`; `pub(crate) fn score::iban_mod97_valid(iban: &str) -> bool`; `pub fn extract_all_scored(text: &str) -> ScoredExtracted`; `#[cfg(feature="html")] pub fn extract_all_scored_html(html: &str) -> ScoredExtracted`.

- [ ] **Step 1: Write the failing tests**

Create `src/score.rs`:

```rust
//! Heuristic confidence scoring for extracted fields (TP2).
//!
//! Confidence = `clamp(validity_base + label_bonus, 0.0, 1.0)`, where
//! `validity_base` comes from a format check (e.g. IBAN mod-97, postcode
//! range) and `label_bonus` is a small boost when the document contains a
//! segment labeled with the field's matching kind. Scores are heuristic but
//! monotonic: a checksum-valid IBAN always outscores an invalid one, and a
//! labeled field never scores below its unlabeled counterpart.

use crate::scored::{Scored, ScoredExtracted};
use crate::segment::{Document, LabelKind};
use crate::{Extracted, Person};

const LABEL_BONUS: f32 = 0.1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iban_mod97_accepts_valid_and_rejects_flipped() {
        assert!(iban_mod97_valid("DE89370400440532013000"));
        assert!(iban_mod97_valid("DE89 3704 0044 0532 0130 00")); // spaces ignored
        assert!(!iban_mod97_valid("DE89370400440532013001")); // last digit changed
        assert!(!iban_mod97_valid("DE")); // too short
    }

    #[test]
    fn valid_iban_outscores_invalid() {
        assert!(base_iban("DE89370400440532013000") > base_iban("DE89370400440532013001"));
    }

    #[test]
    fn postcode_and_phone_bases_are_bounded() {
        assert!(base_postcode("10115") > base_postcode("999")); // valid > malformed
        assert!(base_phone("+493012345678") > base_phone("12"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib score 2>&1 | head -15`
Expected: FAIL — functions not defined.

- [ ] **Step 3: Implement validators + scoring**

Add to `src/score.rs` (above the `#[cfg(test)]` module):

```rust
/// Build a `Scored<T>`, clamping the confidence into `0.0..=1.0`.
fn scored<T>(value: T, confidence: f32) -> Scored<T> {
    Scored { value, confidence: confidence.clamp(0.0, 1.0) }
}

/// Validate an IBAN via the ISO 7064 mod-97 checksum (non-alphanumerics ignored).
pub(crate) fn iban_mod97_valid(iban: &str) -> bool {
    let s: String = iban
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();
    if s.len() < 4 {
        return false;
    }
    // Move the first four characters to the end, then compute mod 97 piecewise.
    let rearranged = format!("{}{}", &s[4..], &s[..4]);
    let mut remainder: u32 = 0;
    for ch in rearranged.chars() {
        let val = if ch.is_ascii_digit() {
            ch as u32 - '0' as u32
        } else {
            (ch as u32 - 'A' as u32) + 10 // A..Z -> 10..35
        };
        remainder = if val >= 10 {
            (remainder * 100 + val) % 97
        } else {
            (remainder * 10 + val) % 97
        };
    }
    remainder == 1
}

fn starts_upper(s: &str) -> bool {
    s.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
}

fn base_iban(v: &str) -> f32 {
    if iban_mod97_valid(v) { 0.95 } else { 0.55 }
}

fn base_postcode(v: &str) -> f32 {
    let ok = v.len() == 5
        && v.chars().all(|c| c.is_ascii_digit())
        && v.parse::<u32>().map(|n| (1000..=99999).contains(&n)).unwrap_or(false);
    if ok { 0.9 } else { 0.4 }
}

fn base_phone(v: &str) -> f32 {
    let digits = v.chars().filter(|c| c.is_ascii_digit()).count();
    if v.starts_with("+49") && (7..=15).contains(&digits) { 0.9 } else { 0.6 }
}

fn base_vat(v: &str) -> f32 {
    let ok = v.len() == 11 && v.starts_with("DE") && v[2..].chars().all(|c| c.is_ascii_digit());
    if ok { 0.9 } else { 0.6 }
}

fn base_year(y: i32) -> f32 {
    if (1700..=2100).contains(&y) { 0.8 } else { 0.3 }
}

fn base_person(p: &Person) -> f32 {
    match (p.first_name.as_deref(), p.last_name.as_deref()) {
        (Some(_), Some(_)) => 0.8,
        (None, Some(_)) => 0.5,
        _ => 0.3,
    }
}

/// Score every field of an already-extracted `Extracted`, using the document
/// for label-presence boosts. Pure post-processing; does not re-extract.
pub(crate) fn score_extracted(base: Extracted, doc: &Document) -> ScoredExtracted {
    let bonus = |kind: LabelKind| if doc.has_label(kind) { LABEL_BONUS } else { 0.0 };

    ScoredExtracted {
        emails: base
            .emails
            .into_iter()
            .map(|e| scored(e, 0.85 + bonus(LabelKind::Email)))
            .collect(),
        phones: base
            .phones
            .into_iter()
            .map(|p| {
                let c = base_phone(&p) + bonus(LabelKind::Phone);
                scored(p, c)
            })
            .collect(),
        // A fax is only produced when a Fax/Telefax label was matched, so it
        // carries the label bonus intrinsically.
        fax: base.fax.map(|f| {
            let c = base_phone(&f) + LABEL_BONUS;
            scored(f, c)
        }),
        postcode: base.postcode.map(|v| {
            let c = base_postcode(&v) + bonus(LabelKind::Postal);
            scored(v, c)
        }),
        city: base.city.map(|v| {
            let c = (if starts_upper(&v) { 0.7 } else { 0.4 }) + bonus(LabelKind::Postal);
            scored(v, c)
        }),
        street: base.street.map(|v| scored(v, 0.85 + bonus(LabelKind::Postal))),
        hr_number: base.hr_number.map(|v| scored(v, 0.85 + bonus(LabelKind::Register))),
        hr_court: base.hr_court.map(|v| scored(v, 0.7 + bonus(LabelKind::Court))),
        vat_id: base.vat_id.map(|v| {
            let c = base_vat(&v) + bonus(LabelKind::VatId);
            scored(v, c)
        }),
        tax_number: base.tax_number.map(|v| scored(v, 0.75 + bonus(LabelKind::TaxNumber))),
        iban: base.iban.map(|v| {
            let c = base_iban(&v) + bonus(LabelKind::Bank);
            scored(v, c)
        }),
        bic: base.bic.map(|v| scored(v, 0.9 + bonus(LabelKind::Bank))),
        legal_form: base.legal_form.map(|v| scored(v, 0.9 + bonus(LabelKind::LegalName))),
        year_founded: base.year_founded.map(|y| {
            let c = base_year(y) + bonus(LabelKind::Founded);
            scored(y, c)
        }),
        persons: base
            .persons
            .into_iter()
            .map(|p| {
                let c = base_person(&p) + bonus(LabelKind::Managers);
                scored(p, c)
            })
            .collect(),
    }
}
```

- [ ] **Step 4: Run unit tests to verify they pass**

Run: `cargo test --lib score 2>&1 | tail -15`
Expected: PASS (3 tests). If `iban_mod97_accepts_valid...` fails, the mod-97 loop has a bug — do NOT change the test's expected values (`DE89370400440532013000` is the canonical valid German example IBAN); fix the algorithm.

- [ ] **Step 5: Wire the public entry points in lib.rs**

In `src/lib.rs`, next to `mod scored;`, add `mod score;`. Then add these public functions near `extract_all` (after `build_extracted`):

```rust
/// Extract every supported field with a heuristic confidence score per field.
///
/// Additive companion to [`extract_all`]: same extraction, each field wrapped
/// in a [`Scored`] with a confidence in `0.0..=1.0`.
pub fn extract_all_scored(text: &str) -> ScoredExtracted {
    let doc = segment::Document::parse(normalize::normalize_text(text));
    score::score_extracted(build_extracted(&doc), &doc)
}

/// Like [`extract_all_scored`], but from an HTML page. Available with the `html` feature.
#[cfg(feature = "html")]
pub fn extract_all_scored_html(html: &str) -> ScoredExtracted {
    let doc = segment::Document::parse(html::html_to_impressum_text(html));
    score::score_extracted(build_extracted(&doc), &doc)
}
```

- [ ] **Step 6: Full gate**

Run: `cargo fmt --all` then `cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean. (Note: `score_extracted` now constructs `LabelKind::LegalName`, so that previously-unconstructed variant becomes live; the `mod segment;` allow stays for the still-unread `Segment.span`/`value_span`.)
Run: `cargo test --all-targets 2>&1 | tail -4` and `cargo test --all-targets --features html 2>&1 | tail -4` → all pass.

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs src/score.rs
git commit -m "feat(score): confidence scoring engine + extract_all_scored"
```

---

## Task 4: Integration tests (`tests/scoring.rs`)

**Files:**
- Create: `tests/scoring.rs`

**Interfaces:**
- Consumes: `german_impressum_extractor::{extract_all, extract_all_scored}` (+ `extract_all_scored_html` under `html`).

- [ ] **Step 1: Write the tests**

Create `tests/scoring.rs`:

```rust
use german_impressum_extractor::{extract_all, extract_all_scored};

const FULL: &str = "\
Musterreinigung GmbH & Co. KG
Geschäftsführer: Dr. Hans Müller
Hauptstraße 12, 10115 Berlin
Tel: +49 30 1234567
E-Mail: info@musterreinigung.de
Eingetragen im Handelsregister Berlin HRB 12345 B
USt-IdNr.: DE 123 456 789
IBAN: DE89 3704 0044 0532 0130 00
BIC: COBADEFFXXX
Gegründet 1985";

#[test]
fn scored_values_match_unscored_extraction() {
    let d = extract_all(FULL);
    let s = extract_all_scored(FULL);
    assert_eq!(s.iban.as_ref().map(|x| x.value.clone()), d.iban);
    assert_eq!(s.vat_id.as_ref().map(|x| x.value.clone()), d.vat_id);
    assert_eq!(s.postcode.as_ref().map(|x| x.value.clone()), d.postcode);
    assert_eq!(s.legal_form.as_ref().map(|x| x.value.clone()), d.legal_form);
    assert_eq!(s.year_founded.as_ref().map(|x| x.value), d.year_founded);
    assert_eq!(
        s.emails.iter().map(|x| x.value.clone()).collect::<Vec<_>>(),
        d.emails
    );
}

#[test]
fn confidences_are_in_range_and_iban_is_checksum_confident() {
    let s = extract_all_scored(FULL);
    for c in s
        .emails
        .iter()
        .map(|x| x.confidence)
        .chain(s.iban.iter().map(|x| x.confidence))
        .chain(s.vat_id.iter().map(|x| x.confidence))
    {
        assert!((0.0..=1.0).contains(&c), "confidence out of range: {c}");
    }
    // Valid IBAN (mod-97) + a Bank label ("IBAN:") present → high confidence.
    assert!(s.iban.unwrap().confidence >= 0.95);
}

#[test]
fn invalid_iban_scores_below_valid_iban() {
    let good = extract_all_scored("IBAN: DE89 3704 0044 0532 0130 00").iban.unwrap();
    let bad = extract_all_scored("IBAN: DE89 3704 0044 0532 0130 01").iban.unwrap();
    assert!(good.confidence > bad.confidence, "good={} bad={}", good.confidence, bad.confidence);
}

#[test]
fn label_presence_boosts_phone_confidence() {
    let labeled = extract_all_scored("Telefon: +49 30 1234567");
    let unlabeled = extract_all_scored("+49 30 1234567");
    let lc = labeled.phones.first().unwrap().confidence;
    let uc = unlabeled.phones.first().unwrap().confidence;
    assert!(lc >= uc, "labeled {lc} should be >= unlabeled {uc}");
}

#[cfg(feature = "serde")]
#[test]
fn scored_extracted_serde_roundtrips() {
    let s = extract_all_scored(FULL);
    let json = serde_json::to_string(&s).unwrap();
    let back: german_impressum_extractor::ScoredExtracted = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

#[cfg(feature = "html")]
#[test]
fn html_scored_matches_text_scored_fields() {
    use german_impressum_extractor::extract_all_scored_html;
    let s = extract_all_scored_html(
        "<p>Muster GmbH</p><dl><dt>USt-IdNr.</dt><dd>DE123456789</dd></dl>",
    );
    assert_eq!(s.legal_form.unwrap().value, "GmbH");
    assert_eq!(s.vat_id.unwrap().value, "DE123456789");
}
```

- [ ] **Step 2: Run (both feature states)**

Run: `cargo test --test scoring 2>&1 | tail -12` → pass (serde/html tests skipped without features).
Run: `cargo test --test scoring --features serde 2>&1 | tail -6` → serde test runs + passes.
Run: `cargo test --test scoring --features html 2>&1 | tail -6` → html test runs + passes.

- [ ] **Step 3: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 4: Commit**

```bash
git add tests/scoring.rs
git commit -m "test(score): integration tests for extract_all_scored"
```

---

## Task 5: Docs + CHANGELOG

**Files:**
- Modify: `README.md`, `CHANGELOG.md`

- [ ] **Step 1: README — document the scored API**

Read `README.md` first. After the "Granular extractors" section, add:

```markdown
### Confidence scores

Need to know how much to trust each field? `extract_all_scored` returns the
same data with a per-field confidence in `0.0..=1.0` (heuristic: format
validity such as an IBAN mod-97 check, plus a boost when the source page
labels the field):

```rust
use german_impressum_extractor::extract_all_scored;

let d = extract_all_scored(impressum_text);
if let Some(iban) = d.iban {
    println!("{} (confidence {:.2})", iban.value, iban.confidence);
}
```

`extract_all` is unchanged; scoring is purely additive. With the `html`
feature, `extract_all_scored_html` does the same from raw HTML.
```

- [ ] **Step 2: CHANGELOG**

Under `## [Unreleased]` → `### Added`, add:

```markdown
- `extract_all_scored` / `extract_all_scored_html` + `Scored<T>` / `ScoredExtracted`:
  per-field heuristic confidence (`0.0..=1.0`) driven by format validators
  (IBAN mod-97, postcode range, phone/VAT/BIC structure) plus a document-label
  boost. Additive; `extract_all` is unchanged.
```

- [ ] **Step 3: Verify build + full gate**

Run: `cargo build --all-features 2>&1 | tail -2` → builds.
Run: `cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 4: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: document extract_all_scored confidence API"
```

---

## Self-Review

**1. Spec coverage:** `Scored`/`ScoredExtracted` → Task 1. `Document::has_label` → Task 2. validators + scoring + `score_extracted` + `extract_all_scored` (+html) → Task 3. Non-breaking parity, confidence range, monotonicity, serde, html → Task 4. Docs/CHANGELOG → Task 5. `extract_all` untouched throughout. ✓

**2. Placeholder scan:** No TBD/TODO; every code step is complete. ✓

**3. Type consistency:** `Scored<T>{value,confidence}`, `ScoredExtracted` field names mirror `Extracted`, `score_extracted(Extracted,&Document)->ScoredExtracted`, `iban_mod97_valid(&str)->bool`, `Document::has_label(LabelKind)->bool`, `extract_all_scored(&str)->ScoredExtracted` — consistent across tasks. ✓

**Executor notes:**
- Every public field needs a `///` (crate is `#![warn(missing_docs)]` + `-D warnings`). Task 1 code already includes them — keep them.
- Do not change the canonical valid IBAN `DE89370400440532013000` in tests; if a checksum test fails, the bug is in `iban_mod97_valid`.
- Keep the `#[allow(dead_code)]` on `mod segment;`/`mod candidate;`; only `LabelKind::LegalName` transitions to constructed via Task 3.
