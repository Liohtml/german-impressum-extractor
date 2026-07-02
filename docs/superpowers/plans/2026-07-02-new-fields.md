# TP5 New Fields Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add five Impressum fields (`supervisory_authority`, `professional_chamber`, `de_mail`, `dispute_resolution_url`, `register_type`) as public standalone extractors and fields on `Extracted`.

**Architecture:** Each `Option<String>` extractor uses the TP3 wrapper+core pattern (public normalizes once → private `*_core` does the regex). `build_extracted` calls the `*_core` variants on the already-normalized `doc.text()`. `Extracted` gains the five fields and `#[non_exhaustive]`.

**Tech Stack:** Rust (edition 2024), existing deps only (`regex`, `LazyLock`).

## Global Constraints
- MSRV 1.85; `#![forbid(unsafe_code)]`; no unsafe; no new dependency.
- Reads stay non-breaking: existing `extract_all` fields unchanged; all existing tests pass (only additions). Adding fields to `Extracted` is an accepted construction-level change (0.x, unpublished); add `#[non_exhaustive]` to `Extracted`.
- Every new public item (5 fns + 5 fields) needs a `///` doc comment (crate is `#![warn(missing_docs)]` + clippy `-D warnings`).
- `build_extracted` MUST call the `*_core` variants (single-normalize invariant — TP3).
- `ScoredExtracted` is NOT extended in TP5 (deferred; note in docs).

---

## File Structure
- Modify `src/lib.rs` — 4 new regex statics; 5 wrapper+core extractor pairs; 5 new `Extracted` fields + `#[non_exhaustive]`; wire into `build_extracted`.
- Create `tests/new_fields.rs`.
- Modify `README.md`, `CHANGELOG.md`.

---

## Task 1: Label-based extractors (authority, chamber, De-Mail)

**Files:** Modify `src/lib.rs`; create `tests/new_fields.rs`.

**Interfaces produced:** `extract_supervisory_authority`, `extract_professional_chamber`, `extract_de_mail` (each `pub fn (&str) -> Option<String>`), with private `*_core` variants.

- [ ] **Step 1: Write failing tests**

Create `tests/new_fields.rs`:

```rust
use german_impressum_extractor::{
    extract_de_mail, extract_professional_chamber, extract_supervisory_authority,
};

#[test]
fn supervisory_authority_labeled() {
    assert_eq!(
        extract_supervisory_authority("Aufsichtsbehörde: Landesärztekammer Hessen"),
        Some("Landesärztekammer Hessen".into())
    );
    assert_eq!(extract_supervisory_authority("Kein Hinweis hier"), None);
}

#[test]
fn professional_chamber_labeled() {
    assert_eq!(
        extract_professional_chamber("Zuständige Kammer: Rechtsanwaltskammer München"),
        Some("Rechtsanwaltskammer München".into())
    );
    assert_eq!(
        extract_professional_chamber("Berufskammer Steuerberaterkammer Berlin"),
        Some("Steuerberaterkammer Berlin".into())
    );
    assert_eq!(extract_professional_chamber("nichts davon"), None);
}

#[test]
fn de_mail_labeled_only() {
    assert_eq!(
        extract_de_mail("De-Mail: kontakt@firma.de-mail.de"),
        Some("kontakt@firma.de-mail.de".into())
    );
    // A normal e-mail label must NOT be picked up as De-Mail.
    assert_eq!(extract_de_mail("E-Mail: info@firma.de"), None);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --test new_fields 2>&1 | head -15` → FAIL (functions undefined).

- [ ] **Step 3: Add regexes**

In `src/lib.rs`, in the regex-statics area (near the other `static …_RE`), add:

```rust
static SUPERVISORY_AUTHORITY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Aufsichtsbeh(?:ö|oe)rde\s*[:\-]?\s*([^\n]{2,100})").unwrap()
});

static CHAMBER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:zust(?:ä|ae)ndige\s+Kammer|Berufskammer)\s*[:\-]?\s*([^\n]{2,100})").unwrap()
});

static DE_MAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)De-?Mail\s*[:\-]?\s*([a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,})").unwrap()
});
```

- [ ] **Step 4: Add the three wrapper+core extractors**

In `src/lib.rs`, near the other `extract_*` functions, add:

```rust
/// Extract the supervisory authority ("Aufsichtsbehörde"), when labeled.
pub fn extract_supervisory_authority(text: &str) -> Option<String> {
    extract_supervisory_authority_core(&normalize::normalize_text(text))
}

fn extract_supervisory_authority_core(text: &str) -> Option<String> {
    SUPERVISORY_AUTHORITY_RE
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Extract the responsible professional chamber ("zuständige Kammer" / "Berufskammer"), when labeled.
pub fn extract_professional_chamber(text: &str) -> Option<String> {
    extract_professional_chamber_core(&normalize::normalize_text(text))
}

fn extract_professional_chamber_core(text: &str) -> Option<String> {
    CHAMBER_RE
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Extract a De-Mail address, when labeled "De-Mail:".
pub fn extract_de_mail(text: &str) -> Option<String> {
    extract_de_mail_core(&normalize::normalize_text(text))
}

fn extract_de_mail_core(text: &str) -> Option<String> {
    DE_MAIL_RE
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_ascii_lowercase())
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test new_fields 2>&1 | tail -10` → 3 tests pass.
Run: `cargo test --all-targets 2>&1 | tail -4` → existing tests still pass.

