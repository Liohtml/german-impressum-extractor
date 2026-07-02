<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/banner-dark.svg">
    <img alt="german-impressum-extractor — structured data from German B2B Impressum text, pure Rust, no async" src="assets/banner-light.svg" width="100%">
  </picture>
</p>

<p align="center">
  <a href="https://crates.io/crates/german-impressum-extractor"><img alt="crates.io" src="https://img.shields.io/crates/v/german-impressum-extractor.svg?logo=rust"></a>
  <a href="https://docs.rs/german-impressum-extractor"><img alt="docs.rs" src="https://img.shields.io/docsrs/german-impressum-extractor?logo=docs.rs"></a>
  <a href="https://github.com/Liohtml/german-impressum-extractor/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Liohtml/german-impressum-extractor/actions/workflows/ci.yml/badge.svg"></a>
  <a href="Cargo.toml"><img alt="MSRV 1.85" src="https://img.shields.io/badge/MSRV-1.85-b7410e.svg?logo=rust"></a>
  <a href="#license"><img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg"></a>
  <a href="https://github.com/rust-secure-code/safety-dance/"><img alt="unsafe forbidden" src="https://img.shields.io/badge/unsafe-forbidden-success.svg"></a>
</p>

<p align="center"><b>Turn a German website's <a href="https://www.gesetze-im-internet.de/tmg/__5.html">Impressum</a> — messy text or raw HTML — into a clean, typed struct.</b></p>

<p align="center">
  📥 text <i>or</i> HTML in &nbsp;·&nbsp; 🎯 20+ typed fields out &nbsp;·&nbsp; 📊 confidence scores &nbsp;·&nbsp; 🦀 pure Rust, no async
</p>

<p align="center">
  <a href="#-quick-start">Quick start</a> ·
  <a href="#-what-it-extracts">Fields</a> ·
  <a href="#-from-html">HTML</a> ·
  <a href="#-confidence-scores">Confidence</a> ·
  <a href="#-multiple-addresses">Multi-address</a> ·
  <a href="#feature-flags">Features</a> ·
  <a href="#-robustness--limits">Limits</a>
</p>

---

Every German commercial website must publish an **Impressum** (TMG §5) — the legal entity, managing directors, address, contact details, and tax identifiers. That data is gold for B2B research, lead enrichment, and compliance tooling, but it arrives as free-form prose. This crate parses it into a struct so you don't have to babysit a pile of regexes.

### ✨ Highlights

- **📥 Text *or* HTML** — feed plain text, or enable the `html` feature and pass a raw page; entities, tags, `<dt>/<dd>` label pairs and tables are handled.
- **🎯 20+ fields, one call** — `extract_all` returns contacts, address, legal form, register, VAT/tax, bank details, people (with roles), and EU-compliance fields.
- **📊 Confidence scores** — `extract_all_scored` annotates each field `0.0..=1.0` using real validators (IBAN **mod-97**, postcode range, VAT/BIC shape) plus a label boost.
- **🏢 Multi-location aware** — `extract_addresses` returns *every* address, never mixing components across blocks.
- **🧹 Noise-resistant** — Unicode/whitespace/entity normalization, TLD allowlist for emails, char-boundary-safe (no panics on `ä`/emoji/zero-width chars).
- **🦀 Lean & safe** — synchronous, `#![forbid(unsafe_code)]`, **MSRV 1.85**, and a default build with no HTTP/HTML/async dependencies. 100+ tests, `clippy -D warnings` in CI.

## 🚀 Quick start

```toml
[dependencies]
german-impressum-extractor = "0.1"
```

