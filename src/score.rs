//! Heuristic confidence scoring for extracted fields (TP2).
//!
//! Confidence = `clamp(validity_base + label_bonus, 0.0, 1.0)`, where
//! `validity_base` comes from a format check (e.g. IBAN mod-97, postcode
//! range) and `label_bonus` is a small boost when the document contains a
//! segment labeled with the field's matching kind. Scores are heuristic but
//! monotonic: a checksum-valid IBAN always outscores an invalid one, and a
//! labeled field never scores below its unlabeled counterpart.

use crate::scored::{Scored, ScoredExtracted};
use crate::segment::{Document, LabelKind};
use crate::{Extracted, Person};

const LABEL_BONUS: f32 = 0.1;

/// Build a `Scored<T>`, clamping the confidence into `0.0..=1.0`.
fn scored<T>(value: T, confidence: f32) -> Scored<T> {
    Scored {
        value,
        confidence: confidence.clamp(0.0, 1.0),
    }
}

/// Validate an IBAN via the ISO 7064 mod-97 checksum (non-alphanumerics ignored).
pub(crate) fn iban_mod97_valid(iban: &str) -> bool {
    let s: String = iban
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();
    if s.len() < 4 {
        return false;
    }
    // Move the first four characters to the end, then compute mod 97 piecewise.
    let rearranged = format!("{}{}", &s[4..], &s[..4]);
    let mut remainder: u32 = 0;
    for ch in rearranged.chars() {
        let val = if ch.is_ascii_digit() {
            ch as u32 - '0' as u32
        } else {
            (ch as u32 - 'A' as u32) + 10 // A..Z -> 10..35
        };
        remainder = if val >= 10 {
            (remainder * 100 + val) % 97
        } else {
            (remainder * 10 + val) % 97
        };
    }
    remainder == 1
}

fn starts_upper(s: &str) -> bool {
    s.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
}

fn base_iban(v: &str) -> f32 {
    if iban_mod97_valid(v) { 0.95 } else { 0.55 }
}

fn base_postcode(v: &str) -> f32 {
    let ok = v.len() == 5
        && v.chars().all(|c| c.is_ascii_digit())
        && v.parse::<u32>()
            .map(|n| (1000..=99999).contains(&n))
            .unwrap_or(false);
    if ok { 0.9 } else { 0.4 }
}

fn base_phone(v: &str) -> f32 {
    let digits = v.chars().filter(|c| c.is_ascii_digit()).count();
    if v.starts_with("+49") && (7..=15).contains(&digits) {
        0.9
    } else {
        0.6
    }
}

fn base_vat(v: &str) -> f32 {
    let ok = v.len() == 11 && v.starts_with("DE") && v[2..].chars().all(|c| c.is_ascii_digit());
    if ok { 0.9 } else { 0.6 }
}

fn base_year(y: i32) -> f32 {
    if (1700..=2100).contains(&y) { 0.8 } else { 0.3 }
}

fn base_person(p: &Person) -> f32 {
    match (p.first_name.as_deref(), p.last_name.as_deref()) {
        (Some(_), Some(_)) => 0.8,
        (None, Some(_)) => 0.5,
        _ => 0.3,
    }
}

/// Score every field of an already-extracted `Extracted`, using the document
/// for label-presence boosts. Pure post-processing; does not re-extract.
pub(crate) fn score_extracted(base: Extracted, doc: &Document) -> ScoredExtracted {
    let bonus = |kind: LabelKind| {
        if doc.has_label(kind) {
            LABEL_BONUS
        } else {
            0.0
        }
    };

    ScoredExtracted {
        emails: base
            .emails
            .into_iter()
            .map(|e| scored(e, 0.85 + bonus(LabelKind::Email)))
            .collect(),
        phones: base
            .phones
            .into_iter()
            .map(|p| {
                let c = base_phone(&p) + bonus(LabelKind::Phone);
                scored(p, c)
            })
            .collect(),
        fax: base.fax.map(|f| {
            let c = base_phone(&f) + bonus(LabelKind::Fax);
            scored(f, c)
        }),
        postcode: base.postcode.map(|v| {
            let c = base_postcode(&v) + bonus(LabelKind::Postal);
            scored(v, c)
        }),
        city: base.city.map(|v| {
            let c = (if starts_upper(&v) { 0.7 } else { 0.4 }) + bonus(LabelKind::Postal);
            scored(v, c)
        }),
        street: base
            .street
            .map(|v| scored(v, 0.85 + bonus(LabelKind::Postal))),
        hr_number: base
            .hr_number
            .map(|v| scored(v, 0.85 + bonus(LabelKind::Register))),
        hr_court: base
            .hr_court
            .map(|v| scored(v, 0.7 + bonus(LabelKind::Court))),
        vat_id: base.vat_id.map(|v| {
            let c = base_vat(&v) + bonus(LabelKind::VatId);
            scored(v, c)
        }),
        tax_number: base
            .tax_number
            .map(|v| scored(v, 0.75 + bonus(LabelKind::TaxNumber))),
        iban: base.iban.map(|v| {
            let c = base_iban(&v) + bonus(LabelKind::Bank);
            scored(v, c)
        }),
        bic: base.bic.map(|v| scored(v, 0.9 + bonus(LabelKind::Bank))),
        legal_form: base
            .legal_form
            .map(|v| scored(v, 0.9 + bonus(LabelKind::LegalName))),
        year_founded: base.year_founded.map(|y| {
            let c = base_year(y) + bonus(LabelKind::Founded);
            scored(y, c)
        }),
        persons: base
            .persons
            .into_iter()
            .map(|p| {
                let c = base_person(&p) + bonus(LabelKind::Managers);
                scored(p, c)
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iban_mod97_accepts_valid_and_rejects_flipped() {
        assert!(iban_mod97_valid("DE89370400440532013000"));
        assert!(iban_mod97_valid("DE89 3704 0044 0532 0130 00")); // spaces ignored
        assert!(!iban_mod97_valid("DE89370400440532013001")); // last digit changed
        assert!(!iban_mod97_valid("DE")); // too short
    }

    #[test]
    fn valid_iban_outscores_invalid() {
        assert!(base_iban("DE89370400440532013000") > base_iban("DE89370400440532013001"));
    }

    #[test]
    fn postcode_and_phone_bases_are_bounded() {
        assert!(base_postcode("10115") > base_postcode("999")); // valid > malformed
        assert!(base_phone("+493012345678") > base_phone("12"));
    }
}
