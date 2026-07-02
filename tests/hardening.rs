use german_impressum_extractor::{extract_all, extract_emails, extract_fax, extract_persons};

// Messy input: NBSP (U+00A0), CRLF, a soft hyphen, and a well-formed entity.
const MESSY: &str = "Firma\u{00AD} GmbH\r\nTelefon:\u{00A0}+49 30 1234567\r\nFax: +49 30 1234568\r\nE-Mail: info&amp;#64;beispiel.de";

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
    let e = extract_emails("Mail:\u{00A0}info&amp;#64;beispiel.de");
    assert_eq!(e, vec!["info@beispiel.de".to_string()]);
}

#[test]
fn persons_still_extracted_after_normalization() {
    let p = extract_persons("Gesch\u{00E4}ftsf\u{00FC}hrer: Dr. Hans M\u{00FC}ller");
    assert!(p.iter().any(|x| x.last_name.as_deref() == Some("Müller")));
}
