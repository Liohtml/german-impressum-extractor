# Label Regex Precision Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Line-start-anchor `SUPERVISORY_AUTHORITY_RE` and `CHAMBER_RE` so mid-sentence prose isn't captured, without changing any legitimate behavior.

**Tech Stack:** Rust (edition 2024), existing deps.

## Global Constraints
- MSRV 1.85; `#![forbid(unsafe_code)]`; no unsafe; no new dep; clippy `-D warnings` clean.
- No API/signature change. All existing tests must pass UNCHANGED (legitimate labels start their lines).

---

## Task 1: Anchor the two label regexes + adversarial tests

**Files:** Modify `src/lib.rs`; append to `tests/new_fields.rs`.

- [ ] **Step 1: Write the failing adversarial tests (append to tests/new_fields.rs)**

```rust
#[test]
fn label_regexes_ignore_mid_sentence_prose() {
    // "Berufskammer"/"Aufsichtsbehörde" mid-sentence (not starting a line) must
    // not be captured as a value.
    assert_eq!(
        extract_professional_chamber("Wir sind Mitglied der Berufskammer der Ärzte Bayern."),
        None
    );
    assert_eq!(
        extract_supervisory_authority("Diese Seite unterliegt der Aufsichtsbehörde des Landes."),
        None
    );
    // Legitimate line-start labels still work.
    assert_eq!(
        extract_professional_chamber("Berufskammer: Rechtsanwaltskammer Berlin"),
        Some("Rechtsanwaltskammer Berlin".into())
    );
    assert_eq!(
        extract_supervisory_authority("Aufsichtsbehörde: Landesamt X"),
        Some("Landesamt X".into())
    );
}
```

(The `use german_impressum_extractor::{...}` line in `tests/new_fields.rs` already imports `extract_professional_chamber` and `extract_supervisory_authority`.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --test new_fields label_regexes_ignore 2>&1 | head -15`
Expected: FAIL — the two mid-sentence inputs are currently captured (return `Some`, not `None`).

- [ ] **Step 3: Anchor the regexes**

In `src/lib.rs`, replace the two statics:

```rust
static SUPERVISORY_AUTHORITY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*Aufsichtsbeh(?:ö|oe)rde\s*[:\-]?\s*([^\n]{2,100})").unwrap()
});

static CHAMBER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*(?:zust(?:ä|ae)ndige\s+Kammer|Berufskammer)\s*[:\-]?\s*([^\n]{2,100})")
        .unwrap()
});
```

(Only the leading `(?i)` → `(?im)^\s*` changes; the rest of each pattern is unchanged.)

- [ ] **Step 4: Run tests**

Run: `cargo test --test new_fields 2>&1 | tail -12` → all pass (incl. the new adversarial test, and the pre-existing `professional_chamber_labeled` / `supervisory_authority_labeled` / `de_mail_labeled_only` unchanged).
Run: `cargo test --all-targets 2>&1 | tail -6` → ALL existing tests pass unchanged (esp. the `extract_all`/scored integration tests whose labels start their lines).
Run: `cargo test --all-targets --features html 2>&1 | tail -4` and `--features serde` → pass.

- [ ] **Step 5: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs tests/new_fields.rs
git commit -m "fix(fields): line-start-anchor authority/chamber labels to skip mid-sentence prose"
```

---

## Task 2: Docs + CHANGELOG

**Files:** Modify `CHANGELOG.md`.

- [ ] **Step 1: CHANGELOG**

Read `CHANGELOG.md`. Under `## [Unreleased]` → `### Fixed` (create after `### Added`/`### Changed` if absent), add:

```markdown
- `supervisory_authority` / `professional_chamber` now require their label to
  begin a line, so a mid-sentence mention (e.g. "…Mitglied der Berufskammer…")
  is no longer captured as the value.
```

- [ ] **Step 2: Verify**

Run: `cargo build --all-features 2>&1 | tail -2` → builds.
Run: `cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: note line-start anchoring for authority/chamber labels"
```

---

## Self-Review
**1. Spec coverage:** anchor both regexes + adversarial tests → Task 1; docs → Task 2. ✓
**2. Placeholder scan:** complete code. ✓
**3. Type consistency:** no signature change; only the two regex literals change. ✓

**Executor notes:** if ANY pre-existing test fails after the regex change, STOP and report — the anchoring is designed to preserve all legitimate (line-start) labels, so a failure signals a test whose label wasn't at a line start (investigate, don't weaken).
