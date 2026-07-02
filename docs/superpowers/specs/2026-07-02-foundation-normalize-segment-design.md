# Foundation: Normalization + Segmentation + Candidate substrate

**Status:** Approved design (2026-07-02)
**Sub-project:** TP1 of a 5-part noise-reduction program (see "Program context").
**Author:** brainstorming session

> Note: this spec is written in English to match the codebase (code, doc comments,
> README, CHANGELOG are all English). Discussion happened in German.

## Program context

The crate currently scans the whole input with flat regexes, which is the root
cause of nearly all output noise (wrong/mixed addresses, junk person names,
prose matches for legal form/year, etc.). The agreed remedy is a small program,
sequenced so each part is its own spec → plan → implementation:

| # | Sub-project | Breaks API? |
|---|-------------|-------------|
| **1** | **Foundation: normalization + segmentation + candidate substrate** (this spec) | No |
| 2 | Confidence scoring + `extract_all_scored()` | No (additive) |
| 3 | Per-field precision hardening + validation (IBAN mod-97, USt checksum, `persons` name heuristics, …) | No |
| 4 | Multi-entity / multiple values | Additive |
| 5 | New fields (Aufsichtsbehörde, Berufskammer, OS-Plattform, DE-Mail, website, register type) | Additive |

Decisions carried into this design:
- **Precision model:** best-guess + confidence score (scoring itself lands in TP2).
- **API integration:** additive / non-breaking. `extract_all(text)` is unchanged.
- **HTML:** full HTML support is desired, delivered **feature-gated** (Approach C).
- **Dependencies:** a lightweight, MSRV-1.85-compatible HTML parser dep is allowed,
  but only behind the `html` feature; the default build gains **no** new deps.

## Scope of TP1

**In scope:**
- A normalization layer that turns text **or** HTML into one canonical string.
- A segmentation layer that splits that string into labeled blocks/segments.
- An internal `Candidate<T>` substrate carrying provenance (span, block, label).
- **One** structural noise fix as a demonstrator: **address from a single block**
  (stop mixing postcode/city/street across unrelated blocks).
- New public entry points: `extract_all_html`, `html_to_impressum_text`.

**Explicitly out of scope (later sub-projects):**
- Confidence scoring and public `Candidate`/`Scored<T>` types (TP2).
- Deep per-field precision (person-name heuristics, IBAN/USt checksums) (TP3).
- Multi-entity output (TP4). New fields (TP5).

## Architecture

Pipeline inserted **before** the existing extractors:

```
input ──► normalize ──► segment ──► [Document: blocks + Candidate substrate] ──► extractors
(text or HTML)   (canonical text)   (labeled blocks/segments w/ provenance)   (existing, fed clean input)
```

Module layout (de-tangles the current 908-line `lib.rs`):

- `src/lib.rs` — public API + re-exports (entry point).
- `src/normalize.rs` — text cleanup; HTML part behind `#[cfg(feature = "html")]`.
- `src/html.rs` — only with `html` feature: parser adapter behind an internal trait.
- `src/segment.rs` — segmentation → `Document`/`Block`/`Segment`.
- `src/candidate.rs` — internal `Candidate<T>` + provenance.
- Existing extractor functions stay where they are; TP1 does not rewrite them.

New public entry points (all additive, non-breaking):

```rust
pub fn extract_all(text: &str) -> Extracted;             // unchanged signature; now runs via normalize
#[cfg(feature = "html")]
pub fn extract_all_html(html: &str) -> Extracted;        // new
#[cfg(feature = "html")]
pub fn html_to_impressum_text(html: &str) -> String;     // new; flatten + normalize only
```

## Component 1 — Normalization (`normalize.rs` + `html.rs`)

**Canonical intermediate format.** Both input paths converge on one string with
two conventions the segmenter reads uniformly:

- `\n` = block/line boundary
- `\t` = label→value separator within a line

This gives the segmenter a **single** code path regardless of input type.

**Text path (`normalize_text`, always compiled), ordered:**

1. Unicode **NFC** once, centrally (replaces the ad-hoc NFC in `clean_phone`).
2. Remove invisible/control chars: zero-width (U+200B/200C/200D), BOM (U+FEFF),
   **soft hyphen** (U+00AD), word joiner (U+2060).
