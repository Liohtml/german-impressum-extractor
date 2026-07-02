//! # german-impressum-extractor
//!
//! Extract structured data from German B2B website text — specifically the
//! **Impressum** page that German websites are legally required to expose
//! under TMG §5.
//!
//! ## Why this crate exists
//!
//! Germany requires every commercial website to publish a structured
//! "Impressum" listing the legal entity, managing directors, address,
//! contact info, and tax identifiers. This metadata is invaluable for
//! B2B research, lead enrichment, compliance checks, and CRM tooling —
//! yet existing scrapers usually leave you with raw HTML and a few hopeful
//! regexes. This crate gives you a battle-tested, well-tested parser that
//! handles the common cases (and many edge cases).
//!
//! ## Features
//!
//! - 🇩🇪 German-aware: handles `ä`, `ö`, `ü`, `ß`, common abbreviations,
//!   obfuscated emails (`info [at] firma [dot] de`), German phone formats
//!   (E.164 normalization), and address patterns.
//! - 📦 Zero async runtime needed; pure synchronous parsing.
//! - 🔍 Granular: each extractor (emails, phones, address, persons,
//!   register numbers, etc.) is separately callable.
//! - 🪪 Pulls out: HR-Nummer, HR-Court, USt-IdNr., Steuernummer, IBAN, BIC.
//! - 📠 Fax / Telefax detection, kept separate from phone numbers.
//! - 👥 Geschäftsführer / Inhaber / Vorstand / Verantwortlicher (§18 MStV)
//!   name extraction with role tag.
//! - 🏢 Legal-form detection: GmbH, GmbH & Co. KG, UG, AG, KG, OHG, GbR, e.K., eG, SE.
//! - 📅 Year-founded heuristics ("gegründet 1973" / "seit 1985" / "founded in 1990").
//!
//! ## Quick start
//!
//! ```rust
//! use german_impressum_extractor::extract_all;
//!
//! let text = "
//!     Musterreinigung GmbH & Co. KG
//!     Geschäftsführer: Dr. Hans Müller und Anna Schmidt
//!     Hauptstraße 12, 10115 Berlin
//!     Tel: +49 30 1234567
//!     E-Mail: info@musterreinigung.de
//!     Eingetragen im Handelsregister Berlin HRB 12345 B
//!     USt-IdNr.: DE 123456789
//!     Gegründet 1985
//! ";
//!
//! let data = extract_all(text);
//!
//! assert_eq!(data.emails, vec!["info@musterreinigung.de"]);
//! assert!(data.phones.contains(&"+493012345-67".replace('-', "")));
//! assert_eq!(data.legal_form.as_deref(), Some("GmbH & Co. KG"));
//! assert_eq!(data.hr_number.as_deref(), Some("HRB 12345 B"));
//! assert_eq!(data.vat_id.as_deref(), Some("DE123456789"));
//! assert_eq!(data.year_founded, Some(1985));
//! assert!(data.persons.iter().any(|p| p.last_name.as_deref() == Some("Müller")));
//! ```
//!
//! ## Granular use
//!
//! All field extractors are public if you want to call them independently:
//!
//! ```rust
//! use german_impressum_extractor::{extract_emails, extract_phones};
//!
//! let emails = extract_emails("Schreiben Sie an info [at] beispiel [dot] de");
//! assert_eq!(emails, vec!["info@beispiel.de"]);
//!
//! let phones = extract_phones("Telefon: 030/123 45 67");
//! assert_eq!(phones[0], "+49301234567");
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::BTreeSet;

use regex::Regex;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

mod normalize;
mod score;
mod scored;

#[cfg(feature = "html")]
mod html;

// `Segment.label`/`span`/`value_span` and some `LabelKind` variants (e.g.
// `LegalName`) are not read by TP1's address demonstrator — they are the
// scoring substrate that TP2 consumes. Interim allow keeps the
// `-D warnings` clippy gate green until then.
#[allow(dead_code)]
mod segment;

// `Candidate.span`/`block`/`label` are not read by TP1 (only `.value` is);
// they are the provenance substrate that TP2's confidence scoring consumes.
// Interim allow keeps the -D warnings clippy gate green until then.
#[allow(dead_code)]
mod candidate;

