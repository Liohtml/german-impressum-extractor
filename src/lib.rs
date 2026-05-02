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
//! - 🪪 Pulls out: HR-Nummer, HR-Court, USt-IdNr., Steuernummer.
//! - 👥 Geschäftsführer / Inhaber / Vorstand name extraction with role tag.
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

use once_cell::sync::Lazy;
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

/// Container for everything `extract_all` returns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Extracted {
    /// Raw email addresses found, normalized to lowercase. De-duplicated.
    pub emails: Vec<String>,
    /// Phone numbers in `+49…` form, deduplicated.
    pub phones: Vec<String>,
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
    /// Detected legal form (canonicalized).
    pub legal_form: Option<String>,
    /// Year company was founded.
    pub year_founded: Option<i32>,
    /// Persons mentioned in the role of Geschäftsführer / Inhaber / Vorstand / Vertretungsberechtigt.
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
    /// Detected role: "Geschäftsführer" | "Inhaber" | "Vorstand" | None.
    pub role: Option<String>,
}

// ───────────────────────── Regexes ─────────────────────────

static EMAIL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)[a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,}").unwrap());

static EMAIL_OBFUSCATED_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b([a-z0-9._%+\-]+)\s*[\[\(]?\s*(?:at|@)\s*[\]\)]?\s*([a-z0-9.\-]+)\s*[\[\(]?\s*(?:dot|\.)\s*[\]\)]?\s*([a-z]{2,})\b",
    )
    .unwrap()
});

static PHONE_RE: Lazy<Regex> = Lazy::new(|| {
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

static GERMAN_POSTCODE_AND_CITY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(\d{5})\s+([A-ZÄÖÜ][A-Za-zÄÖÜäöüß\-/. ]{1,40})\b").unwrap());

static STREET_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"\b([A-ZÄÖÜ][A-Za-zÄÖÜäöüß\-./ ]{2,60}?(?:str(?:asse|aße|\.)|weg|allee|platz|ring|gasse|damm))\s+(\d{1,4}[a-zA-Z]?(?:[\-–]\d{1,4}[a-zA-Z]?)?)\b",
    )
    .unwrap()
});

static HR_NUMBER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\bHR[AB]?\s*[:\-]?\s*(?:Nr\.?\s*)?(\d{2,7}(?:\s*[A-Z])?)\b").unwrap()
});

static HR_COURT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)Amtsgericht\s+([A-ZÄÖÜ][A-Za-zÄÖÜäöüß\-./ ]{2,40})").unwrap()
});

static VAT_DE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bDE\s*\d{9}\b").unwrap());

static TAX_NUMBER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)Steuer-?nummer[:\s]*([\d/\s]{8,20})").unwrap());

static GF_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?ix)
        (?:
            Gesch(?:ä|ae)ftsf(?:ü|ue)hrer(?:in)?
          | Inhaber(?:in)?
          | Vorstand
          | Vertretungsberechtigt(?:e[rn]?)?
          | vertreten\s+durch
        )
        [\s:\-]+
        (?:sind\s+|ist\s+)?
        ([A-ZÄÖÜ][\w\s.\-,&/äöüÄÖÜß]{2,160})
        ",
    )
    .unwrap()
});

