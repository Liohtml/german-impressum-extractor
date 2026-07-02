# ScoredExtracted ↔ TP5 Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Score the five TP5 fields in `extract_all_scored` by extending `ScoredExtracted` + `score_extracted`.

**Architecture:** Add five `Option<Scored<String>>` fields to `ScoredExtracted` (+ `#[non_exhaustive]`); populate them in `score_extracted` via the existing `scored()` clamp helper.

**Tech Stack:** Rust (edition 2024), existing deps only.

## Global Constraints
- MSRV 1.85; `#![forbid(unsafe_code)]`; no unsafe; no new dependency.
- Non-breaking for reads: existing `ScoredExtracted` fields/values unchanged; all existing tests pass.
- Crate is `#![warn(missing_docs)]` + clippy `-D warnings`: every new public field needs a `///` doc.
- Use the existing `scored()` helper (clamps to 0..=1). Confidence must stay in `0.0..=1.0`.

---

## File Structure
- Modify `src/scored.rs` — 5 new fields on `ScoredExtracted` + `#[non_exhaustive]`.
- Modify `src/score.rs` — populate the 5 fields in `score_extracted`.
- Modify `tests/scoring.rs` — parity test for the new scored fields.
- Modify `README.md`/`CHANGELOG.md` — drop the "not yet covered" caveat.

---

## Task 1: Extend ScoredExtracted + score_extracted

**Files:** Modify `src/scored.rs`, `src/score.rs`; append to `tests/scoring.rs`.

- [ ] **Step 1: Write the failing test (append to tests/scoring.rs)**

```rust
#[test]
fn scored_covers_tp5_fields() {
    let text = "\
Muster GmbH
Handelsregister HRB 12345
Aufsichtsbehörde: Landesamt für Gesundheit
Zuständige Kammer: IHK Berlin
De-Mail: kontakt@firma.de-mail.de
Online-Streitbeilegung: https://ec.europa.eu/consumers/odr/";
    let d = extract_all(text);
    let s = extract_all_scored(text);

    // Values match the unscored extraction.
    assert_eq!(s.register_type.as_ref().map(|x| x.value.clone()), d.register_type);
    assert_eq!(s.supervisory_authority.as_ref().map(|x| x.value.clone()), d.supervisory_authority);
    assert_eq!(s.professional_chamber.as_ref().map(|x| x.value.clone()), d.professional_chamber);
    assert_eq!(s.de_mail.as_ref().map(|x| x.value.clone()), d.de_mail);
    assert_eq!(s.dispute_resolution_url.as_ref().map(|x| x.value.clone()), d.dispute_resolution_url);

    // All present and in range; the canonical ODR URL scores highest.
    for c in [
        s.register_type.as_ref().unwrap().confidence,
        s.supervisory_authority.as_ref().unwrap().confidence,
        s.professional_chamber.as_ref().unwrap().confidence,
        s.de_mail.as_ref().unwrap().confidence,
        s.dispute_resolution_url.as_ref().unwrap().confidence,
    ] {
        assert!((0.0..=1.0).contains(&c), "confidence out of range: {c}");
    }
    assert!(s.dispute_resolution_url.unwrap().confidence >= 0.95);
}
```

(`tests/scoring.rs` already imports `extract_all` and `extract_all_scored`; if not, add them to the `use` line.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --test scoring scored_covers_tp5 2>&1 | head -15`
Expected: FAIL — `no field register_type on ScoredExtracted`.

- [ ] **Step 3: Add the five fields to `ScoredExtracted`**

In `src/scored.rs`, add `#[non_exhaustive]` to the `ScoredExtracted` struct (on the line directly above `pub struct ScoredExtracted {`, after the existing `#[derive(...)]`/`#[cfg_attr(...)]` lines). Then add these five fields at the end of the struct (after `pub persons: Vec<Scored<Person>>,`):

```rust
    /// Scored Handelsregister section (HRA/HRB).
    pub register_type: Option<Scored<String>>,
    /// Scored supervisory authority ("Aufsichtsbehörde").
    pub supervisory_authority: Option<Scored<String>>,
    /// Scored professional chamber ("zuständige Kammer" / "Berufskammer").
    pub professional_chamber: Option<Scored<String>>,
    /// Scored De-Mail address.
    pub de_mail: Option<Scored<String>>,
    /// Scored EU Online-Dispute-Resolution (OS-Plattform) URL.
    pub dispute_resolution_url: Option<Scored<String>>,
```

- [ ] **Step 4: Populate them in `score_extracted`**

In `src/score.rs`, inside the `ScoredExtracted { … }` literal returned by `score_extracted`, add these five fields (after `persons: …,`):

```rust
        register_type: base
            .register_type
            .map(|v| scored(v, 0.85 + bonus(LabelKind::Register))),
        supervisory_authority: base.supervisory_authority.map(|v| scored(v, 0.8)),
        professional_chamber: base.professional_chamber.map(|v| scored(v, 0.75)),
        de_mail: base.de_mail.map(|v| scored(v, 0.9)),
        dispute_resolution_url: base.dispute_resolution_url.map(|v| scored(v, 0.97)),
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test scoring 2>&1 | tail -12` → the new test + existing scoring tests pass.
Run: `cargo test --all-targets 2>&1 | tail -4` → all existing tests pass.
Run: `cargo test --all-targets --features serde 2>&1 | tail -4` → serde round-trip (now with new fields) passes.
Run: `cargo test --all-targets --features html 2>&1 | tail -4` → pass.

- [ ] **Step 6: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 7: Commit**

```bash
git add src/scored.rs src/score.rs tests/scoring.rs
git commit -m "feat(scored): score the five TP5 fields; ScoredExtracted non_exhaustive"
```

---

## Task 2: Docs + CHANGELOG

**Files:** Modify `README.md`, `CHANGELOG.md`.

- [ ] **Step 1: CHANGELOG**

Read `CHANGELOG.md`. Under `## [Unreleased]` → `### Added`, add:

```markdown
- `extract_all_scored` / `ScoredExtracted` now cover the five newer fields
  (register_type, supervisory_authority, professional_chamber, de_mail,
  dispute_resolution_url). `ScoredExtracted` is now `#[non_exhaustive]`.
```

Also, in the earlier TP5 `### Changed` entry, remove or amend the sentence stating "`ScoredExtracted` does not yet cover the five new fields" (it now does). If editing that sentence is awkward, add a clarifying line under `### Changed` noting the gap is closed.

- [ ] **Step 2: README (if applicable)**

If `README.md`'s "Confidence scores" section says scoring omits any fields, update it. Otherwise no change needed (the scored API surface is unchanged apart from the new fields).

- [ ] **Step 3: Verify**

Run: `cargo build --all-features 2>&1 | tail -2` → builds.
Run: `cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 4: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: ScoredExtracted now covers the TP5 fields"
```

---

## Self-Review
**1. Spec coverage:** 5 fields + `#[non_exhaustive]` on ScoredExtracted → Task 1 (scored.rs); population → Task 1 (score.rs); docs → Task 2. ✓
**2. Placeholder scan:** complete code in every step. ✓
**3. Type consistency:** field names match between `ScoredExtracted`, `score_extracted`, and the `Extracted` source fields; all `Option<Scored<String>>`; `scored()` clamps. ✓

**Executor notes:**
- Use `scored()` (clamps) for every new field; do not construct `Scored` directly.
- `bonus(LabelKind::Register)` reuses the existing closure/label; the other four use flat bases (no matching `LabelKind`).
- Do not modify existing `ScoredExtracted` fields or their scoring.