- [ ] **Step 6: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs tests/new_fields.rs
git commit -m "feat(fields): supervisory authority, professional chamber, De-Mail extractors"
```

---

## Task 2: `dispute_resolution_url` + `register_type`

**Files:** Modify `src/lib.rs`; append to `tests/new_fields.rs`.

**Interfaces produced:** `extract_dispute_resolution_url`, `extract_register_type` (`pub fn (&str) -> Option<String>`) + `*_core`.

- [ ] **Step 1: Write failing tests (append to tests/new_fields.rs)**

```rust
use german_impressum_extractor::{extract_dispute_resolution_url, extract_register_type};

#[test]
fn odr_url_detected() {
    assert_eq!(
        extract_dispute_resolution_url(
            "Plattform der EU zur OS: https://ec.europa.eu/consumers/odr/ — bitte beachten"
        ),
        Some("https://ec.europa.eu/consumers/odr/".into())
    );
    assert_eq!(extract_dispute_resolution_url("keine url hier"), None);
}

#[test]
fn register_type_from_hr() {
    assert_eq!(extract_register_type("Amtsgericht Berlin HRB 12345 B"), Some("HRB".into()));
    assert_eq!(extract_register_type("Handelsregister HRA 5678"), Some("HRA".into()));
    assert_eq!(extract_register_type("kein register hier"), None);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --test new_fields odr_url 2>&1 | head -12` → FAIL.

- [ ] **Step 3: Add the ODR regex**

In `src/lib.rs` regex area, add:

```rust
static ODR_URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)https?://(?:www\.)?ec\.europa\.eu/consumers/odr/?").unwrap());
```

- [ ] **Step 4: Add the two extractors**

```rust
/// Extract the EU Online-Dispute-Resolution (OS-Plattform) URL, if present.
pub fn extract_dispute_resolution_url(text: &str) -> Option<String> {
    extract_dispute_resolution_url_core(&normalize::normalize_text(text))
}

fn extract_dispute_resolution_url_core(text: &str) -> Option<String> {
    ODR_URL_RE.find(text).map(|m| m.as_str().to_string())
}

/// Extract the Handelsregister section — `"HRA"` or `"HRB"` — from the HR number.
pub fn extract_register_type(text: &str) -> Option<String> {
    extract_register_type_core(&normalize::normalize_text(text))
}

fn extract_register_type_core(text: &str) -> Option<String> {
    let hr = extract_hr_number_core(text)?;
    let upper = hr.to_ascii_uppercase();
    if upper.starts_with("HRB") {
        Some("HRB".to_string())
    } else if upper.starts_with("HRA") {
        Some("HRA".to_string())
    } else {
        None
    }
}
```

(`extract_hr_number_core` already exists from TP3.)

- [ ] **Step 5: Run tests**

Run: `cargo test --test new_fields 2>&1 | tail -10` → all pass (5 tests total).
Run: `cargo test --all-targets 2>&1 | tail -4` → existing tests pass.

- [ ] **Step 6: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs tests/new_fields.rs
git commit -m "feat(fields): EU ODR dispute-resolution URL + Handelsregister type"
```

---

## Task 3: Extend `Extracted` + wire into `build_extracted`

**Files:** Modify `src/lib.rs`; append integration test to `tests/new_fields.rs`.

**Interfaces:** `Extracted` gains `register_type`, `supervisory_authority`, `professional_chamber`, `de_mail`, `dispute_resolution_url` (all `Option<String>`), and `#[non_exhaustive]`.

- [ ] **Step 1: Add `#[non_exhaustive]` + the five fields to `Extracted`**

In `src/lib.rs`, add `#[non_exhaustive]` to the `Extracted` struct (place the attribute on the line directly above `pub struct Extracted {`, after the existing `#[derive(...)]`/`#[cfg_attr(...)]` lines). Then add these five fields at the end of the struct (after `pub persons: Vec<Person>,`):

```rust
    /// Handelsregister section: "HRA" or "HRB", if determinable from the HR number.
    pub register_type: Option<String>,
    /// Supervisory authority ("Aufsichtsbehörde"), if labeled.
    pub supervisory_authority: Option<String>,
    /// Responsible professional chamber ("zuständige Kammer" / "Berufskammer"), if labeled.
    pub professional_chamber: Option<String>,
    /// De-Mail address, if labeled "De-Mail:".
    pub de_mail: Option<String>,
    /// EU Online-Dispute-Resolution (OS-Plattform) URL, if present.
    pub dispute_resolution_url: Option<String>,
```

