# german-impressum-extractor

[![crates.io](https://img.shields.io/crates/v/german-impressum-extractor.svg)](https://crates.io/crates/german-impressum-extractor)
[![docs.rs](https://docs.rs/german-impressum-extractor/badge.svg)](https://docs.rs/german-impressum-extractor)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Extract structured data from German B2B website Impressum text — pure Rust, no async runtime needed.

Germany's [TMG §5](https://www.gesetze-im-internet.de/tmg/__5.html) requires every commercial website to publish an "Impressum" listing the legal entity, managing directors, address, contact info, and tax identifiers. This crate gives you a battle-tested parser for that data.

## What it extracts

- 📧 **Email addresses** — plain (`info@firma.de`) and obfuscated (`info [at] firma [dot] de`).
- ☎️ **Phone numbers** — normalized to `+49…` form regardless of input format.
- 🏠 **Address** — German postcode + city + street with house number.
- 🪪 **HR-Nummer** — Handelsregister number (e.g. `HRB 12345 B`).
- 🏛️ **HR court** — registration court (e.g. `Amtsgericht Berlin (Charlottenburg)`).
- 💶 **USt-IdNr.** — German VAT ID (`DE` + 9 digits).
- 🧾 **Steuernummer** — local tax number.
- 🏢 **Legal form** — `GmbH`, `GmbH & Co. KG`, `UG`, `AG`, `KG`, `OHG`, `GbR`, `e.K.`, `eG`, `SE`.
- 📅 **Year founded** — `gegründet 1973` / `seit 1985` / `founded in 1990`.
- 👥 **Persons** — Geschäftsführer / Inhaber / Vorstand / Vertretungsberechtigt with role tag.

## Why not just regex it yourself

Because German B2B websites have hundreds of edge cases:

- Phone: `+49 (0) 30 / 1234 5-67` vs `0030 12 345-678` vs `030 / 12345/678`
- Names: `Dr. h.c. Hans-Peter von der Mühle und Anna Schmidt-Lutz` should yield two distinct people.
- Postcodes vs random 5-digit numbers: `12345` is not always a postcode.
- Legal forms with `&`: `GmbH & Co. KG` vs `GmbH \\& Co. KGaA` vs just `GmbH`.

This crate has 7+ unit tests covering these cases and is used in production lead-gen pipelines.

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
german-impressum-extractor = "0.1"
```

### One-shot extract

```rust
use german_impressum_extractor::extract_all;

let text = std::fs::read_to_string("impressum.txt").unwrap();
let data = extract_all(&text);

println!("Legal form: {:?}", data.legal_form);
println!("Email:      {:?}", data.emails);
println!("Phones:     {:?}", data.phones);
println!("Persons:    {:?}", data.persons);
```

### Granular extractors

Each field has a separate function if you only need part of the picture:

```rust
use german_impressum_extractor::{
    extract_emails, extract_phones, extract_persons,
    extract_address, extract_legal_form, extract_vat_id,
    extract_hr_number, extract_year_founded,
};

let text = "Geschäftsführer: Hans Müller, Tel: +49 30 1234567";

let emails  = extract_emails(text);
let phones  = extract_phones(text);
let persons = extract_persons(text);
let (postcode, city, street) = extract_address(text);
let legal_form  = extract_legal_form(text);
let vat_id      = extract_vat_id(text);
let hr_number   = extract_hr_number(text);
let founded     = extract_year_founded(text);
```

### `serde` support (optional)

```toml
[dependencies]
german-impressum-extractor = { version = "0.1", features = ["serde"] }
```

The `Extracted` and `Person` types then derive `Serialize` + `Deserialize`.

## Pipeline pattern

This crate is for the **extraction** step only. Bring your own HTTP client / HTML cleaner. A typical pipeline:

```text
HTML page  →  visible text (e.g. via `scraper` or `html2text`)  →  extract_all()  →  Extracted struct
```

## Examples

```bash
cargo run --example basic
```

## Robustness & limits

- ✅ Handles `ä`, `ö`, `ü`, `ß`, double-letter substitutions (`ae`, `oe`, `ue`, `ss`).
- ✅ Tested on dozens of real German cleaning-industry Impressum pages in production.
- ⚠️ Extraction is regex-based; some unusual layouts may produce noise. The `persons` field in particular benefits from a downstream cleanup step that filters obvious non-names (e.g. you may want to drop `last_name == "Geschäftsführung"`).
- ⚠️ Address regex finds the *first* postcode/street in the text. If the page contains multiple legal entities or branch addresses, only the first is returned.

## Contributing

Issues and PRs welcome. Please:

1. `cargo test` passes
2. New regex patterns ship with at least one test case derived from real-world text
3. Run `cargo fmt` + `cargo clippy --all-targets`

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
