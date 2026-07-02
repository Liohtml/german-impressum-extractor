use german_impressum_extractor::{extract_all, extract_emails, extract_fax, extract_persons};

#[test]
fn extract_all_decodes_entities_exactly_once() {
    // "info&amp;#64;beispiel.de" single-decodes to "info&#64;beispiel.de" (no '@'),
    // so NO email. A second (buggy) decode would produce "info@beispiel.de".
    // extract_all must normalize exactly once (would fail pre-fix; passes post-fix).
    let raw = "Kontakt: info&amp;#64;beispiel.de";
    let d = extract_all(raw);
    assert!(
        d.emails.is_empty(),
        "extract_all double-decoded the entity into an email: {:?}",
        d.emails
    );
    // The aggregate path must agree with the standalone extractor (both single-decode).
    assert_eq!(d.emails, extract_emails(raw));
}

// Messy input: NBSP (U+00A0), CRLF, a soft hyphen, and a well-formed entity.
const MESSY: &str = "Firma\u{00AD} GmbH\r\nTelefon:\u{00A0}+49 30 1234567\r\nFax: +49 30 1234568\r\nE-Mail: info&#64;beispiel.de";

#[test]
fn standalone_fax_and_emails_match_extract_all_on_messy_input() {
    // extract_all normalizes; after this task the standalone fns do too, so
    // these fields must agree on the SAME messy input. (Phones intentionally
    // not compared: extract_all removes the fax from `phones`, extract_phones
    // does not — that difference is by design, not a normalization gap.)
    let d = extract_all(MESSY);
    assert_eq!(extract_fax(MESSY), d.fax, "fax parity");
    assert_eq!(extract_emails(MESSY), d.emails, "email parity");
}

#[test]
fn standalone_email_decodes_entity_and_ignores_nbsp() {
    // &#64; is '@'; NBSP around the address must not break extraction.
    let e = extract_emails("Mail:\u{00A0}info&#64;beispiel.de");
    assert_eq!(e, vec!["info@beispiel.de".to_string()]);
}

#[test]
fn persons_still_extracted_after_normalization() {
    let p = extract_persons("Gesch\u{00E4}ftsf\u{00FC}hrer: Dr. Hans M\u{00FC}ller");
    assert!(p.iter().any(|x| x.last_name.as_deref() == Some("Müller")));
}

#[test]
fn persons_rejects_digit_tokens_and_noise_nouns() {
    // "Webdesign" is a common footer noise word; must not become a surname.
    let p = extract_persons("Inhaber: Webdesign Berlin");
    assert!(
        !p.iter().any(|x| x.last_name.as_deref() == Some("Webdesign")
            || x.first_name.as_deref() == Some("Webdesign")),
        "noise noun leaked as name: {p:?}"
    );

    // A token containing a digit is not a name part.
    let p2 = extract_persons("Geschäftsführer: Hans Müller2");
    assert!(
        !p2.iter().any(|x| x.last_name.as_deref() == Some("Müller2")),
        "digit-bearing token leaked as name: {p2:?}"
    );

    // Real name still works.
    let p3 = extract_persons("Geschäftsführer: Dr. Hans Müller");
    assert!(p3.iter().any(|x| x.last_name.as_deref() == Some("Müller")));
}
