# TP3: Precision hardening (standalone normalization + persons noise guards)

**Status:** Approved design (2026-07-02) — autonomous continuation (Spec→PR without per-gate acceptance).
**Sub-project:** TP3 of 5. Builds on TP1 (normalization/segmentation) and TP2 (scoring), both merged.

## Rationale / model constraint
The project chose a **best-guess + confidence** model (TP2): extractors return their best candidate; a *score* reflects trust. So "validation" that would **drop** a plausible-but-checksum-invalid value (e.g. rejecting a mod-97-invalid IBAN in `extract_iban`) is the wrong layer — TP2 already scores such a value low, and dropping it would (a) contradict the model and (b) break TP2's `invalid_iban_scores_below_valid_iban` test. TP3 therefore does **not** add value-dropping validation to extractors.

What TP3 *does* deliver — two safe, model-consistent precision improvements:

## Scope
**In scope**
1. **Normalize the standalone extractors.** Every public `extract_*(&str)` normalizes its input (Unicode/whitespace/entity cleanup, via TP1's `normalize_text`) before matching — so calling e.g. `extract_fax(raw)` gives the same result as `extract_all(raw).fax`. Closes the divergence flagged in TP1/TP2 reviews (only `extract_all`/`extract_all_html`/`extract_address` normalized before).
2. **Harden `persons` against non-name noise.** Reject name tokens containing digits, and extend the `NOT_A_NAME` blocklist with common Impressum noise nouns (`team`, `kontakt`, `impressum`, `datenschutz`, `vertrieb`, `büro`, `sekretariat`, `webdesign`, `homepage`, `copyright`, `firma`, `unternehmen`, `postfach`, `telefon`, `telefax`, `mobil`, `adresse`) that otherwise leak in as fake surnames. Dropping obvious garbage is consistent with best-guess (garbage isn't a guess).

**Out of scope**
- Value-dropping checksum validation in extractors (belongs to scoring; would break the model). USt "checksum" (no standard public one). Multi-entity (TP4). New fields (TP5).

## Design

### 1. Standalone normalization
Each public `extract_*` prepends:

```rust
let normalized = normalize::normalize_text(text);
let text = normalized.as_str();
```

This shadows the `&str` param with a normalized `&str`, so the rest of each body is unchanged. Functions covered: `extract_emails`, `extract_phones`, `extract_fax`, `extract_iban`, `extract_bic`, `extract_legal_form`, `extract_hr_number`, `extract_hr_court`, `extract_tax_number`, `extract_vat_id`, `extract_year_founded`, `extract_persons`. (`extract_address` already normalizes.)

`extract_all`/`build_extracted` keep calling these on the already-normalized `doc.text()`; re-normalization is **idempotent** (NFC, whitespace-collapse, entity-decode, zero-width-strip all stabilize on a normalized string), so `extract_all` output is unchanged and existing tests stay green. The minor redundant work (≤ a dozen normalize passes per `extract_all`) is acceptable for this non-perf-critical, small-input crate.

### 2. persons noise guards
- `is_valid_name_part`: additionally return `false` if the token contains any ASCII digit.
- `NOT_A_NAME`: append the noise nouns listed above (all lowercase; the check lowercases first). Verified none is a plausible German surname.

## Error handling / constraints
- Infallible/panic-free; `#![forbid(unsafe_code)]`; MSRV 1.85; no new dependency.
- Non-breaking: all existing public signatures, `extract_all`, and all existing tests unchanged (idempotence + only-adds-rejections-of-garbage).

## Testing
- Existing suite unchanged (idempotence).
- Standalone normalization: `extract_emails("info&amp;#64;a.de")`-style entity / NBSP / CRLF inputs now extract correctly; parity: `extract_fax(raw_messy) == extract_all(raw_messy).fax` for a messy input.
- persons guards: a token with a digit is dropped; a blocklisted noise noun (e.g. "Team", "Webdesign") is not returned as a surname; a real name ("Dr. Hans Müller") is still extracted intact.

## Success criteria
1. `extract_all` and all existing tests unchanged (non-breaking).
2. For a messy input, `extract_fax`/`extract_emails`/… standalone == the corresponding `extract_all` field (parity).
3. `persons` no longer returns digit-bearing tokens or the listed noise nouns as names; real names still extracted.
4. No new dep; builds + tests on Rust 1.85; clippy `-D warnings` clean.
