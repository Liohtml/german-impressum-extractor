# TP3 Precision Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the standalone `extract_*` functions normalize their input (parity with `extract_all`), and harden `persons` against non-name noise — both safe and non-breaking.

**Architecture:** Prepend a normalization shadow to each public extractor; add two guards to the person name-token filter. No API changes, no new modules, no value-dropping validation (that would fight the best-guess+score model).

**Tech Stack:** Rust (edition 2024), existing deps only.

## Global Constraints

- MSRV `rust-version = "1.85"`; no APIs newer than 1.85. `#![forbid(unsafe_code)]`; no `unsafe`.
- Non-breaking: `extract_all`, `extract_all_html`, `extract_all_scored*`, all public signatures, and `Extracted`/`Person`/`ScoredExtracted` unchanged; ALL existing tests pass unchanged (re-normalization is idempotent; persons change only drops garbage).
- No new dependency. CI gate `cargo clippy --all-targets --all-features -- -D warnings` clean; keep the interim `#[allow(dead_code)]` on `mod segment;`/`mod candidate;`.
- Do NOT add checksum/validation that DROPS a returned value from any extractor (breaks the model + TP2 tests). Scoring already handles validity.

---

## File Structure
- Modify `src/lib.rs` — prepend normalization to 12 extractors (Task 1); harden `is_valid_name_part` + extend `NOT_A_NAME` (Task 2).
- Create `tests/hardening.rs` — parity + persons-noise tests (Tasks 1 & 2).
- Modify `README.md`, `CHANGELOG.md` (Task 3).

---

## Task 1: Normalize the standalone extractors

**Files:**
- Modify: `src/lib.rs` (12 functions)
- Test: `tests/hardening.rs` (create)

**Interfaces:** No signature changes. `normalize::normalize_text(&str) -> String` already exists (TP1) and `mod normalize;` is already declared.

- [ ] **Step 1: Write the failing parity test**

Create `tests/hardening.rs`:

```rust
use german_impressum_extractor::{extract_all, extract_emails, extract_fax, extract_persons};

// Messy input: NBSP (U+00A0), CRLF, a soft hyphen, and a well-formed entity.
const MESSY: &str = "Firma\u{00AD} GmbH\r\nTelefon:\u{00A0}+49 30 1234567\r\nFax: +49 30 1234568\r\nE-Mail: info&amp;#64;beispiel.de";

#[test]
fn standalone_fax_and_emails_match_extract_all_on_messy_input() {
    // extract_all normalizes; after this task the standalone fns do too, so
    // these fields must agree on the SAME messy input. (Phones intentionally
    // not compared: extract_all removes the fax from `phones`, extract_phones
    // does not — that difference is by design, not a normalization gap.)
    let d = extract_all(MESSY);
    assert_eq!(extract_fax(MESSY), d.fax, "fax parity");
    assert_eq!(extract_emails(MESSY), d.emails, "email parity");
}

#[test]
fn standalone_email_decodes_entity_and_ignores_nbsp() {
    // &#64; is '@'; NBSP around the address must not break extraction.
    let e = extract_emails("Mail:\u{00A0}info&amp;#64;beispiel.de");
    assert_eq!(e, vec!["info@beispiel.de".to_string()]);
}

#[test]
fn persons_still_extracted_after_normalization() {
    let p = extract_persons("Gesch\u{00E4}ftsf\u{00FC}hrer: Dr. Hans M\u{00FC}ller");
    assert!(p.iter().any(|x| x.last_name.as_deref() == Some("Müller")));
}
```

Note for implementer: if the exact `MESSY` fax/email values differ from expectation, DO NOT weaken the parity assertions — the point is `standalone == extract_all` on the SAME input; fix the normalization, not the test.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --test hardening standalone_email 2>&1 | tail -20`
Expected: FAIL — `extract_emails` does not decode the `&#64;` entity / NBSP (it doesn't normalize yet), so the address isn't produced.

- [ ] **Step 3: Prepend normalization to each extractor**

In `src/lib.rs`, at the top of EACH of these functions, insert these two lines as the first statements:

```rust
    let normalized = normalize::normalize_text(text);
    let text = normalized.as_str();
```

Functions to edit (all take `text: &str`): `extract_emails`, `extract_fax`, `extract_iban`, `extract_bic`, `extract_legal_form`, `extract_hr_number`, `extract_hr_court`, `extract_tax_number`, `extract_vat_id`, `extract_year_founded`, `extract_persons`.

For `extract_phones` (a one-liner `collect_phones(text, &[])`), rewrite it to:

```rust
pub fn extract_phones(text: &str) -> Vec<String> {
    let normalized = normalize::normalize_text(text);
    collect_phones(normalized.as_str(), &[])
}
```

Do NOT modify `extract_address` (already normalizes), `extract_all`, `extract_all_html`, `extract_all_scored`, `extract_all_scored_html`, `build_extracted`, or `collect_phones`.

- [ ] **Step 4: Run tests**