3. Normalize whitespace variants to a regular space: **NBSP** (U+00A0), narrow
   NBSP (U+202F), other Unicode spaces; literal tabs → space (the intentional
   `\t` separators are inserted by the HTML flattener, not present in raw text).
4. Line endings CRLF/CR → `\n`.
5. Decode **well-formed** HTML entities only: named (`&amp;`, `&uuml;`, …),
   decimal (`&#252;`), hex (`&#xFC;`). Malformed/ambiguous `&` sequences are left
   as-is (real prose almost never contains a well-formed `&name;`). Fixes the
   `&amp;` leak even for text input.
6. Collapse runs of spaces; right-trim each line. **Newlines are preserved** (they
   are the block signal). Collapse 3+ consecutive newlines to exactly 2 (one blank
   line = block boundary).

Dashes / en-dash / em-dash are left untouched (needed for address & house-number
ranges).

**HTML path (`html.rs`, `#[cfg(feature = "html")]`):**

- Parsing goes through an internal trait so the concrete crate is swappable:
  ```rust
  pub(crate) trait HtmlFlattener {
      /// Returns structured text using the canonical `\n` / `\t` conventions.
      fn flatten(&self, html: &str) -> String;
  }
  ```
- Default implementation wraps the chosen parser (candidate crates: `tl`,
  `html5gum`, `scraper` — final choice made during planning against **MSRV ≤ 1.85**
  and dependency weight; prefer the lightest adequate option).
- Flattening rules:
  - Drop `<script>`, `<style>`, `<head>`, `<noscript>`, and comments entirely.
  - Block-level elements and `<br>` → `\n`.
  - `<dt>`/`<dd>` pairs and table cells (`<th>`/`<td>` within a `<tr>`) →
    `label\tvalue` on one line (the semantic gift for segmentation).
  - Decode entities via the shared decoder.
- Output is then passed through `normalize_text`, so both paths converge.
- `html_to_impressum_text(html)` = `DefaultFlattener.flatten(html)` then `normalize_text`.

**Provenance note:** `Candidate` spans point into the **normalized** string (what
the extractors actually see). No mapping back to raw HTML in TP1.

## Component 2 — Segmentation (`segment.rs`)

Owning `Document`; segments reference it by byte span (no lifetime gymnastics):

```rust
pub struct Document { text: String, blocks: Vec<Block> }   // text = normalized string
struct Block   { span: Range<usize>, segments: Vec<Segment> }
struct Segment { span: Range<usize>, label: Option<LabelKind>, value_span: Range<usize> }
```

- **Block** = consecutive lines separated by a blank line (≥1 empty line). The unit
  of locality (address from one block).
- **Segment** = one logical line. If it has a label→value shape, `label` is set and
  `value_span` covers only the value part (else `value_span == span`).

**Label detection**, two sources, one output:

1. Segment contains `\t` (from HTML `dt/dd`/tables) → left = label text, right = value.
2. Otherwise a leading `Label:` pattern in the text (`Telefon:`, `E-Mail:`,
   `Geschäftsführer:`, `USt-IdNr.:`, `Registergericht:`, `HRB:`, …) via a label
   lexicon (a `LazyLock` table) → `LabelKind`.

```rust
enum LabelKind { Phone, Fax, Email, Postal, Managers, VatId, TaxNumber,
                 Register, Court, Bank, LegalName, Founded, Web, Other }
```

`LabelKind` is intentionally broader than TP1 needs; TP2 (scoring) and TP3
(precision) reuse the same label as a strong signal without touching the model.

All span/label computation uses char-boundary-safe slicing (reuse the
`floor_char_boundary` discipline from the panic fix).

## Component 3 — Candidate substrate (`candidate.rs`)

Internal (`pub(crate)`) in TP1, so adding `confidence` in TP2 is not a breaking
change:

```rust
pub(crate) struct Candidate<T> {
    value: T,
    span: Range<usize>,        // into Document.text
    block: usize,              // block index → locality
    label: Option<LabelKind>,  // label of the source segment
    // confidence: added in TP2
}
```

