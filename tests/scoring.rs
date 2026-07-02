use german_impressum_extractor::{extract_all, extract_all_scored};

const FULL: &str = "\
Musterreinigung GmbH & Co. KG
Geschäftsführer: Dr. Hans Müller
Hauptstraße 12, 10115 Berlin
Tel: +49 30 1234567
E-Mail: info@musterreinigung.de
Eingetragen im Handelsregister Berlin HRB 12345 B
USt-IdNr.: DE 123 456 789
IBAN: DE89 3704 0044 0532 0130 00
BIC: COBADEFFXXX
Gegründet 1985";

#[test]
fn scored_values_match_unscored_extraction() {
    let d = extract_all(FULL);
    let s = extract_all_scored(FULL);
    assert_eq!(s.iban.as_ref().map(|x| x.value.clone()), d.iban);
    assert_eq!(s.vat_id.as_ref().map(|x| x.value.clone()), d.vat_id);
    assert_eq!(s.postcode.as_ref().map(|x| x.value.clone()), d.postcode);
    assert_eq!(s.legal_form.as_ref().map(|x| x.value.clone()), d.legal_form);
    assert_eq!(s.year_founded.as_ref().map(|x| x.value), d.year_founded);
    assert_eq!(
        s.emails.iter().map(|x| x.value.clone()).collect::<Vec<_>>(),
        d.emails
    );
}

#[test]
fn confidences_are_in_range_and_iban_is_checksum_confident() {
    let s = extract_all_scored(FULL);
    for c in s
        .emails
        .iter()
        .map(|x| x.confidence)
        .chain(s.iban.iter().map(|x| x.confidence))
        .chain(s.vat_id.iter().map(|x| x.confidence))
    {
        assert!((0.0..=1.0).contains(&c), "confidence out of range: {c}");
    }
    // Valid IBAN (mod-97) + a Bank label ("IBAN:") present → high confidence.
    assert!(s.iban.unwrap().confidence >= 0.95);
}

#[test]
fn invalid_iban_scores_below_valid_iban() {
    let good = extract_all_scored("IBAN: DE89 3704 0044 0532 0130 00")
        .iban
        .unwrap();
    let bad = extract_all_scored("IBAN: DE89 3704 0044 0532 0130 01")
        .iban
        .unwrap();
    assert!(
        good.confidence > bad.confidence,
        "good={} bad={}",
        good.confidence,
        bad.confidence
    );
}

#[test]
fn label_presence_boosts_phone_confidence() {
    let labeled = extract_all_scored("Telefon: +49 30 1234567");
    let unlabeled = extract_all_scored("+49 30 1234567");
    let lc = labeled.phones.first().unwrap().confidence;
    let uc = unlabeled.phones.first().unwrap().confidence;
    assert!(lc > uc, "labeled {lc} should be > unlabeled {uc}");
}

#[cfg(feature = "serde")]
#[test]
fn scored_extracted_serde_roundtrips() {
    let s = extract_all_scored(FULL);
    let json = serde_json::to_string(&s).unwrap();
    let back: german_impressum_extractor::ScoredExtracted = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

#[cfg(feature = "html")]
#[test]
fn html_scored_matches_text_scored_fields() {
    use german_impressum_extractor::extract_all_scored_html;
    let s = extract_all_scored_html(
        "<p>Muster GmbH</p><dl><dt>USt-IdNr.</dt><dd>DE123456789</dd></dl>",
    );
    assert_eq!(s.legal_form.unwrap().value, "GmbH");
    assert_eq!(s.vat_id.unwrap().value, "DE123456789");
}