/// Container for everything `extract_all` returns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Extracted {
    /// Raw email addresses found, normalized to lowercase. De-duplicated.
    pub emails: Vec<String>,
    /// Phone numbers in `+49…` form, deduplicated. Numbers detected as a
    /// fax (see [`Extracted::fax`]) are removed from this list.
    pub phones: Vec<String>,
    /// Fax / Telefax number in `+49…` form, if one was explicitly labeled.
    pub fax: Option<String>,
    /// First detected German postcode (5 digits).
    pub postcode: Option<String>,
    /// First detected city following the postcode.
    pub city: Option<String>,
    /// First detected street + house number combination.
    pub street: Option<String>,
    /// HR (Handelsregister) number, e.g. "HRB 12345 B".
    pub hr_number: Option<String>,
    /// HR court, e.g. "Berlin (Charlottenburg)".
    pub hr_court: Option<String>,
    /// USt-IdNr. (DE + 9 digits, no spaces).
    pub vat_id: Option<String>,
    /// Steuernummer.
    pub tax_number: Option<String>,
    /// German IBAN, normalized without spaces (e.g. `DE89370400440532013000`).
    pub iban: Option<String>,
    /// BIC / SWIFT code, uppercased (e.g. `COBADEFFXXX`).
    pub bic: Option<String>,
    /// Detected legal form (canonicalized).
    pub legal_form: Option<String>,
    /// Year company was founded.
    pub year_founded: Option<i32>,
    /// Persons mentioned in the role of Geschäftsführer / Inhaber / Vorstand /
    /// Verantwortlicher / Vertretungsberechtigt.
    pub persons: Vec<Person>,
}

/// A single person mentioned in the Impressum (typically a managing director).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Person {
    /// Best-effort first name (after stripping titles like Dr., Prof.).
    pub first_name: Option<String>,
    /// Last name (last token in the name string).
    pub last_name: Option<String>,
    /// The raw matched string from the source, useful for debugging.
    pub full_raw: String,
    /// Detected role: "Geschäftsführer" | "Inhaber" | "Vorstand" | "Verantwortlich" | None.
    pub role: Option<String>,
}

/// A single postal address (one per address-bearing text block).
///
/// Each field is independent: a block containing only a street yields an
/// `Address` with `street: Some(..)` and the rest `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Address {
    /// German postcode (5 digits), if present in the block.
    pub postcode: Option<String>,
    /// City following the postcode, if present.
    pub city: Option<String>,
    /// Street + house number, if present in the block.
    pub street: Option<String>,
}

pub use scored::{Scored, ScoredExtracted};

// ───────────────────────── Regexes ─────────────────────────

static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,}").unwrap());

static EMAIL_OBFUSCATED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b([a-z0-9._%+\-]+)\s*[\[\(]?\s*(?:at|@)\s*[\]\)]?\s*([a-z0-9.\-]+)\s*[\[\(]?\s*(?:dot|\.)\s*[\]\)]?\s*([a-z]{2,})\b",
    )
    .unwrap()
});

static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        (?:\+49|0049|0)
        [\s\-/]?
        (?:\(0?\)|\d{1,5})
        [\s\-/]?
        \d{2,4}
        [\s\-/]?
        \d{2,4}
        (?:[\s\-/]?\d{0,4})?
        ",
    )
    .unwrap()
});

static GERMAN_POSTCODE_AND_CITY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{5})\s+([A-ZÄÖÜ][A-Za-zÄÖÜäöüß\-/. ]{1,40})\b").unwrap());

static STREET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b([A-ZÄÖÜ][A-Za-zÄÖÜäöüß\-./ ]{2,60}?(?:str(?:asse|aße|\.)|weg|allee|platz|ring|gasse|damm))\s+(\d{1,4}[a-zA-Z]?(?:[\-–]\d{1,4}[a-zA-Z]?)?)\b",
    )
    .unwrap()
});

static HR_NUMBER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bHR[AB]?\s*[:\-]?\s*(?:Nr\.?\s*)?(\d{2,7}(?:\s*[A-Z])?)\b").unwrap()
});

// The court-name capture stops at the first digit, comma or newline (none of
// those characters are in the class). A trailing Handelsregister prefix
// (`HRB`/`HRA`) that shares the line with the court is stripped afterwards by
// `clean_hr_court`. See issue #26.
static HR_COURT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Amtsgericht\s+([A-ZÄÖÜ][A-Za-zÄÖÜäöüß\-./ ()]{1,60})").unwrap()
});

// A Handelsregister prefix (`HRA`/`HRB`/bare `HR`) that the greedy court-name
// capture may have swallowed when no separator preceded it. Case-sensitive so
// it never clips a city name like "Ahrensburg". See issue #26.
static HR_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+HR[AB]?\b").unwrap());

// USt-IdNr.: `DE` followed by exactly 9 digits, which may be grouped with
// internal spaces (e.g. `DE 123 456 789`). Normalization strips the spaces.
static VAT_DE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bDE[\s\-]?(?:\d[\s\-]?){8}\d\b").unwrap());

// German IBAN: `DE` + 2 check digits + 18 BBAN digits (22 chars total),
// commonly written in groups of four with spaces.
static IBAN_DE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bDE(?:[\s\-]?\d){20}\b").unwrap());