The address demonstrator *produces* real `Candidate`s (a postcode candidate and a
street candidate, each tagged with its `block`), so the substrate is exercised end
to end and validated for TP2 (which will score using `span`/`block`/`label`).

## Data flow / wiring

```rust
pub fn extract_all(text: &str) -> Extracted {
    let doc = Document::parse(normalize_text(text));
    build_extracted(&doc)
}

#[cfg(feature = "html")]
pub fn extract_all_html(html: &str) -> Extracted {
    build_extracted(&Document::parse(html_to_impressum_text(html)))
}

fn build_extracted(doc: &Document) -> Extracted {
    // address: block-aware candidate path (the demonstrator)
    // everything else: existing extractor functions run on `doc.text` (normalized string)
}
```

`Document::parse(text: String) -> Document` performs segmentation. The returned
`Extracted` type is unchanged → non-breaking.

**Address demonstrator logic:** iterate `doc.blocks`; find the block where a
postcode+city and a street occur together (same block, adjacent segments allowed);
return that triple. Fall back to the current first-match behavior only if no block
contains both (preserves recall on single-line/degenerate inputs).

## Error handling

- `normalize_*`, `Document::parse`, and the flattener are **infallible and
  panic-free**. No `Result`; malformed input yields best-effort output (possibly
  empty). No `unwrap` on parser output.
- All spans come from regex matches or char-boundary-safe splits on the normalized
  string; provenance spans are always valid ranges into `doc.text`.
- `#![forbid(unsafe_code)]` stays. The HTML dep may contain `unsafe` (does not
  affect our crate) but must satisfy **MSRV ≤ 1.85**.
- With the `html` feature off, `extract_all_html` / `html_to_impressum_text` do
  not exist (`cfg`-gated) — documented in the crate docs.

## Testing strategy

- **Non-breaking contract:** all existing 40+ tests stay green unchanged (proves
  additivity). This is the primary safety net.
- **Unit — `normalize`:** table of (input → expected) covering zero-width, soft
  hyphen, NBSP/narrow-NBSP, CRLF, entity decode (named/decimal/hex), entities
  **not** decoded when malformed, and space collapse that preserves newlines /
  collapses blank-line runs to one.
- **Unit — `html`** (feature-gated fixtures): `dt/dd` and table row → `label\tvalue`;
  `<script>`/`<style>` dropped; `<br>`/block element → `\n`; nested/broken markup
  does not panic; entity handling.
- **Unit — `segment`:** blank-line block splitting; label detection from `\t` and
  from `Label:`; `value_span` correctness; `LabelKind` mapping.
- **Address demonstrator:** a multi-block fixture where the naive approach mixes
  postcode (block A) with an unrelated street (block B) and the block-aware path
  picks the correct same-block triple; a single-address fixture still yields the
  same result as today.
- **Adversarial / fuzz-light:** a handful of garbage/multibyte strings through
  `normalize`/`segment`/`Document::parse` must never panic (guards the
  char-boundary bug class).
- **Fixtures:** start with **synthetic** `tests/fixtures/` (HTML + text) with golden
  expectations. Real anonymized Impressum samples can be added later to calibrate
  TP2–TP5.
- **CI matrix:** default build, `--features html`, `--all-features`, plus the
  existing `serde` and MSRV-1.85 jobs.

## Success criteria

1. `extract_all(text)` output is unchanged for all existing tests (non-breaking).
   The address demonstrator is behavior-preserving on the existing inputs (their
   address parts already sit in a single block); the first-match fallback covers
   degenerate/single-line cases.
2. `extract_all_html(html)` produces the same `Extracted` fields from an HTML
   Impressum as `extract_all` does from its text equivalent.
3. The multi-block address fixture returns a same-block triple (no cross-mixing).
4. `normalize`/`segment` never panic on adversarial/multibyte input.
5. Default build gains no new dependencies; `html` feature builds on Rust 1.85.

## Open items for the planning step

- Final HTML parser crate selection (`tl` vs `html5gum` vs `scraper`) against
  MSRV ≤ 1.85, dependency weight, and robustness.
- Exact label lexicon contents (which German labels map to which `LabelKind`).
