# TP1 Foundation (Normalization + Segmentation + Candidate) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Insert a normalization + segmentation layer (text and feature-gated HTML) with an internal `Candidate<T>` provenance substrate, and use it to fix cross-block address mixing — all additive and non-breaking.

**Architecture:** Both input paths converge on one canonical string (`\n` = block/line boundary, `\t` = label→value separator). `Document::parse` segments it into labeled blocks/segments. Existing extractors run on the normalized string; the address extractor becomes block-aware via `Candidate<T>`. HTML support lives behind a `html` cargo feature backed by the `html5gum` tokenizer.

**Tech Stack:** Rust (edition 2024), `regex`, `unicode-normalization`, `html5gum` (feature-gated, `default-features = false`).

## Global Constraints

- MSRV: `rust-version = "1.85"` — no APIs newer than 1.85 (no `str::floor_char_boundary`; use the existing crate helper).
- `#![forbid(unsafe_code)]` stays; write no `unsafe`.
- Non-breaking: `extract_all(&str) -> Extracted` signature and the `Extracted`/`Person` types are unchanged; all existing tests stay green.
- Default build gains no new dependencies. HTML support is only under `--features html`.
- Infallible/panic-free: normalization, segmentation, and the flattener return data (no `Result`), never panic, no `unwrap` on parser output.
- All slicing is char-boundary-safe (reuse the existing `floor_char_boundary` helper in `src/lib.rs`).
- Canonical intermediate format: `\n` separates lines/blocks (one blank line = block boundary); `\t` separates a label from its value within a line. Tabs are preserved by normalization (they are meaningful separators), not converted to spaces.

---

## File Structure

- Create `src/normalize.rs` — `normalize_text`, `decode_entities`, whitespace/control-char cleanup. Always compiled.
- Create `src/segment.rs` — `LabelKind`, `Segment`, `Block`, `Document`, `Document::parse`, label classification.
- Create `src/candidate.rs` — internal `Candidate<T>`.
- Create `src/html.rs` — `HtmlFlattener` trait, `DefaultFlattener`, flattening rules. Only under `#[cfg(feature = "html")]`.
- Modify `src/lib.rs` — module declarations, re-exports, wire `extract_all` through normalize+segment, add block-aware address logic, add `extract_all_html` + `html_to_impressum_text`.
- Modify `Cargo.toml` — add optional `html5gum` dep + `html` feature.
- Modify `.github/workflows/ci.yml` — add `--features html` to the test matrix.
- Modify `README.md` and `CHANGELOG.md` — document HTML support.
- Create `tests/foundation.rs` — integration tests (address demonstrator, HTML equivalence, adversarial no-panic).

---

## Task 1: Text normalization (`normalize.rs`)

**Files:**
- Create: `src/normalize.rs`
- Modify: `src/lib.rs` (add `mod normalize;` after the existing `use` block, near line 80)
- Test: unit tests inside `src/normalize.rs`

**Interfaces:**
- Produces: `pub(crate) fn normalize_text(input: &str) -> String`

- [ ] **Step 1: Declare the module**

In `src/lib.rs`, immediately after the `use unicode_normalization::UnicodeNormalization;` line, add:

```rust
mod normalize;
```

- [ ] **Step 2: Write the failing tests**

Create `src/normalize.rs`:

```rust
//! Turn raw text (or, via `html.rs`, flattened HTML) into one canonical string:
//! `\n` separates lines/blocks, `\t` separates a label from its value, and
//! invisible/duplicate whitespace and well-formed HTML entities are cleaned up.

use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

// Collapses 3+ consecutive newlines down to exactly two (one blank line = one
// block boundary).
static BLANK_LINES_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_zero_width_and_soft_hyphen() {
        assert_eq!(normalize_text("Mül\u{00AD}ler\u{200B}"), "Müller");
    }

    #[test]
    fn normalizes_nbsp_and_crlf_and_collapses_spaces() {
        assert_eq!(
            normalize_text("Hans\u{00A0}Müller\r\nSitz   Berlin"),
            "Hans Müller\nSitz Berlin"
        );
    }

    #[test]
    fn decodes_well_formed_entities_only() {
        assert_eq!(normalize_text("M\u{00FC}ller &amp; S\u{00F6}hne"), "Müller & Söhne");
        assert_eq!(normalize_text("&uuml;ber &#252;ber &#xFC;ber"), "über über über");
        // Malformed / no semicolon: left untouched.
        assert_eq!(normalize_text("R&D Abteilung"), "R&D Abteilung");
    }

    #[test]
    fn preserves_tabs_and_collapses_blank_lines() {
        assert_eq!(normalize_text("Telefon\t030\n\n\n\nSitz"), "Telefon\t030\n\nSitz");
    }

    #[test]
    fn never_panics_on_multibyte_garbage() {
        for s in ["\u{200B}\u{00AD}", "€\t\r\n", "ä".repeat(50).as_str(), "&#;&#x;&amp"] {
            let _ = normalize_text(s);
        }
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib normalize 2>&1 | head -20`
Expected: FAIL — `cannot find function normalize_text`.