// BIC / SWIFT: 4 letters (bank) + 2 letters (country) + 2 alphanumerics
// (location) + optional 3 alphanumerics (branch). Matched case-insensitively
// and only accepted in a banking context (see `extract_bic`).
static BIC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}(?:[A-Z0-9]{3})?\b").unwrap());

// A fax/telefax label immediately followed by a German phone number.
static FAX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        (?:tele)?fax\.?\s*[:\-]?\s*
        ((?:\+49|0049|0)
         [\s\-/]?(?:\(0?\)|\d{1,5})
         [\s\-/]?\d{2,4}
         [\s\-/]?\d{2,4}
         (?:[\s\-/]?\d{0,4})?)
        ",
    )
    .unwrap()
});

// Matches "Steuernummer" plus the common abbreviations "Steuer-Nr.",
// "St.-Nr.", "StNr." and "St.Nr.". See issue #32.
static TAX_NUMBER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        (?:Steuer-?nummer | Steuer-?Nr\.? | St\.?-?Nr\.?)
        [:\s]*
        ([\d/\s]{8,20})
        ",
    )
    .unwrap()
});

static GF_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        (?:
            Gesch(?:ä|ae)ftsf(?:ü|ue)hrer(?:in)?
          | Inhaber(?:in)?
          | Vorstand
          | Vertretungsberechtigt(?:e[rn]?)?
          | vertreten\s+durch
          | Verantwortlich(?:e[rn]?)?\s+f(?:ü|ue)r\s+den\s+Inhalt
          | Redaktionell\s+verantwortlich
          | Verantwortlich(?:e[rn]?)?(?:\s+(?:i\.?\s?S\.?\s?[dv]\.?|gem(?:ä|ae)ß|nach)[^:\n]{0,40})?
        )
        [\s:\-]+
        (?:sind\s+|ist\s+)?
        ([A-ZÄÖÜ][\w\s.\-,&/äöüÄÖÜß]{2,160})
        ",
    )
    .unwrap()
});

static YEAR_FOUNDED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        \b
        (?:gegr(?:ü|ue)ndet|seit|founded\s*in?|since|established\s*in?|gründungsjahr)
        \s*[:\-]?\s*
        (\d{4})
        \b
        ",
    )
    .unwrap()
});

static LEGAL_FORM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        \b
        (
            GmbH\s*&\s*Co\.?\s*KGaA
          | GmbH\s*&\s*Co\.?\s*KG
          | KGaA
          | GmbH
          | UG\s*\(?haftungsbeschr(?:ä|ae)nkt\)?
          | UG
          | AG
          | KG
          | OHG
          | GbR
          | e\.K\.
          | eG
          | SE
        )
        \b
        ",
    )
    .unwrap()
});

// ───────────────────────── Blocklists ─────────────────────────

/// Allowlist of real top-level domains an email address may end in.
///
/// The email regex accepts any `[a-z]{2,}` sequence as a TLD, which lets
/// code fragments (`…@….css`, `…@….js`) *and* prose words picked out of
/// HTML-to-text output (`…matters. Discover…` → `th@matters.discover`) leak
/// through as bogus addresses. Validating the TLD against a compact set of
/// genuine TLDs — country-code plus the common generics — removes both
/// classes of false positive. See issues #3 and #15.
const REAL_TLDS: &[&str] = &[
    // Generic
    "com", "net", "org", "info", "biz", "name", "pro", "io", "co", "gov", "edu", "mil", "int", "eu",
    "app", "dev", "shop", "online", "site", "tech", "gmbh", "email", "de", "berlin",
    // German-speaking / neighbouring Europe (primary market)
    "at", "ch", "li", "lu", "nl", "be", "fr", "it", "es", "pt", "pl", "cz", "sk", "hu", "dk", "se",
    "no", "fi", "is", "ie", "uk", "gr", "ro", "bg", "hr", "si", "rs", "ua",
    // Rest of world (common in international B2B)
    "us", "ca", "mx", "br", "ar", "au", "nz", "jp", "cn", "kr", "in", "sg", "hk", "tw", "za", "ru",
    "tr", "ae", "sa", "il",
];

/// Domains observed as false positives from theme/widget code. See issue #3.
const BLOCKED_EMAIL_DOMAINS: &[&str] = &["sapp.com", "tet.soweit"];