- [ ] **Step 2: Wire into `build_extracted`**

In `build_extracted`, add these five fields to the returned `Extracted { … }` literal (calling the `*_core` variants on the already-normalized `text`):

```rust
        register_type: extract_register_type_core(text),
        supervisory_authority: extract_supervisory_authority_core(text),
        professional_chamber: extract_professional_chamber_core(text),
        de_mail: extract_de_mail_core(text),
        dispute_resolution_url: extract_dispute_resolution_url_core(text),
```

- [ ] **Step 3: Write the integration test (append to tests/new_fields.rs)**

```rust
use german_impressum_extractor::extract_all;

#[test]
fn extract_all_includes_new_fields() {
    let text = "\
Muster GmbH
Handelsregister HRB 12345
Aufsichtsbehörde: Landesamt für Gesundheit
Zuständige Kammer: IHK Berlin
De-Mail: kontakt@firma.de-mail.de
Online-Streitbeilegung: https://ec.europa.eu/consumers/odr/";
    let d = extract_all(text);
    assert_eq!(d.register_type.as_deref(), Some("HRB"));
    assert_eq!(d.supervisory_authority.as_deref(), Some("Landesamt für Gesundheit"));
    assert_eq!(d.professional_chamber.as_deref(), Some("IHK Berlin"));
    assert_eq!(d.de_mail.as_deref(), Some("kontakt@firma.de-mail.de"));
    assert_eq!(d.dispute_resolution_url.as_deref(), Some("https://ec.europa.eu/consumers/odr/"));
    // Existing fields still work.
    assert_eq!(d.legal_form.as_deref(), Some("GmbH"));
    assert_eq!(d.hr_number.as_deref(), Some("HRB 12345"));
}
```

- [ ] **Step 4: Run everything**

Run: `cargo test --test new_fields 2>&1 | tail -12` → all pass.
Run: `cargo test --all-targets 2>&1 | tail -6` → all existing tests pass. (If a pre-existing test constructs a full `Extracted { .. }` literal it will now fail to compile — update it to add the new fields with their expected values or `..Default::default()`; this is a necessary, non-weakening update. The current tests read fields via `d.field`, so none should need changes.)
Run: `cargo test --all-targets --features html 2>&1 | tail -4` and `cargo test --all-targets --features serde 2>&1 | tail -4` → pass.

- [ ] **Step 5: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs tests/new_fields.rs
git commit -m "feat(extracted): add 5 new fields + non_exhaustive; wire into extract_all"
```

---

## Task 4: Docs + CHANGELOG

**Files:** Modify `README.md`, `CHANGELOG.md`.

- [ ] **Step 1: README**

Read `README.md`. In the "What it extracts" list, add bullets for the five new fields (supervisory authority, professional chamber, De-Mail, EU ODR link, Handelsregister type HRA/HRB). If there's a granular-extractors code list, add the five new `extract_*` function names.

- [ ] **Step 2: CHANGELOG**

Under `## [Unreleased]` → `### Added`, add:

```markdown
- New fields + extractors: `supervisory_authority` (Aufsichtsbehörde),
  `professional_chamber` (zuständige Kammer / Berufskammer), `de_mail`,
  `dispute_resolution_url` (EU OS-Plattform / ODR link), and `register_type`
  (HRA/HRB). Added to `Extracted` and as standalone `extract_*` functions.
```

Under `### Changed` (create after `### Added` if absent), add:

```markdown
- `Extracted` is now `#[non_exhaustive]` (construct it via `..Default::default()`
  or obtain it from `extract_all`). `ScoredExtracted` does not yet cover the five
  new fields.
```

- [ ] **Step 3: Verify**

Run: `cargo build --all-features 2>&1 | tail -2` → builds.
Run: `cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 4: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: document the five new Impressum fields"
```

---

## Self-Review
**1. Spec coverage:** 3 label extractors → Task 1; ODR url + register_type → Task 2; `Extracted` fields + `#[non_exhaustive]` + wiring → Task 3; docs → Task 4. ✓
**2. Placeholder scan:** complete code in every step. ✓
**3. Type consistency:** all five extractors `(&str) -> Option<String>` with `*_core`; `build_extracted` calls the cores; field names match between `Extracted` and `build_extracted`. ✓

**Executor notes:**
- `build_extracted` MUST call the `*_core` variants (never the public wrappers) — this preserves the single-normalize invariant from TP3.
- Public wrappers carry the `///` docs; private `*_core` don't need them.
- Keep the `#[allow(dead_code)]` on `mod segment;`/`mod candidate;`.
- Do not extend `ScoredExtracted` (out of scope; noted in CHANGELOG).