static YEAR_FOUNDED_RE: Lazy<Regex> = Lazy::new(|| {
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

static LEGAL_FORM_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?ix)
        \b
        (
            GmbH\s*&\s*Co\.?\s*KG
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

// ───────────────────────── Public API ─────────────────────────

/// Extract every supported field from a free-form text blob.
///
/// `text` should be the visible text of an Impressum page (or the whole
/// site — the extractors are forgiving).
pub fn extract_all(text: &str) -> Extracted {
    let mut out = Extracted::default();

    // Emails (plain + obfuscated)
    let mut emails: BTreeSet<String> = BTreeSet::new();
    for m in EMAIL_RE.find_iter(text) {
        emails.insert(m.as_str().to_ascii_lowercase());
    }
    for cap in EMAIL_OBFUSCATED_RE.captures_iter(text) {
        if let (Some(local), Some(host), Some(tld)) = (cap.get(1), cap.get(2), cap.get(3)) {
            emails.insert(
                format!("{}@{}.{}", local.as_str(), host.as_str(), tld.as_str())
                    .to_ascii_lowercase(),
            );
        }
    }
    out.emails = emails.into_iter().collect();

    // Phones
    let mut phones: BTreeSet<String> = BTreeSet::new();
    for m in PHONE_RE.find_iter(text) {
        let p = clean_phone(m.as_str());
        if p.len() >= 7 {
            phones.insert(p);
        }
    }
    out.phones = phones.into_iter().collect();

    // Address
    if let Some(cap) = GERMAN_POSTCODE_AND_CITY_RE.captures(text) {
        out.postcode = cap.get(1).map(|m| m.as_str().to_string());
        out.city = cap.get(2).map(|m| m.as_str().trim().to_string());
    }
    if let Some(cap) = STREET_RE.captures(text) {
        out.street = Some(format!(
            "{} {}",
            cap.get(1).map(|m| m.as_str().trim()).unwrap_or(""),
            cap.get(2).map(|m| m.as_str().trim()).unwrap_or("")
        ));
    }

    // HR / VAT / Tax
    if let Some(cap) = HR_NUMBER_RE.captures(text) {
        let raw = cap.get(0).unwrap().as_str();
        out.hr_number = Some(raw.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    if let Some(cap) = HR_COURT_RE.captures(text) {
        out.hr_court = cap.get(1).map(|m| m.as_str().trim().to_string());
    }
    if let Some(m) = VAT_DE_RE.find(text) {
        out.vat_id = Some(m.as_str().split_whitespace().collect::<Vec<_>>().join(""));
    }
    if let Some(cap) = TAX_NUMBER_RE.captures(text) {
        out.tax_number = cap.get(1).map(|m| m.as_str().trim().to_string());
    }

    // Legal form
    if let Some(m) = LEGAL_FORM_RE.find(text) {
        out.legal_form = Some(canonicalize_legal_form(m.as_str()));
    }

    // Year founded
    if let Some(cap) = YEAR_FOUNDED_RE.captures(text)
        && let Some(year_match) = cap.get(1)
        && let Ok(y) = year_match.as_str().parse::<i32>()
        && (1700..=2100).contains(&y)
    {
        out.year_founded = Some(y);
    }

    // Persons
    out.persons = extract_persons(text);

    out
}

/// Return all email addresses (plain and obfuscated `info [at] domain [dot] de`),
/// lowercased and deduplicated.
pub fn extract_emails(text: &str) -> Vec<String> {
    let mut emails: BTreeSet<String> = BTreeSet::new();
    for m in EMAIL_RE.find_iter(text) {
        emails.insert(m.as_str().to_ascii_lowercase());
    }
    for cap in EMAIL_OBFUSCATED_RE.captures_iter(text) {
        if let (Some(local), Some(host), Some(tld)) = (cap.get(1), cap.get(2), cap.get(3)) {
            emails.insert(
                format!("{}@{}.{}", local.as_str(), host.as_str(), tld.as_str())
                    .to_ascii_lowercase(),
            );
        }
    }
    emails.into_iter().collect()
}

/// Return all German phone numbers normalized to `+49…` (digits only).
pub fn extract_phones(text: &str) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for m in PHONE_RE.find_iter(text) {
        let p = clean_phone(m.as_str());
        if p.len() >= 7 {
            out.insert(p);
        }
    }
    out.into_iter().collect()
}

/// Extract the first German address (`(postcode, city, street)`) from text.
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

/// Detect the German legal form mentioned in the text (e.g. "GmbH", "GmbH & Co. KG", "AG").
pub fn extract_legal_form(text: &str) -> Option<String> {
    LEGAL_FORM_RE.find(text).map(|m| canonicalize_legal_form(m.as_str()))
}

/// Extract the HR (Handelsregister) number (e.g. `HRB 12345 B`).
pub fn extract_hr_number(text: &str) -> Option<String> {
    HR_NUMBER_RE
        .captures(text)
        .map(|c| c.get(0).unwrap().as_str().split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Extract the German VAT-ID (USt-IdNr., format `DE` + 9 digits).
pub fn extract_vat_id(text: &str) -> Option<String> {
    VAT_DE_RE
        .find(text)
        .map(|m| m.as_str().split_whitespace().collect::<Vec<_>>().join(""))
}

/// Extract the company's founding year.
pub fn extract_year_founded(text: &str) -> Option<i32> {
    let cap = YEAR_FOUNDED_RE.captures(text)?;
    let y: i32 = cap.get(1)?.as_str().parse().ok()?;
    if (1700..=2100).contains(&y) {
        Some(y)
    } else {
        None
    }
}

/// Extract Geschäftsführer / Inhaber / Vorstand persons.
pub fn extract_persons(text: &str) -> Vec<Person> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for cap in GF_REGEX.captures_iter(text) {
        let Some(m) = cap.get(1) else { continue };
        let trimmed = truncate_at_sentence_end(m.as_str().trim());
        for piece in split_persons(&trimmed) {
            let p = parse_person(&piece);
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

fn canonicalize_legal_form(raw: &str) -> String {
    let r = raw.trim();
    if r.to_ascii_lowercase().contains("co") && r.to_ascii_lowercase().contains("kg") {
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
        " Sitz", " Tel", " Telefon", " Fax", " E-Mail", " Email",
        " USt", " Eingetragen", " HRB", " HRA", " Steuer", " Adresse",
        " Anschrift", " Datenschutz", " Web", " Geschäfts",
        " Handelsregister", " Amtsgericht", " UID",
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
    s[..end].trim().trim_end_matches(',').trim_end_matches('.').to_string()
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

fn parse_person(raw: &str) -> Person {
    let cleaned = strip_titles(raw);
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    let role = if raw.to_ascii_lowercase().contains("gesch") {
        Some("Geschäftsführer".to_string())
    } else if raw.to_ascii_lowercase().contains("vorstand") {
        Some("Vorstand".to_string())
    } else if raw.to_ascii_lowercase().contains("inhaber") {
        Some("Inhaber".to_string())
    } else {
        None
    };
    match parts.len() {
        0 => Person { full_raw: raw.to_string(), role, ..Default::default() },
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

fn strip_titles(s: &str) -> String {
    let titles = [
        "Dr.", "Dr", "Prof.", "Prof", "Dipl.-Ing.", "Dipl.-Kfm.", "Dipl.-Ök.",
        "B.Sc.", "M.Sc.", "MBA", "B.A.", "M.A.", "Herr", "Frau", "RA",
    ];
    let mut out = s.to_string();
    for t in titles {
        out = out.replace(t, "");
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
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
        assert_eq!(extract_legal_form("Muster GmbH & Co. KG"), Some("GmbH & Co. KG".into()));
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
        assert_eq!(extract_vat_id("USt-ID: DE 123456789"), Some("DE123456789".into()));
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
        assert!(d.persons.iter().any(|p| p.last_name.as_deref() == Some("Müller")));
    }
}