/// Tokens that are never a person name: German articles, prepositions,
/// pronouns, common verbs, and legal/role terms. See issue #4.
const NOT_A_NAME: &[&str] = &[
    // Articles
    "der",
    "die",
    "das",
    "den",
    "dem",
    "des",
    "ein",
    "eine",
    "einem",
    "einer",
    "einen",
    "eines",
    // Prepositions
    "in",
    "an",
    "auf",
    "zu",
    "zum",
    "zur",
    "von",
    "bei",
    "mit",
    "nach",
    "vor",
    "aus",
    "durch",
    "ohne",
    "gegen",
    "um",
    "für",
    "über",
    "unter",
    "zwischen",
    "als",
    // Pronouns / particles
    "sich",
    "uns",
    "wir",
    "ihre",
    "ihren",
    "ihrer",
    "seinen",
    "seine",
    "seiner",
    "deren",
    "dessen",
    "nicht",
    "auch",
    "nur",
    "noch",
    "schon",
    "sowie",
    "und",
    "oder",
    // Common verbs near role keywords
    "ist",
    "hat",
    "sind",
    "haben",
    "wird",
    "werden",
    "kann",
    "soll",
    "bemühen",
    "sollten",
    "vertreten",
    // Legal / role terms
    "geschäftsführer",
    "geschäftsführerin",
    "geschäftsführung",
    "gesellschafterin",
    "gesellschafter",
    "vorstand",
    "vorständin",
    "inhaber",
    "inhaberin",
    "vertretungsberechtigt",
    "verantwortlich",
    "domain",
    "inhalt",
    "inhalte",
    "gmbh",
    "ug",
    "ag",
    "ohg",
    "gbr",
    "kg",
    "eg",
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
];

// ───────────────────────── Public API ─────────────────────────

/// Extract every supported field from a free-form text blob.
///
/// `text` should be the visible text of an Impressum page (or the whole
/// site — the extractors are forgiving).
pub fn extract_all(text: &str) -> Extracted {
    let doc = segment::Document::parse(normalize::normalize_text(text));
    build_extracted(&doc)
}

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

fn build_extracted(doc: &segment::Document) -> Extracted {
    let text = doc.text();
    // Fax (labeled) is extracted first so it can be excluded from phones.
    let fax = extract_fax_core(text);
    // IBANs contain long digit runs that the phone regex would otherwise pick
    // up as bogus numbers, so skip any phone match overlapping an IBAN.
    let iban_spans: Vec<(usize, usize)> = IBAN_DE_RE
        .find_iter(text)
        .map(|m| (m.start(), m.end()))
        .collect();
    let phones = collect_phones(text, &iban_spans)
        .into_iter()
        .filter(|p| fax.as_deref() != Some(p.as_str()))
        .collect();

    let (postcode, city, street) = address_from_document(doc);

    let hr_number = extract_hr_number_core(text);
    let hr_court = extract_hr_court_core(text);
    let tax_number = extract_tax_number_core(text);

    Extracted {
        emails: extract_emails_core(text),
        phones,
        fax,
        postcode,
        city,
        street,
        hr_number,
        hr_court,
        vat_id: extract_vat_id_core(text),
        tax_number,
        iban: extract_iban_core(text),
        bic: extract_bic_core(text),
        legal_form: extract_legal_form_core(text),
        year_founded: extract_year_founded_core(text),
        persons: extract_persons_core(text),
    }
}

/// Return all email addresses (plain and obfuscated `info [at] domain [dot] de`),
/// lowercased and deduplicated. Code-fragment false positives (invalid TLDs
/// like `.css`/`.js`, known junk domains) are filtered out.
pub fn extract_emails(text: &str) -> Vec<String> {
    extract_emails_core(&normalize::normalize_text(text))
}

fn extract_emails_core(text: &str) -> Vec<String> {
    let mut emails: BTreeSet<String> = BTreeSet::new();
    for m in EMAIL_RE.find_iter(text) {
        let e = m.as_str().to_ascii_lowercase();
        if is_plausible_email(&e) {
            emails.insert(e);
        }
    }
    for cap in EMAIL_OBFUSCATED_RE.captures_iter(text) {
        if let (Some(local), Some(host), Some(tld)) = (cap.get(1), cap.get(2), cap.get(3)) {
            let e = format!("{}@{}.{}", local.as_str(), host.as_str(), tld.as_str())
                .to_ascii_lowercase();
            if is_plausible_email(&e) {
                emails.insert(e);
            }
        }
    }
    emails.into_iter().collect()
}

/// Return all German phone numbers normalized to `+49…` (digits only).
///
/// Note: this returns *every* matched number, including any labeled as a fax.
/// [`extract_all`] additionally removes the detected [`Extracted::fax`] from
/// its `phones` list; use [`extract_fax`] to obtain it separately.
pub fn extract_phones(text: &str) -> Vec<String> {
    let normalized = normalize::normalize_text(text);
    collect_phones(normalized.as_str(), &[])
}