- [ ] **Step 4: Implement `normalize_text` and `decode_entities`**

Add to `src/normalize.rs` (above the `#[cfg(test)]` module):

```rust
/// Normalize raw text into the canonical intermediate form.
pub(crate) fn normalize_text(input: &str) -> String {
    // 1. Unicode NFC once.
    let nfc: String = input.nfc().collect();
    // 2. Unify line endings before per-char work.
    let unified = nfc.replace("\r\n", "\n").replace('\r', "\n");
    // 3. Decode well-formed HTML entities.
    let decoded = decode_entities(&unified);
    // 4. Char-level cleanup: drop invisibles, fold whitespace (keep \n and \t).
    let mut cleaned = String::with_capacity(decoded.len());
    for ch in decoded.chars() {
        match ch {
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{00AD}' | '\u{2060}' => {}
            '\n' => cleaned.push('\n'),
            '\t' => cleaned.push('\t'),
            c if c.is_whitespace() => cleaned.push(' '),
            c => cleaned.push(c),
        }
    }
    // 5. Per-line: collapse runs of spaces and runs of tabs, trim line ends.
    let mut lines: Vec<String> = Vec::new();
    for line in cleaned.split('\n') {
        lines.push(collapse_line_ws(line));
    }
    let joined = lines.join("\n");
    // 6. Collapse blank-line runs to a single blank line.
    BLANK_LINES_RE.replace_all(&joined, "\n\n").into_owned()
}

/// Collapse consecutive spaces (and consecutive tabs) to one, and trim leading
/// and trailing spaces/tabs. Interior single tabs (label/value separators) and
/// other content are preserved.
fn collapse_line_ws(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut prev_space = false;
    let mut prev_tab = false;
    for ch in line.chars() {
        match ch {
            ' ' => {
                if !prev_space {
                    out.push(' ');
                }
                prev_space = true;
                prev_tab = false;
            }
            '\t' => {
                if !prev_tab {
                    out.push('\t');
                }
                prev_tab = true;
                prev_space = false;
            }
            c => {
                out.push(c);
                prev_space = false;
                prev_tab = false;
            }
        }
    }
    out.trim_matches(|c| c == ' ' || c == '\t').to_string()
}

/// Decode only well-formed HTML entities: named (from a small table), decimal
/// `&#NNN;`, and hex `&#xHH;`. Anything not recognized is emitted verbatim.
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '&' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // Find a terminating ';' within a short window.
        let end = (i + 1..chars.len().min(i + 12)).find(|&j| chars[j] == ';');
        match end {
            Some(j) => {
                let entity: String = chars[i + 1..j].iter().collect();
                if let Some(decoded) = decode_one_entity(&entity) {
                    out.push_str(&decoded);
                    i = j + 1;
                } else {
                    out.push('&');
                    i += 1;
                }
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

/// Decode the inner text of one entity (without `&`/`;`). Returns None if unknown.
fn decode_one_entity(body: &str) -> Option<String> {
    if let Some(num) = body.strip_prefix('#') {
        let code = if let Some(hex) = num.strip_prefix(['x', 'X']) {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            num.parse::<u32>().ok()?
        };
        return char::from_u32(code).map(|c| c.to_string());
    }
    let c = match body {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{00A0}',
        "auml" => 'ä',
        "ouml" => 'ö',
        "uuml" => 'ü',
        "Auml" => 'Ä',
        "Ouml" => 'Ö',
        "Uuml" => 'Ü',
        "szlig" => 'ß',
        "eacute" => 'é',
        "egrave" => 'è',
        "agrave" => 'à',
        "ccedil" => 'ç',
        "euro" => '€',
        "copy" => '©',
        "reg" => '®',
        "ndash" => '–',
        "mdash" => '—',
        _ => return None,
    };
    Some(c.to_string())
}
```

Note: `'\u{00A0}'` (from `&nbsp;`) is folded to a normal space by step 4 because it is whitespace — that is intended.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib normalize 2>&1 | tail -20`
Expected: PASS (5 tests).

- [ ] **Step 6: Verify the crate still builds and existing tests pass**

Run: `cargo test --all-targets 2>&1 | tail -5`
Expected: all existing tests still PASS (the module is not wired into `extract_all` yet).

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs src/normalize.rs
git commit -m "feat(normalize): canonical text normalization layer"
```

---

## Task 2: Segmentation (`segment.rs`)

**Files:**
- Create: `src/segment.rs`
- Modify: `src/lib.rs` (add `mod segment;` next to `mod normalize;`)
- Test: unit tests inside `src/segment.rs`

**Interfaces:**
- Consumes: `crate::normalize::normalize_text` (indirectly — callers pass already-normalized text to `Document::parse`).
- Produces:
  - `pub(crate) enum LabelKind { Phone, Fax, Email, Postal, Managers, VatId, TaxNumber, Register, Court, Bank, LegalName, Founded, Web, Other }`
  - `pub(crate) struct Document` with `pub(crate) fn parse(text: String) -> Document`, `pub(crate) fn text(&self) -> &str`, `pub(crate) fn block_texts(&self) -> impl Iterator<Item = &str>`
  - `pub(crate) struct Segment { pub(crate) span: Range<usize>, pub(crate) label: Option<LabelKind>, pub(crate) value_span: Range<usize> }`

- [ ] **Step 1: Declare the module**

In `src/lib.rs`, next to `mod normalize;`, add:

```rust
mod segment;
```

- [ ] **Step 2: Write the failing tests**

Create `src/segment.rs`:

```rust
//! Split the canonical normalized string into labeled blocks and segments.
//! A blank line separates blocks; a `\t` or a leading `Label:` marks a
//! label→value segment.

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

/// Kind of label a segment carries, if any. Broader than TP1 uses; later
/// sub-projects (scoring, precision) reuse it as a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelKind {
    Phone,
    Fax,
    Email,
    Postal,
    Managers,
    VatId,
    TaxNumber,
    Register,
    Court,
    Bank,
    LegalName,
    Founded,
    Web,
    Other,
}

// Matches a leading "Label: value" shape. Group 1 = label, group 2 = value.
static LABEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^\s*([\p{L}.\-/ ]{1,30}?)\s*:\s*(.*)$").unwrap()
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_blocks_on_blank_line() {
        let doc = Document::parse("A GmbH\nHauptstr. 1\n\nDatenschutz\nText".to_string());
        let blocks: Vec<&str> = doc.block_texts().collect();
        assert_eq!(blocks, vec!["A GmbH\nHauptstr. 1", "Datenschutz\nText"]);
    }

    #[test]
    fn detects_tab_label_value() {
        let doc = Document::parse("Telefon\t030 123".to_string());
        let seg = doc.first_segment();
        assert_eq!(seg.label, Some(LabelKind::Phone));
        assert_eq!(&doc.text()[seg.value_span.clone()], "030 123");
    }

    #[test]
    fn detects_colon_label_value() {
        let doc = Document::parse("USt-IdNr.: DE123456789".to_string());
        let seg = doc.first_segment();
        assert_eq!(seg.label, Some(LabelKind::VatId));
        assert_eq!(&doc.text()[seg.value_span.clone()], "DE123456789");
    }

    #[test]
    fn plain_line_has_no_label_and_full_value_span() {
        let doc = Document::parse("Musterreinigung GmbH".to_string());
        let seg = doc.first_segment();
        assert_eq!(seg.label, None);
        assert_eq!(&doc.text()[seg.value_span.clone()], "Musterreinigung GmbH");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib segment 2>&1 | head -20`
Expected: FAIL — `cannot find type Document` / method not found.

- [ ] **Step 4: Implement the model, classification, and `parse`**

Add to `src/segment.rs` (above the `#[cfg(test)]` module):

```rust
pub(crate) struct Segment {
    pub(crate) span: Range<usize>,
    pub(crate) label: Option<LabelKind>,
    pub(crate) value_span: Range<usize>,
}

struct Block {
    span: Range<usize>,
}

pub(crate) struct Document {
    text: String,
    blocks: Vec<Block>,
    segments: Vec<Segment>,
}

impl Document {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn block_texts(&self) -> impl Iterator<Item = &str> {
        self.blocks.iter().map(move |b| &self.text[b.span.clone()])
    }

    #[cfg(test)]
    fn first_segment(&self) -> &Segment {
        &self.segments[0]
    }

    /// Segment already-normalized text into blocks and labeled segments.
    pub(crate) fn parse(text: String) -> Document {
        let mut blocks = Vec::new();
        let mut segments = Vec::new();
        let mut block_start: Option<usize> = None;
        let mut block_end = 0usize;
        let mut pos = 0usize;

        for line in text.split_inclusive('\n') {
            let content_len = line.trim_end_matches('\n').len();
            let start = pos;
            let end = start + content_len;
            pos += line.len();

            let content = &text[start..end];
            if content.trim().is_empty() {
                // Blank line closes the current block.
                if let Some(bs) = block_start.take() {
                    blocks.push(Block { span: bs..block_end });
                }
                continue;
            }
            if block_start.is_none() {
                block_start = Some(start);
            }
            block_end = end;
            segments.push(build_segment(&text, start..end));
        }
        if let Some(bs) = block_start.take() {
            blocks.push(Block { span: bs..block_end });
        }

        Document { text, blocks, segments }
    }
}

fn build_segment(text: &str, span: Range<usize>) -> Segment {
    let line = &text[span.clone()];
    // 1. Tab-separated label/value (from HTML dt/dd or table cells).
    if let Some(tab) = line.find('\t') {
        let label_text = line[..tab].trim();
        let value_rel_start = span.start + tab + 1;
        let value_span = value_rel_start..span.end;
        return Segment {
            label: Some(classify_label(label_text)),
            span,
            value_span,
        };
    }
    // 2. "Label: value" shape.
    if let Some(caps) = LABEL_RE.captures(line) {
        let label_text = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        if let Some(value) = caps.get(2) {
            let value_span = (span.start + value.start())..(span.start + value.end());
            return Segment {
                label: Some(classify_label(label_text)),
                span,
                value_span,
            };
        }
    }
    // 3. Plain line.
    Segment {
        label: None,
        value_span: span.clone(),
        span,
    }
}

/// Map a label string to a `LabelKind` (case-insensitive substring match).
fn classify_label(label: &str) -> LabelKind {
    let l = label.to_lowercase();
    let has = |k: &str| l.contains(k);
    if has("telefax") || has("fax") {
        LabelKind::Fax
    } else if has("telefon") || has("tel.") || has("tel ") || has("phone") || l == "tel" {
        LabelKind::Phone
    } else if has("mail") {
        LabelKind::Email
    } else if has("geschäftsführ") || has("geschaeftsführ") || has("inhaber")
        || has("vorstand") || has("vertretungsberechtigt") || has("verantwortlich")
    {
        LabelKind::Managers
    } else if has("ust") || has("umsatzsteuer") || has("vat") {
        LabelKind::VatId
    } else if has("steuernummer") || has("steuer-nr") || has("st.-nr") || has("stnr") {
        LabelKind::TaxNumber
    } else if has("registergericht") || has("amtsgericht") || (has("gericht") && !has("register")) {
        LabelKind::Court
    } else if has("handelsregister") || has("registernummer") || l == "hrb" || l == "hra" {
        LabelKind::Register
    } else if has("iban") || has("bic") || has("bank") {
        LabelKind::Bank
    } else if has("sitz") || has("adresse") || has("anschrift") {
        LabelKind::Postal
    } else if has("gegründet") || has("gegruendet") || has("seit") || has("gründungsjahr") {
        LabelKind::Founded
    } else if has("web") || has("internet") || has("homepage") {
        LabelKind::Web
    } else {
        LabelKind::Other
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib segment 2>&1 | tail -20`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/segment.rs
git commit -m "feat(segment): block/segment model with label detection"
```

---

## Task 3: Candidate substrate (`candidate.rs`)

**Files:**
- Create: `src/candidate.rs`
- Modify: `src/lib.rs` (add `mod candidate;`)
- Test: unit test inside `src/candidate.rs`

**Interfaces:**
- Produces: `pub(crate) struct Candidate<T> { pub(crate) value: T, pub(crate) span: Range<usize>, pub(crate) block: usize, pub(crate) label: Option<LabelKind> }` and `Candidate::new(value, span, block, label)`.

- [ ] **Step 1: Declare the module**

In `src/lib.rs`, next to the other new `mod` lines, add:

```rust
mod candidate;
```

- [ ] **Step 2: Write the failing test**

Create `src/candidate.rs`:

```rust
//! Internal provenance substrate: a value plus where it came from. Confidence
//! scoring (TP2) will extend this; kept `pub(crate)` so that is not a breaking
//! change.

use std::ops::Range;

use crate::segment::LabelKind;

pub(crate) struct Candidate<T> {
    pub(crate) value: T,
    pub(crate) span: Range<usize>,
    pub(crate) block: usize,
    pub(crate) label: Option<LabelKind>,
}

impl<T> Candidate<T> {
    pub(crate) fn new(value: T, span: Range<usize>, block: usize, label: Option<LabelKind>) -> Self {
        Candidate { value, span, block, label }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_value_and_provenance() {
        let c = Candidate::new("10115".to_string(), 3..8, 2, Some(LabelKind::Postal));
        assert_eq!(c.value, "10115");
        assert_eq!(c.span, 3..8);
        assert_eq!(c.block, 2);
        assert_eq!(c.label, Some(LabelKind::Postal));
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib candidate 2>&1 | head -20`
Expected: FAIL — `cannot find ... Candidate` (module not yet declared / compiled).

- [ ] **Step 4: (implementation already written in Step 2 file)**

No additional code — the struct is the deliverable. Proceed to verify.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib candidate 2>&1 | tail -20`
Expected: PASS (1 test). If clippy later flags `value`/`label` as unused, that is resolved in Task 4 which consumes them; if a dead-code warning blocks a `-D warnings` build here, add `#[allow(dead_code)]` on the struct and remove it in Task 4.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/candidate.rs
git commit -m "feat(candidate): internal provenance substrate"
```

---

## Task 4: Block-aware address + wire `extract_all` through normalize/segment

**Files:**
- Modify: `src/lib.rs` — `extract_all` (around line 405), `extract_address` (around line 519); add `build_extracted` and `address_from_document`.
- Test: `tests/foundation.rs` (create)

**Interfaces:**
- Consumes: `normalize::normalize_text`, `segment::{Document, LabelKind}`, `candidate::Candidate`, existing statics `GERMAN_POSTCODE_AND_CITY_RE`, `STREET_RE`.
- Produces: `pub fn extract_address(text: &str) -> (Option<String>, Option<String>, Option<String>)` (unchanged signature, now block-aware); internal `fn build_extracted(doc: &Document) -> Extracted`.

- [ ] **Step 1: Write the failing integration test**

Create `tests/foundation.rs`:

```rust
use german_impressum_extractor::{extract_address, extract_all};

#[test]
fn address_picks_the_block_where_street_and_postcode_coexist() {
    // Naive first-match would take the street from the first block and the
    // postcode from the second, producing a mixed, wrong address.
    let text = "\
Kontaktbüro
Musterweg 5

Hauptsitz
Beispielstraße 12
10115 Berlin";
    let (pc, city, street) = extract_address(text);
    assert_eq!(pc.as_deref(), Some("10115"));
    assert_eq!(city.as_deref(), Some("Berlin"));
    assert_eq!(street.as_deref(), Some("Beispielstraße 12"));
}

#[test]
fn single_block_address_unchanged() {
    let (pc, city, street) = extract_address("Hauptstraße 12, 10115 Berlin");
    assert_eq!(pc.as_deref(), Some("10115"));
    assert_eq!(city.as_deref(), Some("Berlin"));
    assert_eq!(street.as_deref(), Some("Hauptstraße 12"));
}

#[test]
fn extract_all_still_works_end_to_end() {
    let d = extract_all("Muster GmbH\nHauptstraße 12, 10115 Berlin\nUSt-IdNr.: DE123456789");
    assert_eq!(d.postcode.as_deref(), Some("10115"));
    assert_eq!(d.vat_id.as_deref(), Some("DE123456789"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test foundation address_picks 2>&1 | tail -20`
Expected: FAIL — the first test's `street` is `Some("Musterweg 5")` (cross-block mixing).

- [ ] **Step 3: Add `address_from_document` and refactor `extract_address`**

In `src/lib.rs`, replace the existing `extract_address` function body so it routes through a `Document`, and add the block-aware helper. Replace:

```rust
pub fn extract_address(text: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut postcode = None;
    let mut city = None;
    let mut street = None;
    if let Some(cap) = GERMAN_POSTCODE_AND_CITY_RE.captures(text) {
        postcode = cap.get(1).map(|m| m.as_str().to_string());
        city = cap.get(2).map(|m| m.as_str().trim().to_string());
    }
    if let Some(cap) = STREET_RE.captures(text) {
        street = Some(format!(
            "{} {}",
            cap.get(1).map(|m| m.as_str().trim()).unwrap_or(""),
            cap.get(2).map(|m| m.as_str().trim()).unwrap_or("")
        ));
    }
    (postcode, city, street)
}
```

with:

```rust
/// Extract the first German address (`(postcode, city, street)`) from text.
///
/// Postcode/city and street are drawn from the *same* block when possible, so
/// pages listing multiple addresses do not mix parts across entities.
pub fn extract_address(text: &str) -> (Option<String>, Option<String>, Option<String>) {
    let doc = segment::Document::parse(normalize::normalize_text(text));
    address_from_document(&doc)
}

fn parse_postcode_city(block: &str) -> Option<(String, String)> {
    let cap = GERMAN_POSTCODE_AND_CITY_RE.captures(block)?;
    Some((
        cap.get(1)?.as_str().to_string(),
        cap.get(2)?.as_str().trim().to_string(),
    ))
}

fn parse_street(block: &str) -> Option<String> {
    let cap = STREET_RE.captures(block)?;
    Some(format!(
        "{} {}",
        cap.get(1).map(|m| m.as_str().trim()).unwrap_or(""),
        cap.get(2).map(|m| m.as_str().trim()).unwrap_or("")
    ))
}

fn address_from_document(
    doc: &segment::Document,
) -> (Option<String>, Option<String>, Option<String>) {
    use candidate::Candidate;
    use segment::LabelKind;

    let mut pc_cands: Vec<Candidate<(String, String)>> = Vec::new();
    let mut street_cands: Vec<Candidate<String>> = Vec::new();

    for (idx, block) in doc.block_texts().enumerate() {
        let pc = parse_postcode_city(block);
        let st = parse_street(block);
        // Same-block hit: strongest signal, return immediately.
        if let (Some((code, city)), Some(street)) = (&pc, &st) {
            return (Some(code.clone()), Some(city.clone()), Some(street.clone()));
        }
        if let Some(pcv) = pc {
            pc_cands.push(Candidate::new(pcv, 0..0, idx, Some(LabelKind::Postal)));
        }
        if let Some(sv) = st {
            street_cands.push(Candidate::new(sv, 0..0, idx, Some(LabelKind::Postal)));
        }
    }

    // Fallback: first postcode/city and first street seen anywhere.
    let (postcode, city) = match pc_cands.into_iter().next() {
        Some(c) => (Some(c.value.0), Some(c.value.1)),
        None => (None, None),
    };
    let street = street_cands.into_iter().next().map(|c| c.value);
    (postcode, city, street)
}
```

- [ ] **Step 4: Route `extract_all` through a shared `Document`**

In `src/lib.rs`, change the top of `extract_all` so it builds one `Document` and delegates to `build_extracted`. Replace the existing `extract_all` function with:

```rust
pub fn extract_all(text: &str) -> Extracted {
    let doc = segment::Document::parse(normalize::normalize_text(text));
    build_extracted(&doc)
}

fn build_extracted(doc: &segment::Document) -> Extracted {
    let text = doc.text();
    let fax = extract_fax(text);
    let iban_spans: Vec<(usize, usize)> = IBAN_DE_RE
        .find_iter(text)
        .map(|m| (m.start(), m.end()))
        .collect();
    let phones = collect_phones(text, &iban_spans)
        .into_iter()
        .filter(|p| fax.as_deref() != Some(p.as_str()))
        .collect();

    let (postcode, city, street) = address_from_document(doc);

    let hr_number = extract_hr_number(text);
    let hr_court = extract_hr_court(text);
    let tax_number = extract_tax_number(text);

    Extracted {
        emails: extract_emails(text),
        phones,
        fax,
        postcode,
        city,
        street,
        hr_number,
        hr_court,
        vat_id: extract_vat_id(text),
        tax_number,
        iban: extract_iban(text),
        bic: extract_bic(text),
        legal_form: extract_legal_form(text),
        year_founded: extract_year_founded(text),
        persons: extract_persons(text),
    }
}
```

(Keep the surrounding doc comment on `extract_all`. Remove any now-duplicated address logic that previously lived inline in `extract_all`.)

- [ ] **Step 5: Run the new and existing tests**

Run: `cargo test --all-targets 2>&1 | tail -8`
Expected: PASS — new `tests/foundation.rs` (3 tests) and all existing tests green.

- [ ] **Step 6: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3`
Expected: no warnings. (If Task 3 needed `#[allow(dead_code)]` on `Candidate`, remove it now — the fields are used here.)

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs tests/foundation.rs
git commit -m "feat(address): block-aware address via Document; wire extract_all through normalize"
```

---

## Task 5: HTML support (`html.rs`, feature-gated)

**Files:**
- Modify: `Cargo.toml` (add optional `html5gum` dep + `html` feature)
- Create: `src/html.rs`
- Modify: `src/lib.rs` (add `#[cfg(feature = "html")] mod html;`, `extract_all_html`, `html_to_impressum_text`)
- Test: unit tests in `src/html.rs` + a gated test in `tests/foundation.rs`

**Interfaces:**
- Consumes: `normalize::normalize_text`, `segment::Document`, `build_extracted`.
- Produces:
  - `#[cfg(feature = "html")] pub fn html_to_impressum_text(html: &str) -> String`
  - `#[cfg(feature = "html")] pub fn extract_all_html(html: &str) -> Extracted`
  - internal `pub(crate) trait HtmlFlattener { fn flatten(&self, html: &str) -> String; }` and `pub(crate) struct DefaultFlattener`.

- [ ] **Step 1: Add the dependency and feature**

In `Cargo.toml`, under `[dependencies]` add:

```toml
html5gum = { version = "0.8", default-features = false, optional = true }
```

Under `[features]` add:

```toml
html = ["dep:html5gum"]
```

Run: `cargo build --features html 2>&1 | tail -3`
Expected: builds (html5gum pulls no transitive deps with `default-features = false`).

- [ ] **Step 2: Write the failing tests**

Create `src/html.rs`:

```rust
//! HTML → canonical structured text, behind the `html` feature. Uses the
//! `html5gum` tokenizer (which also decodes character references). Block-level
//! elements and `<br>` become `\n`; `<dt>`/`<dd>` and table cells become
//! `label\tvalue`; `<script>`/`<style>`/`<head>`/`<noscript>` content is dropped.

use html5gum::{HtmlString, Token, Tokenizer};

use crate::normalize::normalize_text;

/// Flattens raw HTML into the canonical `\n`/`\t` string.
pub(crate) trait HtmlFlattener {
    fn flatten(&self, html: &str) -> String;
}

pub(crate) struct DefaultFlattener;

fn name_is(name: &HtmlString, want: &[u8]) -> bool {
    name.eq_ignore_ascii_case(want)
}

fn is_block(name: &HtmlString) -> bool {
    const BLOCK: &[&[u8]] = &[
        b"p", b"div", b"section", b"article", b"header", b"footer", b"main",
        b"aside", b"nav", b"ul", b"ol", b"li", b"table", b"tr", b"dl",
        b"blockquote", b"address", b"h1", b"h2", b"h3", b"h4", b"h5", b"h6", b"br",
    ];
    BLOCK.iter().any(|b| name_is(name, b))
}

fn is_raw(name: &HtmlString) -> bool {
    name_is(name, b"script")
        || name_is(name, b"style")
        || name_is(name, b"head")
        || name_is(name, b"noscript")
        || name_is(name, b"template")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(html: &str) -> String {
        normalize_text(&DefaultFlattener.flatten(html))
    }

    #[test]
    fn dl_becomes_label_tab_value() {
        assert_eq!(flat("<dl><dt>Telefon</dt><dd>030 123</dd></dl>"), "Telefon\t030 123");
    }

    #[test]
    fn table_row_becomes_label_tab_value() {
        assert_eq!(
            flat("<table><tr><th>PLZ</th><td>10115</td></tr></table>"),
            "PLZ\t10115"
        );
    }

    #[test]
    fn script_and_style_content_dropped() {
        assert_eq!(flat("<p>Hallo</p><script>var x=1;</script><style>a{}</style>"), "Hallo");
    }

    #[test]
    fn br_and_blocks_become_newlines_and_entities_decode() {
        assert_eq!(flat("Meyer&amp;Co<br>10115 Berlin"), "Meyer&Co\n10115 Berlin");
    }

    #[test]
    fn broken_markup_does_not_panic() {
        let _ = flat("<div><span>unclosed <b> &notanentity <<< ");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --features html --lib html 2>&1 | head -20`
Expected: FAIL — `flatten` not implemented (trait method has no body / DefaultFlattener has no impl).

- [ ] **Step 4: Implement `flatten`**

Add to `src/html.rs` (above the `#[cfg(test)]` module):

```rust
impl HtmlFlattener for DefaultFlattener {
    fn flatten(&self, html: &str) -> String {
        let mut out = String::with_capacity(html.len());
        let mut raw_depth: u32 = 0;

        for token in Tokenizer::new(html).flatten() {
            match token {
                Token::StartTag(tag) => {
                    if is_raw(&tag.name) {
                        raw_depth += 1;
                        continue;
                    }
                    if is_block(&tag.name) {
                        out.push('\n');
                    }
                }
                Token::EndTag(tag) => {
                    if is_raw(&tag.name) {
                        raw_depth = raw_depth.saturating_sub(1);
                        continue;
                    }
                    // Label/value separators.
                    if name_is(&tag.name, b"dt") || name_is(&tag.name, b"th")
                        || name_is(&tag.name, b"td")
                    {
                        out.push('\t');
                    } else if is_block(&tag.name) {
                        out.push('\n');
                    }
                }
                Token::String(s) => {
                    if raw_depth == 0 {
                        out.push_str(&String::from_utf8_lossy(&s));
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// Convert an HTML document into the canonical Impressum text (flatten + normalize).
pub fn html_to_impressum_text(html: &str) -> String {
    normalize_text(&DefaultFlattener.flatten(html))
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --features html --lib html 2>&1 | tail -20`
Expected: PASS (5 tests).

Note on `br`: `is_block` returns true for `br`, and the flattener pushes `\n` on its start tag; the end tag (if the tokenizer emits one for the void element) would push another `\n`, collapsed by `normalize_text`. If the `table_row` test shows a stray leading/trailing tab or newline, it is removed by `normalize_text`'s line trimming — the assertions above already expect the normalized result.

- [ ] **Step 6: Wire the public HTML entry points**

In `src/lib.rs`, next to the other `mod` declarations add:

```rust
#[cfg(feature = "html")]
mod html;
```

Add these public functions near `extract_all` (after `build_extracted`):

```rust
/// Extract every supported field from an HTML Impressum page.
///
/// Available with the `html` feature. Equivalent to running [`extract_all`] on
/// [`html_to_impressum_text`].
#[cfg(feature = "html")]
pub fn extract_all_html(html: &str) -> Extracted {
    let doc = segment::Document::parse(html::html_to_impressum_text(html));
    build_extracted(&doc)
}

/// Flatten an HTML document to the crate's canonical Impressum text.
///
/// Available with the `html` feature.
#[cfg(feature = "html")]
pub use html::html_to_impressum_text;
```

- [ ] **Step 7: Add a feature-gated equivalence integration test**

Append to `tests/foundation.rs`:

```rust
#[cfg(feature = "html")]
#[test]
fn html_extraction_matches_text_equivalent() {
    use german_impressum_extractor::extract_all_html;
    let html = "\
<h1>Muster GmbH</h1>
<p>Hauptstra&szlig;e 12, 10115 Berlin</p>
<dl><dt>USt-IdNr.</dt><dd>DE123456789</dd></dl>";
    let d = extract_all_html(html);
    assert_eq!(d.legal_form.as_deref(), Some("GmbH"));
    assert_eq!(d.postcode.as_deref(), Some("10115"));
    assert_eq!(d.city.as_deref(), Some("Berlin"));
    assert_eq!(d.street.as_deref(), Some("Hauptstraße 12"));
    assert_eq!(d.vat_id.as_deref(), Some("DE123456789"));
}
```

- [ ] **Step 8: Run everything (both feature states) + fmt/clippy**

Run: `cargo test --all-targets 2>&1 | tail -4`
Expected: PASS (html functions absent, still compiles).
Run: `cargo test --all-targets --features html 2>&1 | tail -6`
Expected: PASS including `html_extraction_matches_text_equivalent`.
Run: `cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3`
Expected: no warnings.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml src/html.rs src/lib.rs tests/foundation.rs
git commit -m "feat(html): feature-gated HTML support via html5gum flattener"
```

---

## Task 6: CI matrix + docs

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`
- Modify: `CHANGELOG.md`

**Interfaces:** none (project meta).

- [ ] **Step 1: Add the `html` feature to CI**

In `.github/workflows/ci.yml`, in the `test` job, after the `Test (serde feature)` step, add:

```yaml
      - name: Test (html feature)
        run: cargo test --all-targets --features html
```

The existing `Doc tests` step already runs `--all-features` (covers `html`), and the `msrv` job already runs `cargo test --all-features` (verifies `html` on 1.85).

- [ ] **Step 2: Update the README HTML positioning**

In `README.md`, under the `## Usage` section, replace the "Pipeline pattern" note that says *"Bring your own HTML client / HTML cleaner"* with an HTML-support subsection:

```markdown
### From HTML directly (optional `html` feature)

```toml
[dependencies]
german-impressum-extractor = { version = "0.1", features = ["html"] }
```

```rust
use german_impressum_extractor::{extract_all_html, html_to_impressum_text};

let data = extract_all_html(html_page);            // parse + extract in one step
let text = html_to_impressum_text(html_page);      // just the cleaned text
```

Without the feature, keep feeding plain text to `extract_all` — the default
build pulls no HTML dependencies.
```

- [ ] **Step 3: Update the CHANGELOG**

In `CHANGELOG.md`, under `## [Unreleased]` → `### Added`, add:

```markdown
- Text normalization layer (Unicode NFC, invisible-char / whitespace cleanup,
  well-formed HTML entity decoding) applied to all input before extraction.
- Block-aware address extraction: postcode/city and street are taken from the
  same text block, preventing cross-entity mixing on multi-address pages.
- Optional `html` feature: `extract_all_html` and `html_to_impressum_text`
  parse raw HTML (via `html5gum`) into structured data. Default build unchanged.
```

- [ ] **Step 4: Verify formatting and full build**

Run: `cargo build --all-features 2>&1 | tail -2`
Expected: builds.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml README.md CHANGELOG.md
git commit -m "ci+docs: test html feature; document HTML support"
```

---

## Self-Review

**1. Spec coverage:**
- Canonical `\n`/`\t` format → Task 1 (normalize), Task 5 (flattener emits it). ✓
- Text normalization steps (NFC, invisibles, NBSP, CRLF, entities, collapse) → Task 1. ✓
- HTML path behind `html` feature, parser behind trait, `dt/dd`+table → `label\tvalue`, script/style dropped → Task 5. ✓
- `Document`/`Block`/`Segment` + `LabelKind` + label detection (tab and `Label:`) → Task 2. ✓
- Internal `Candidate<T>` with provenance → Task 3, exercised in Task 4. ✓
- Same-block address demonstrator + fallback → Task 4. ✓
- Additive entry points `extract_all_html`, `html_to_impressum_text` → Task 5. ✓
- Non-breaking (existing tests green) → verified in Tasks 4, 5. ✓
- Error handling: infallible, no `unwrap` on parser (`Tokenizer::new(..).flatten()` skips errors), `from_utf8_lossy` → Task 5; char-boundary-safe spans (segment uses byte offsets from `split_inclusive`, no mid-codepoint slicing) → Task 2. ✓
- CI matrix + MSRV → Task 6 (+ existing msrv job). ✓
- Success criteria 1–5 → covered by Tasks 4 (non-breaking + address), 5 (HTML equivalence + no new default deps + 1.85), and the adversarial no-panic tests in Tasks 1/5. ✓

**2. Placeholder scan:** No TBD/TODO; every code step contains complete code. ✓

**3. Type consistency:** `normalize_text(&str)->String`, `Document::parse(String)->Document`, `Document::text(&self)->&str`, `Document::block_texts(&self)->impl Iterator<Item=&str>`, `LabelKind` variants, `Candidate::new(value, span, block, label)`, `address_from_document(&Document)->(Option,Option,Option)`, `build_extracted(&Document)->Extracted`, `html_to_impressum_text(&str)->String`, `extract_all_html(&str)->Extracted` — used consistently across tasks. ✓

**Note for the executor:** `html5gum` API specifics (`Token`, `HtmlString`, `Tokenizer::new(..).flatten()`, `HtmlString: Deref<[u8]>` enabling `eq_ignore_ascii_case` and `String::from_utf8_lossy`) were verified against `html5gum 0.8.4`. If a point release changes them, consult `cargo doc -p html5gum --open` or context7 and adjust the token match arms only.