```rust
use german_impressum_extractor::extract_all;

let impressum = "
    Musterreinigung GmbH & Co. KG
    Geschäftsführer: Dr. Hans Müller
    Hauptstraße 12, 10115 Berlin
    Tel: +49 30 1234567
    E-Mail: info@musterreinigung.de
    Amtsgericht Berlin HRB 12345 B
    USt-IdNr.: DE 123 456 789
    IBAN: DE89 3704 0044 0532 0130 00
    Gegründet 1985
";

let data = extract_all(impressum);

assert_eq!(data.legal_form.as_deref(),    Some("GmbH & Co. KG"));
assert_eq!(data.postcode.as_deref(),      Some("10115"));
assert_eq!(data.vat_id.as_deref(),        Some("DE123456789"));
assert_eq!(data.iban.as_deref(),          Some("DE89370400440532013000"));
assert_eq!(data.register_type.as_deref(), Some("HRB"));
assert_eq!(data.year_founded,             Some(1985));
assert_eq!(data.emails,                   vec!["info@musterreinigung.de"]);
assert!(data.persons.iter().any(|p| p.last_name.as_deref() == Some("Müller")));
```

Got a raw HTML page instead of clean text? Enable the [`html`](#-from-html) feature and call `extract_all_html` — same result, one step.

## 📋 What it extracts

`extract_all` returns an [`Extracted`](https://docs.rs/german-impressum-extractor/latest/german_impressum_extractor/struct.Extracted.html) struct with these fields:

| Group | Fields |
|-------|--------|
| 📇 **Contact** | `emails` (plain **and** obfuscated `info [at] firma [dot] de`), `phones` (normalized to `+49…`), `fax`, `de_mail` |
| 🏠 **Address** | `postcode`, `city`, `street` — first match, or [`extract_addresses`](#-multiple-addresses) for all |
| 🏢 **Legal & register** | `legal_form` (`GmbH`, `GmbH & Co. KG`, `GmbH & Co. KGaA`, `KGaA`, `UG`, `AG`, `KG`, `OHG`, `GbR`, `e.K.`, `eG`, `SE`), `hr_number` (`HRB 12345 B`), `hr_court`, `register_type` (`HRA`/`HRB`), `year_founded` |
| 💶 **Tax & banking** | `vat_id` (USt-IdNr., incl. grouped `DE 123 456 789`), `tax_number` (Steuernummer, incl. `St.-Nr.`/`StNr.`), `iban`, `bic` |
| 👥 **People** | `persons` — Geschäftsführer / Inhaber / Vorstand / Verantwortlicher (§18 MStV), each with a role tag and best-effort first/last name |
| 🛡️ **Compliance** | `supervisory_authority` (Aufsichtsbehörde), `professional_chamber` (zuständige Kammer / Berufskammer), `dispute_resolution_url` (EU ODR / OS-Plattform) |

## 🌐 From HTML

<a id="-from-html"></a>

```toml
german-impressum-extractor = { version = "0.1", features = ["html"] }
```

```rust
use german_impressum_extractor::{extract_all_html, html_to_impressum_text};

let data = extract_all_html(html_page);        // parse + extract in one step
let text = html_to_impressum_text(html_page);  // just the cleaned, structured text
```

The flattener (powered by [`html5gum`](https://crates.io/crates/html5gum)) drops `<script>`/`<style>`, turns block elements and `<br>` into line breaks, and maps `<dt>/<dd>` pairs and table cells into `label → value` lines — so definition-list and table Impressums parse correctly. Without the feature, the default build pulls **no** HTML dependency.

## 📊 Confidence scores

Need to know how much to trust each field? `extract_all_scored` returns the same data, each value wrapped in a `Scored { value, confidence }` where `confidence` is a heuristic in `0.0..=1.0` — driven by format validators (a real IBAN **mod-97** check, postcode range, VAT/BIC shape) plus a boost when the page explicitly labels the field.

```rust
use german_impressum_extractor::extract_all_scored;

let scored = extract_all_scored(impressum_text);

if let Some(iban) = scored.iban {
    println!("{} — confidence {:.2}", iban.value, iban.confidence); // e.g. 0.95
}
```

`extract_all` stays untouched; scoring is purely additive. With the `html` feature, `extract_all_scored_html` does the same from raw HTML.

## 🏢 Multiple addresses

`extract_address` returns the first address. For pages listing several locations, `extract_addresses` returns one `Address` per address block — components are **never** mixed across blocks:

```rust
use german_impressum_extractor::extract_addresses;

for a in extract_addresses(impressum_text) {
    println!("{:?}, {:?} {:?}", a.street, a.postcode, a.city);
}
```

## 🔧 Granular extractors

Only need one field? Every field has a standalone `extract_*` function (all normalize their input exactly like `extract_all`, so results match):

<details>
<summary><b>Show all granular functions</b></summary>

```rust
use german_impressum_extractor::{
    // contact
    extract_emails, extract_phones, extract_fax, extract_de_mail,
    // address (single + all)
    extract_address, extract_addresses,
    // legal & register
    extract_legal_form, extract_hr_number, extract_hr_court, extract_register_type,
    extract_year_founded,
    // tax & banking
    extract_vat_id, extract_tax_number, extract_iban, extract_bic,
    // people
    extract_persons,
    // compliance
    extract_supervisory_authority, extract_professional_chamber,
    extract_dispute_resolution_url,
};

let text = "Geschäftsführer: Hans Müller, Tel: +49 30 1234567";
let phones  = extract_phones(text);          // ["+493012345 67".replace(' ', "")]
let persons = extract_persons(text);         // [Person { last_name: "Müller", role: "Geschäftsführer", .. }]
let (postcode, city, street) = extract_address(text);
```

</details>

## Feature flags

| Feature | Adds | Extra dependencies |
|---------|------|--------------------|
| *(default)* | text extraction, all fields, confidence scores | `regex`, `unicode-normalization` |
| `html` | `extract_all_html`, `extract_all_scored_html`, `html_to_impressum_text` | `html5gum` |
| `serde` | `Serialize` + `Deserialize` on `Extracted`, `Person`, `Address`, `Scored<T>`, `ScoredExtracted` | `serde` |

```toml
german-impressum-extractor = { version = "0.1", features = ["html", "serde"] }
```

## 🧠 Why not just roll your own regex?

Because real German B2B pages have hundreds of edge cases:

- **Phones:** `+49 (0) 30 / 1234 5-67` vs `0049 30 12345-678` vs `030 / 12345/678` — all one number.
- **Names:** `Dr. h.c. Hans-Peter von der Mühle und Anna Schmidt-Lutz` → two distinct people, titles stripped, "von der" handled.
- **Postcodes** vs random 5-digit numbers; a street and its "PLZ Stadt" line that belong together vs. two different branch addresses.
- **Legal forms with `&`:** `GmbH & Co. KG` vs `GmbH & Co. KGaA` vs plain `GmbH`.
- **Unicode gremlins:** zero-width spaces, soft hyphens, non-breaking spaces, HTML entities, `ae/oe/ue/ss` — normalized away before matching, on char boundaries (no panics).

This crate ships an extensive unit + regression + integration suite (**100+ tests**, green on stable **and** the 1.85 MSRV), enforces `clippy -D warnings` in CI, and is built for production B2B research pipelines.

## ✅ Robustness & limits

- ✅ Handles `ä ö ü ß`, `ae/oe/ue/ss` substitutions, obfuscated emails, and label/definition-list/table HTML layouts.
- ✅ Infallible and panic-free — every extractor returns `Option`/`Vec`, never panics, even on adversarial multi-byte input.
- ⚠️ Extraction is heuristic/regex-based; unusual layouts can still produce noise. Use `extract_all_scored` to rank field trust, and treat low-confidence `persons`/`professional_chamber` results as best-effort.
- ⚠️ `extract_address` returns only the first address — use `extract_addresses` for multi-location pages.
- ⚠️ This crate does the **extraction** step only. Bring your own HTTP client / HTML fetcher; pass the page (or its text) here.

## 🤝 Contributing

Issues and PRs welcome. Please make sure:

1. `cargo test --all-features` passes,
2. new patterns ship with at least one test derived from real-world text,
3. `cargo fmt` and `cargo clippy --all-targets --all-features -- -D warnings` are clean.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