/// Collect normalized phone numbers, skipping matches that overlap any of the
/// given byte spans (used by [`extract_all`] to drop IBAN digit runs).
fn collect_phones(text: &str, skip_spans: &[(usize, usize)]) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for m in PHONE_RE.find_iter(text) {
        if skip_spans
            .iter()
            .any(|(s, e)| m.start() < *e && m.end() > *s)
        {
            continue;
        }
        let p = clean_phone(m.as_str());
        if p.len() >= 7 {
            out.insert(p);
        }
    }
    out.into_iter().collect()
}

/// Extract a labeled Fax / Telefax number, normalized to `+49…`.
pub fn extract_fax(text: &str) -> Option<String> {
    extract_fax_core(&normalize::normalize_text(text))
}

fn extract_fax_core(text: &str) -> Option<String> {
    let cap = FAX_RE.captures(text)?;
    let p = clean_phone(cap.get(1)?.as_str());
    if p.len() >= 7 { Some(p) } else { None }
}

/// Extract a German IBAN, normalized without spaces (e.g. `DE89370400440532013000`).
pub fn extract_iban(text: &str) -> Option<String> {
    extract_iban_core(&normalize::normalize_text(text))
}

fn extract_iban_core(text: &str) -> Option<String> {
    let m = IBAN_DE_RE.find(text)?;
    let normalized: String = m
        .as_str()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();
    // German IBAN is exactly 22 characters.
    if normalized.len() == 22 {
        Some(normalized)
    } else {
        None
    }
}

/// Extract a BIC / SWIFT code (uppercased). Only accepted when it appears in a
/// banking context (`BIC`, `SWIFT`, or `Bankverbindung` nearby) to avoid
/// matching ordinary uppercase words.
pub fn extract_bic(text: &str) -> Option<String> {
    extract_bic_core(&normalize::normalize_text(text))
}

fn extract_bic_core(text: &str) -> Option<String> {
    for m in BIC_RE.find_iter(text) {
        let candidate = m.as_str().to_ascii_uppercase();
        // The country part (chars 5-6) must be letters; require a banking
        // keyword within the preceding ~40 chars of context.
        let start = m.start();
        // Snap to a char boundary so multi-byte codepoints in the preceding
        // context (e.g. a zero-width space) can't cause a mid-codepoint slice
        // panic. See issue #13.
        let ctx_start = floor_char_boundary(text, start.saturating_sub(40));
        let ctx = text[ctx_start..start].to_ascii_lowercase();
        if ctx.contains("bic") || ctx.contains("swift") || ctx.contains("bankverbindung") {
            return Some(candidate);
        }
    }
    None
}

/// Extract the first German address (`(postcode, city, street)`) from text.
///
/// Postcode/city and street are drawn from the *same* block when possible, so
/// pages listing multiple addresses do not mix parts across entities.
pub fn extract_address(text: &str) -> (Option<String>, Option<String>, Option<String>) {
    let doc = segment::Document::parse(normalize::normalize_text(text));
    address_from_document(&doc)
}

/// Extract every postal address on the page — one [`Address`] per text block
/// that contains a postcode/city and/or a street, in document order, with exact
/// duplicates removed.
///
/// Unlike [`extract_address`] (which returns only the first address), this is
/// intended for pages listing multiple locations/branches. Address components
/// are only ever combined within a single block, so parts from different
/// entities are never mixed.
pub fn extract_addresses(text: &str) -> Vec<Address> {
    let doc = segment::Document::parse(normalize::normalize_text(text));
    addresses_from_document(&doc)
}