Run: `cargo test --test hardening 2>&1 | tail -15` → the parity + entity tests pass.
Run: `cargo test --all-targets 2>&1 | tail -6` → ALL existing tests still pass (idempotence).
Run: `cargo test --all-targets --features html 2>&1 | tail -4` → pass.

- [ ] **Step 5: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs tests/hardening.rs
git commit -m "feat(extract): normalize input in standalone extractors (parity with extract_all)"
```

---

## Task 2: Harden `persons` against non-name noise

**Files:**
- Modify: `src/lib.rs` (`NOT_A_NAME` constant; `is_valid_name_part`)
- Test: append to `tests/hardening.rs`

**Interfaces:** none changed.

- [ ] **Step 1: Write the failing tests**

Append to `tests/hardening.rs`:

```rust
#[test]
fn persons_rejects_digit_tokens_and_noise_nouns() {
    // "Webdesign" is a common footer noise word; must not become a surname.
    let p = extract_persons("Inhaber: Webdesign Berlin");
    assert!(
        !p.iter().any(|x| x.last_name.as_deref() == Some("Webdesign")
            || x.first_name.as_deref() == Some("Webdesign")),
        "noise noun leaked as name: {p:?}"
    );

    // A token containing a digit is not a name part.
    let p2 = extract_persons("Geschäftsführer: Hans Müller2");
    assert!(
        !p2.iter().any(|x| x.last_name.as_deref() == Some("Müller2")),
        "digit-bearing token leaked as name: {p2:?}"
    );

    // Real name still works.
    let p3 = extract_persons("Geschäftsführer: Dr. Hans Müller");
    assert!(p3.iter().any(|x| x.last_name.as_deref() == Some("Müller")));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --test hardening persons_rejects 2>&1 | tail -20`
Expected: FAIL — "Webdesign" is currently returned as a name, and "Müller2" is accepted.

- [ ] **Step 3: Add the digit guard to `is_valid_name_part`**

In `src/lib.rs`, in `is_valid_name_part`, after the `trimmed.chars().count() <= 1` check, add a digit rejection:

```rust
    if trimmed.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
```

(Place it before the `NOT_A_NAME.contains(...)` check.)

- [ ] **Step 4: Extend `NOT_A_NAME`**

In `src/lib.rs`, add these entries to the `NOT_A_NAME` array (append near its end, keeping the existing entries; all lowercase):

```rust
    // Impressum footer / contact noise nouns that leak in as fake names.
    "team",
    "kontakt",
    "impressum",
    "datenschutz",
    "vertrieb",
    "büro",
    "sekretariat",
    "webdesign",
    "webseite",
    "homepage",
    "copyright",
    "firma",
    "unternehmen",
    "postfach",
    "telefon",
    "telefax",
    "mobil",
    "adresse",
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test hardening 2>&1 | tail -12` → pass.
Run: `cargo test --all-targets 2>&1 | tail -6` → all existing tests still pass.

- [ ] **Step 6: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs tests/hardening.rs
git commit -m "feat(persons): reject digit tokens and extend non-name blocklist"
```

---

## Task 3: Docs + CHANGELOG

**Files:**
- Modify: `README.md`, `CHANGELOG.md`

- [ ] **Step 1: README — note standalone normalization**

Read `README.md`. In the "Granular extractors" area (or the "Robustness & limits" section), add a sentence:

```markdown
All granular `extract_*` functions normalize their input the same way `extract_all`
does (Unicode/whitespace cleanup, HTML-entity decoding), so calling them directly
gives the same result as the corresponding field of `extract_all`.
```

If a "Robustness & limits" bullet claims only `extract_all` normalizes, update it accordingly.

- [ ] **Step 2: CHANGELOG**

Under `## [Unreleased]` → `### Fixed` (create the subheading if absent, after `### Added`), add:

```markdown
- Standalone `extract_*` functions now normalize their input (Unicode, whitespace,
  HTML entities) like `extract_all`, so direct calls no longer diverge from the
  corresponding `extract_all` field on messy input.
- `extract_persons` rejects tokens containing digits and an expanded set of
  non-name noise nouns (team, kontakt, webdesign, …), reducing false persons.
```

- [ ] **Step 3: Verify**

Run: `cargo build --all-features 2>&1 | tail -2` → builds.
Run: `cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 4: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: standalone-extractor normalization + persons hardening notes"
```

---

## Self-Review

**1. Spec coverage:** standalone normalization (12 fns) → Task 1; persons digit-guard + blocklist → Task 2; docs → Task 3. No value-dropping validation added (model-consistent). ✓
**2. Placeholder scan:** complete code in every step. ✓
**3. Type consistency:** no signature changes; `normalize::normalize_text` used consistently; `is_valid_name_part`/`NOT_A_NAME` are the only person-path edits. ✓

**Executor notes:**
- Re-normalization must stay idempotent — if any EXISTING test changes output, stop and report (would signal a normalize non-idempotency bug, not a test to edit).
- Do not touch `extract_all`/`build_extracted` or the scored path.
- Keep the `#[allow(dead_code)]` on `mod segment;`/`mod candidate;`.
