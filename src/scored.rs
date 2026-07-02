//! Scored extraction results (TP2). [`crate::extract_all_scored`] returns a
//! [`ScoredExtracted`]; each field carries a heuristic confidence in
//! `0.0..=1.0` (higher means more likely correct).

use crate::Person;

/// A value paired with a heuristic confidence in `0.0..=1.0`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Scored<T> {
    /// The extracted value — identical to what the unscored `extract_all` returns.
    pub value: T,
    /// Heuristic confidence in `0.0..=1.0`; higher means more likely correct.
    pub confidence: f32,
}

/// Scored counterpart of [`crate::Extracted`]: the same fields, each annotated
/// with a confidence. Returned by [`crate::extract_all_scored`].
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScoredExtracted {
    /// Scored email addresses.
    pub emails: Vec<Scored<String>>,
    /// Scored phone numbers.
    pub phones: Vec<Scored<String>>,
    /// Scored fax number, if any.
    pub fax: Option<Scored<String>>,
    /// Scored postcode.
    pub postcode: Option<Scored<String>>,
    /// Scored city.
    pub city: Option<Scored<String>>,
    /// Scored street.
    pub street: Option<Scored<String>>,
    /// Scored Handelsregister number.
    pub hr_number: Option<Scored<String>>,
    /// Scored Handelsregister court.
    pub hr_court: Option<Scored<String>>,
    /// Scored USt-IdNr. (VAT ID).
    pub vat_id: Option<Scored<String>>,
    /// Scored Steuernummer.
    pub tax_number: Option<Scored<String>>,
    /// Scored IBAN.
    pub iban: Option<Scored<String>>,
    /// Scored BIC.
    pub bic: Option<Scored<String>>,
    /// Scored legal form.
    pub legal_form: Option<Scored<String>>,
    /// Scored founding year.
    pub year_founded: Option<Scored<i32>>,
    /// Scored persons.
    pub persons: Vec<Scored<Person>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scored_holds_value_and_confidence() {
        let s = Scored { value: "info@a.de".to_string(), confidence: 0.85 };
        assert_eq!(s.value, "info@a.de");
        assert!((s.confidence - 0.85).abs() < f32::EPSILON);
        let d = ScoredExtracted::default();
        assert!(d.emails.is_empty() && d.iban.is_none());
    }
}