fn addresses_from_document(doc: &segment::Document) -> Vec<Address> {
    let mut out: Vec<Address> = Vec::new();
    for block in doc.block_texts() {
        let pc = parse_postcode_city(block);
        let street = parse_street(block);
        if pc.is_none() && street.is_none() {
            continue;
        }
        let (postcode, city) = match pc {
            Some((code, city)) => (Some(code), Some(city)),
            None => (None, None),
        };
        let addr = Address {
            postcode,
            city,
            street,
        };
        if !out.contains(&addr) {
            out.push(addr);
        }
    }
    out
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
            // TODO(TP2): real span + label when scoring consumes these
            street_cands.push(Candidate::new(sv, 0..0, idx, None));
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

/// Detect the German legal form mentioned in the text (e.g. "GmbH", "GmbH & Co. KG", "AG").
pub fn extract_legal_form(text: &str) -> Option<String> {
    extract_legal_form_core(&normalize::normalize_text(text))
}

fn extract_legal_form_core(text: &str) -> Option<String> {
    LEGAL_FORM_RE
        .find(text)
        .map(|m| canonicalize_legal_form(m.as_str()))
}

/// Extract the HR (Handelsregister) number (e.g. `HRB 12345 B`).
pub fn extract_hr_number(text: &str) -> Option<String> {
    extract_hr_number_core(&normalize::normalize_text(text))
}

fn extract_hr_number_core(text: &str) -> Option<String> {
    HR_NUMBER_RE.captures(text).map(|c| {
        c.get(0)
            .unwrap()
            .as_str()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// Extract the Handelsregister court that follows an `Amtsgericht` label
/// (e.g. `"Berlin"`, `"Berlin (Charlottenburg)"`).
///
/// A Handelsregister prefix sharing the same line as the court name
/// (`Amtsgericht Berlin HRB 12345`) is stripped, so this returns `"Berlin"`
/// rather than `"Berlin HRB"`. See issue #26.
pub fn extract_hr_court(text: &str) -> Option<String> {
    extract_hr_court_core(&normalize::normalize_text(text))
}

fn extract_hr_court_core(text: &str) -> Option<String> {
    HR_COURT_RE
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| clean_hr_court(m.as_str()))
        .filter(|s| !s.is_empty())
}

/// Extract a Steuernummer (local German tax number), e.g. `"28/815/0815 1"`.
///
/// Recognizes the full word `Steuernummer` as well as the common abbreviations
/// `Steuer-Nr.`, `St.-Nr.`, `StNr.` and `St.Nr.`. See issues #31 and #32.
pub fn extract_tax_number(text: &str) -> Option<String> {
    extract_tax_number_core(&normalize::normalize_text(text))
}

fn extract_tax_number_core(text: &str) -> Option<String> {
    TAX_NUMBER_RE
        .captures(text)
        .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
}

/// Extract the German VAT-ID (USt-IdNr., format `DE` + 9 digits).
///
/// Matches are allowed to contain internal grouping spaces (`DE 123 456 789`).
/// Any candidate that overlaps a detected IBAN is skipped, since an IBAN also
/// starts with `DE` followed by digits.
pub fn extract_vat_id(text: &str) -> Option<String> {
    extract_vat_id_core(&normalize::normalize_text(text))
}

fn extract_vat_id_core(text: &str) -> Option<String> {
    let iban_spans: Vec<(usize, usize)> = IBAN_DE_RE
        .find_iter(text)
        .map(|m| (m.start(), m.end()))
        .collect();
    for m in VAT_DE_RE.find_iter(text) {
        let overlaps_iban = iban_spans
            .iter()
            .any(|(s, e)| m.start() < *e && m.end() > *s);
        if overlaps_iban {
            continue;
        }
        let normalized: String = m
            .as_str()
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect();
        return Some(normalized.to_ascii_uppercase());
    }
    None
}

/// Extract the company's founding year.
pub fn extract_year_founded(text: &str) -> Option<i32> {
    extract_year_founded_core(&normalize::normalize_text(text))
}

fn extract_year_founded_core(text: &str) -> Option<i32> {
    let cap = YEAR_FOUNDED_RE.captures(text)?;
    let y: i32 = cap.get(1)?.as_str().parse().ok()?;
    if (1700..=2100).contains(&y) {
        Some(y)
    } else {
        None
    }
}

/// Extract Geschäftsführer / Inhaber / Vorstand / Verantwortlicher persons.
pub fn extract_persons(text: &str) -> Vec<Person> {
    extract_persons_core(&normalize::normalize_text(text))
}

fn extract_persons_core(text: &str) -> Vec<Person> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for cap in GF_REGEX.captures_iter(text) {
        let Some(m) = cap.get(1) else { continue };
        // The role keyword is in the full match (group 0), not group 1.
        let role = detect_role(cap.get(0).map(|g| g.as_str()).unwrap_or(""));
        let trimmed = truncate_at_sentence_end(m.as_str().trim());
        for piece in split_persons(&trimmed) {
            let p = parse_person(&piece, role.clone());
            if p.last_name.is_none() {
                continue;
            }
            let key = format!(
                "{}_{}",
                p.first_name.clone().unwrap_or_default(),
                p.last_name.clone().unwrap_or_default()
            );
            if seen.insert(key) {
                out.push(p);
            }
        }
    }
    out
}

// ───────────────────────── Helpers ─────────────────────────

/// Snap a byte `index` down to the nearest UTF-8 character boundary at or
/// below it, returning a value that is always safe to slice `s` at.
///
/// This is a hand-rolled, MSRV-safe equivalent of `str::floor_char_boundary`
/// (only stabilized in Rust 1.91, above this crate's 1.85 MSRV). Prevents the
/// mid-codepoint slice panics reported in issues #13 and #14.
fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    let mut i = index;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn clean_phone(s: &str) -> String {
    let s = s.nfc().collect::<String>();
    let s: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();
    if let Some(rest) = s.strip_prefix("0049") {
        format!("+49{}", rest)
    } else if let Some(rest) = s.strip_prefix('0') {
        if rest.starts_with('+') {
            s
        } else {
            format!("+49{}", rest)
        }
    } else {
        s
    }
}

/// Strip a trailing Handelsregister prefix (`HRB`/`HRA`/`HR`) that the greedy
/// court-name capture may have swallowed, then trim. See issue #26.
fn clean_hr_court(raw: &str) -> String {
    let trimmed = raw.trim();
    let cut = HR_SUFFIX_RE
        .find(trimmed)
        .map(|m| m.start())
        .unwrap_or(trimmed.len());
    trimmed[..cut].trim().to_string()
}

fn canonicalize_legal_form(raw: &str) -> String {
    let r = raw.trim();
    let lower = r.to_ascii_lowercase();
    // KGaA variants must be checked before the plain KG / GmbH cases, since
    // "GmbH & Co. KGaA" also contains "co", "kg", and "gmbh". See issue #30.
    if lower.contains("co") && lower.contains("kgaa") {
        "GmbH & Co. KGaA".to_string()
    } else if lower.contains("kgaa") {
        "KGaA".to_string()
    } else if lower.contains("co") && lower.contains("kg") {
        "GmbH & Co. KG".to_string()
    } else if r.to_ascii_uppercase().contains("GMBH") {
        "GmbH".to_string()
    } else if r.to_ascii_uppercase().contains("UG") {
        "UG".to_string()
    } else {
        r.to_string()
    }
}

fn truncate_at_sentence_end(s: &str) -> String {
    let stop_words = [
        " Sitz",
        " Tel",
        " Telefon",
        " Fax",
        " E-Mail",
        " Email",
        " USt",
        " Eingetragen",
        " HRB",
        " HRA",
        " Steuer",
        " Adresse",
        " Anschrift",
        " Datenschutz",
        " Web",
        " Geschäfts",
        " Handelsregister",
        " Amtsgericht",
        " UID",
    ];
    let mut end = s.len();
    for sw in stop_words {
        if let Some(pos) = s.find(sw) {
            end = end.min(pos);
        }
    }
    // Stop at first newline — names rarely span line breaks.
    if let Some(pos) = s.find(['\n', '\r']) {
        end = end.min(pos);
    }
    // A stop-word/newline byte offset can land inside a multi-byte codepoint
    // (soft hyphens, non-breaking spaces, em-dashes …); snap back to a valid
    // boundary before slicing to avoid a panic. See issue #14.
    let end = floor_char_boundary(s, end);
    s[..end]
        .trim()
        .trim_end_matches(',')
        .trim_end_matches('.')
        .to_string()
}

fn split_persons(raw: &str) -> Vec<String> {
    let separators = [" und ", " sowie ", " & ", ",", ";", " / "];
    let mut parts = vec![raw.to_string()];
    for sep in separators {
        let mut next = Vec::new();
        for p in parts.drain(..) {
            for sub in p.split(sep) {
                let s = sub.trim();
                if !s.is_empty() {
                    next.push(s.to_string());
                }
            }
        }
        parts = next;
    }
    parts
}

fn parse_person(raw: &str, role: Option<String>) -> Person {
    let cleaned = strip_titles(raw);
    // Keep only tokens that plausibly belong to a person's name.
    let parts: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|t| is_valid_name_part(t))
        .collect();
    match parts.len() {
        // No usable name tokens — drop it (last_name stays None, filtered upstream).
        0 => Person {
            full_raw: raw.to_string(),
            role,
            ..Default::default()
        },
        1 => Person {
            last_name: Some(parts[0].to_string()),
            full_raw: raw.to_string(),
            role,
            ..Default::default()
        },
        _ => Person {
            first_name: Some(parts[0].to_string()),
            last_name: Some(parts[parts.len() - 1].to_string()),
            full_raw: raw.to_string(),
            role,
        },
    }
}

/// Detect the role from the full regex match (which still contains the keyword).
fn detect_role(full_match: &str) -> Option<String> {
    let lower = full_match.to_ascii_lowercase();
    if lower.contains("gesch") {
        Some("Geschäftsführer".to_string())
    } else if lower.contains("vorstand") {
        Some("Vorstand".to_string())
    } else if lower.contains("verantwortlich") {
        Some("Verantwortlich".to_string())
    } else if lower.contains("inhaber") {
        Some("Inhaber".to_string())
    } else {
        None
    }
}

/// Reject tokens that are obviously not name parts: blocklisted words,
/// single characters (truncation artifacts), or lowercase-initial tokens
/// (German names are always capitalized). See issue #4.
fn is_valid_name_part(s: &str) -> bool {
    let trimmed = s.trim().trim_matches(|c: char| !c.is_alphanumeric());
    if trimmed.chars().count() <= 1 {
        return false;
    }
    if trimmed.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    if NOT_A_NAME.contains(&trimmed.to_lowercase().as_str()) {
        return false;
    }
    match trimmed.chars().next() {
        Some(first) => first.is_uppercase(),
        None => false,
    }
}

/// Strip academic/courtesy titles using whole-token matching so that names
/// embedding a title substring (e.g. "Herrmann", "Draxler") are preserved.
/// See issue #2.
fn strip_titles(s: &str) -> String {
    const TITLES: &[&str] = &[
        "dr",
        "dr.",
        "prof",
        "prof.",
        "dipl.-ing.",
        "dipl.-kfm.",
        "dipl.-ök.",
        "dipl",
        "b.sc.",
        "m.sc.",
        "mba",
        "b.a.",
        "m.a.",
        "ll.m.",
        "ll.b.",
        "herr",
        "frau",
        "ra",
        "h.c.",
    ];
    s.split_whitespace()
        .filter(|token| {
            let key = token.to_ascii_lowercase();
            !TITLES.contains(&key.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reject emails whose domain is blocklisted or whose TLD is not a real
/// top-level domain (filters both code fragments and prose false positives).
/// See issues #3 and #15.
fn is_plausible_email(email: &str) -> bool {
    let Some((_, domain)) = email.rsplit_once('@') else {
        return false;
    };
    if BLOCKED_EMAIL_DOMAINS.contains(&domain) {
        return false;
    }
    match domain.rsplit_once('.') {
        Some((_, tld)) => REAL_TLDS.contains(&tld),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_basic_and_obfuscated() {
        let txt = "info@a.de info [at] b [dot] de support@a.de";
        let e = extract_emails(txt);
        assert!(e.contains(&"info@a.de".to_string()));
        assert!(e.contains(&"info@b.de".to_string()));
        assert!(e.contains(&"support@a.de".to_string()));
    }

    #[test]
    fn phones_normalised() {
        let p = extract_phones("Tel +49 30 12345-67 oder 0049 30 12345 68 oder 030/123 45 69");
        assert!(p.iter().any(|x| x.starts_with("+493012345")));
    }

    #[test]
    fn legal_forms() {
        assert_eq!(
            extract_legal_form("Muster GmbH & Co. KG"),
            Some("GmbH & Co. KG".into())
        );
        assert_eq!(extract_legal_form("Muster GmbH"), Some("GmbH".into()));
        assert_eq!(
            extract_legal_form("Muster UG (haftungsbeschränkt)"),
            Some("UG".into())
        );
    }

    #[test]
    fn year_founded_pattern() {
        assert_eq!(
            extract_year_founded("Unser Unternehmen wurde gegründet 1973"),
            Some(1973)
        );
        assert_eq!(extract_year_founded("Founded in 2010"), Some(2010));
    }

    #[test]
    fn vat_id_extracted() {
        assert_eq!(
            extract_vat_id("USt-ID: DE 123456789"),
            Some("DE123456789".into())
        );
    }

    #[test]
    fn hr_number_extracted() {
        assert!(extract_hr_number("Eingetragen im Handelsregister Berlin HRB 12345 B").is_some());
    }

    #[test]
    fn persons_with_titles_and_connectors() {
        let p = extract_persons("Geschäftsführer: Dr. Hans Müller und Anna Schmidt");
        assert!(p.iter().any(|x| x.last_name.as_deref() == Some("Müller")));
        assert!(p.iter().any(|x| x.last_name.as_deref() == Some("Schmidt")));
    }

    #[test]
    fn full_extract() {
        let text = "
            Musterreinigung GmbH & Co. KG
            Geschäftsführer: Hans Müller
            Hauptstraße 12, 10115 Berlin
            Tel: +49 30 1234567
            E-Mail: info@musterreinigung.de
            Eingetragen im Handelsregister Berlin HRB 12345 B
            USt-IdNr.: DE 123456789
            Gegründet 1985
        ";
        let d = extract_all(text);
        assert_eq!(d.legal_form.as_deref(), Some("GmbH & Co. KG"));
        assert_eq!(d.postcode.as_deref(), Some("10115"));
        assert!(d.emails.contains(&"info@musterreinigung.de".to_string()));
        assert_eq!(d.year_founded, Some(1985));
        assert_eq!(d.vat_id.as_deref(), Some("DE123456789"));
        assert!(
            d.persons
                .iter()
                .any(|p| p.last_name.as_deref() == Some("Müller"))
        );
    }
}
