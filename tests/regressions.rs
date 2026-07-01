//! Regression and feature tests covering the open issues addressed in this
//! change set. Each test references the issue number it guards.

use german_impressum_extractor::{
    extract_address, extract_all, extract_bic, extract_emails, extract_fax, extract_hr_court,
    extract_hr_number, extract_iban, extract_legal_form, extract_persons, extract_phones,
    extract_tax_number, extract_vat_id,
};

// ───────────────────────── Issue #1: Person.role ─────────────────────────

#[test]
fn role_geschaeftsfuehrer_is_detected() {
    let p = extract_persons("Geschäftsführer: Hans Müller");
    let m = p
        .iter()
        .find(|x| x.last_name.as_deref() == Some("Müller"))
        .unwrap();
    assert_eq!(m.role.as_deref(), Some("Geschäftsführer"));
}

#[test]
fn role_vorstand_is_detected() {
    let p = extract_persons("Vorstand: Anna Schmidt");
    let m = p
        .iter()
        .find(|x| x.last_name.as_deref() == Some("Schmidt"))
        .unwrap();
    assert_eq!(m.role.as_deref(), Some("Vorstand"));
}

#[test]
fn role_inhaber_is_detected() {
    let p = extract_persons("Inhaber: Peter Klein");
    let m = p
        .iter()
        .find(|x| x.last_name.as_deref() == Some("Klein"))
        .unwrap();
    assert_eq!(m.role.as_deref(), Some("Inhaber"));
}

// ───────────────────────── Issue #2: strip_titles ─────────────────────────

#[test]
fn names_containing_title_substrings_are_preserved() {
    // "Herrmann" must not become "mann", "Draxler" must not lose "Dr".
    let p = extract_persons("Geschäftsführer: Herrmann Draxler");
    assert!(
        p.iter().any(|x| x.last_name.as_deref() == Some("Draxler")),
        "got {p:?}"
    );
    assert!(
        p.iter()
            .any(|x| x.first_name.as_deref() == Some("Herrmann")),
        "got {p:?}"
    );
}

#[test]
fn real_titles_are_still_stripped() {
    let p = extract_persons("Geschäftsführer: Dr. Hans Müller");
    let m = p
        .iter()
        .find(|x| x.last_name.as_deref() == Some("Müller"))
        .unwrap();
    assert_eq!(m.first_name.as_deref(), Some("Hans"));
}

// ───────────────────────── Issue #3: email false positives ─────────────────

#[test]
fn ignores_code_fragment_emails() {
    let txt = "component-cart-notific@ion.css und main@bundle.js sind kein Kontakt";
    let e = extract_emails(txt);
    assert!(e.is_empty(), "code fragments leaked as emails: {e:?}");
}

#[test]
fn ignores_blocked_domains_but_keeps_real_emails() {
    let txt = "wh@sapp.com gest@tet.soweit aber info@firma.de ist echt";
    let e = extract_emails(txt);
    assert_eq!(e, vec!["info@firma.de".to_string()]);
}

// ───────────────────────── Issue #4: articles as names ─────────────────────

#[test]
fn articles_and_common_words_are_not_persons() {
    let p = extract_persons("vertreten durch die Gesellschafterin");
    assert!(p.is_empty(), "article/role word parsed as name: {p:?}");
}

#[test]
fn role_keyword_alone_yields_no_person() {
    let p = extract_persons("Der Geschäftsführer ist verantwortlich.");
    assert!(p.is_empty(), "got {p:?}");
}

// ───────────────────────── Issue #5: VAT with spaces ───────────────────────

#[test]
fn vat_id_with_internal_spaces() {
    assert_eq!(
        extract_vat_id("USt-IdNr: DE 123 456 789"),
        Some("DE123456789".into())
    );
    assert_eq!(extract_vat_id("DE123 456789"), Some("DE123456789".into()));
}

// ───────────────────────── Issue #9: IBAN / BIC ─────────────────────────

#[test]
fn iban_extracted_and_normalized() {
    assert_eq!(
        extract_iban("IBAN: DE89 3704 0044 0532 0130 00"),
        Some("DE89370400440532013000".into())
    );
}

#[test]
fn iban_does_not_pollute_vat_id() {
    let txt = "IBAN: DE89 3704 0044 0532 0130 00";
    assert_eq!(extract_vat_id(txt), None, "IBAN prefix wrongly read as VAT");
}

#[test]
fn iban_digits_do_not_leak_into_phones() {
    let txt = "Tel: +49 30 1234567\nIBAN: DE89 3704 0044 0532 0130 00";
    let d = extract_all(txt);
    assert_eq!(d.phones, vec!["+493012345 67".replace(' ', "")]);
}

#[test]
fn vat_and_iban_coexist() {
    let txt = "USt-IdNr.: DE123456789\nIBAN: DE89370400440532013000";
    let d = extract_all(txt);
    assert_eq!(d.vat_id.as_deref(), Some("DE123456789"));
    assert_eq!(d.iban.as_deref(), Some("DE89370400440532013000"));
}

#[test]
fn bic_requires_banking_context() {
    assert_eq!(extract_bic("BIC: COBADEFFXXX"), Some("COBADEFFXXX".into()));
    // No banking keyword nearby -> not picked up.
    assert_eq!(extract_bic("ABCDEFGH erscheint im Fließtext"), None);
}

// ───────────────────────── Issue #10: Fax separation ───────────────────────

#[test]
fn fax_is_separated_from_phones() {
    let txt = "Tel.: +49 30 1234567\nFax: +49 30 1234568";
    let d = extract_all(txt);
    assert_eq!(
        d.fax.as_deref(),
        Some("+493012345 68".replace(' ', "").as_str())
    );
    assert!(
        !d.phones.contains(&"+493012345 68".replace(' ', "")),
        "fax leaked into phones: {:?}",
        d.phones
    );
    assert!(
        d.phones
            .iter()
            .any(|p| p == "+493012345 67".replace(' ', "").as_str())
    );
}

#[test]
fn telefax_label_is_recognized() {
    assert_eq!(
        extract_fax("Telefax: +49 30 1234599"),
        Some("+49301234599".into())
    );
}

// ───────────────────────── Issue #11: Verantwortlicher ─────────────────────

#[test]
fn content_responsible_person_is_detected() {
    let p = extract_persons("Verantwortlich für den Inhalt: Anna Schmidt");
    let m = p
        .iter()
        .find(|x| x.last_name.as_deref() == Some("Schmidt"))
        .expect("Schmidt not found");
    assert_eq!(m.role.as_deref(), Some("Verantwortlich"));
}

#[test]
fn content_responsible_with_law_clause() {
    let p = extract_persons("Verantwortlich i.S.d. § 18 Abs. 2 MStV: Klaus Bauer");
    assert!(
        p.iter().any(|x| x.last_name.as_deref() == Some("Bauer")
            && x.role.as_deref() == Some("Verantwortlich")),
        "got {p:?}"
    );
}

// ───────────────────────── Smoke: full extraction ──────────────────────────

#[test]
fn full_impressum_smoke() {
    let text = "
        Musterreinigung GmbH & Co. KG
        Geschäftsführer: Dr. Hans Müller und Anna Schmidt
        Hauptstraße 12, 10115 Berlin
        Tel: +49 30 1234567
        Fax: +49 30 1234568
        E-Mail: info@musterreinigung.de
        Eingetragen im Handelsregister Berlin HRB 12345 B
        USt-IdNr.: DE 123 456 789
        IBAN: DE89 3704 0044 0532 0130 00
        BIC: COBADEFFXXX
        Gegründet 1985
    ";
    let d = extract_all(text);
    assert_eq!(d.legal_form.as_deref(), Some("GmbH & Co. KG"));
    assert_eq!(d.vat_id.as_deref(), Some("DE123456789"));
    assert_eq!(d.iban.as_deref(), Some("DE89370400440532013000"));
    assert_eq!(d.bic.as_deref(), Some("COBADEFFXXX"));
    assert_eq!(d.year_founded, Some(1985));
    assert!(d.fax.is_some());
    assert!(
        d.persons
            .iter()
            .any(|p| p.last_name.as_deref() == Some("Müller"))
    );
}

// ───────────── Issues #13 / #14 / #25: char-boundary panics ─────────────

#[test]
fn bic_with_multibyte_context_does_not_panic() {
    // Force the 40-byte context window preceding the BIC candidate to begin
    // inside a multi-byte character. 25 × 'ä' = 50 bytes; the candidate starts
    // at byte 55, so the window start (byte 15) lands mid-codepoint. Before the
    // fix this panicked with "byte index 15 is not a char boundary" (#13).
    let text = format!("{}!bic DEUTDEDB", "ä".repeat(25));
    assert_eq!(extract_bic(&text), Some("DEUTDEDB".into()));
}

#[test]
fn bic_zero_width_space_reproducer_does_not_panic() {
    // Exact reproducer from issue #13.
    let _ = extract_bic("Bankverbindung\u{200B}DEUTDEDB");
}

#[test]
fn persons_with_multibyte_control_chars_do_not_panic() {
    // Soft hyphen / non-breaking space near the name must not trigger a
    // mid-codepoint slice in truncate_at_sentence_end (#14).
    let p = extract_persons("Geschäftsführer: Hans Müller\u{00AD}\nSitz: Berlin");
    assert!(p.iter().any(|x| x.last_name.as_deref() == Some("Müller")));
    // Non-breaking space is captured by \s inside the name group.
    let _ = extract_persons("Geschäftsführer: Hans\u{00A0}Müller\u{00A0}Sitz Berlin");
}

// ───────────── Issue #15: natural-language words as TLDs ─────────────

#[test]
fn prose_words_are_not_accepted_as_tlds() {
    let txt = "th@matters.discover cre@ed.should info@link.missing gest@tet.siehe \
               aber info@firma.de ist echt";
    let e = extract_emails(txt);
    assert_eq!(e, vec!["info@firma.de".to_string()], "got {e:?}");
}

#[test]
fn common_real_tlds_are_kept() {
    for addr in [
        "kontakt@firma.de",
        "sales@company.com",
        "info@verein.org",
        "hello@startup.io",
        "kontakt@firma.at",
        "info@firma.ch",
    ] {
        let e = extract_emails(addr);
        assert_eq!(e, vec![addr.to_string()], "dropped valid address");
    }
}

// ───────────── Issue #26: HR court captures HR number prefix ─────────────

#[test]
fn hr_court_stops_before_hr_number() {
    let d = extract_all("Amtsgericht Berlin HRB 12345 B");
    assert_eq!(
        d.hr_court.as_deref(),
        Some("Berlin"),
        "got {:?}",
        d.hr_court
    );
    assert!(d.hr_number.is_some());
}

#[test]
fn hr_court_multiword_is_preserved() {
    assert_eq!(
        extract_hr_court("Amtsgericht Frankfurt am Main HRB 4321"),
        Some("Frankfurt am Main".into())
    );
    assert_eq!(
        extract_hr_court("Amtsgericht München"),
        Some("München".into())
    );
    // Separator form already worked; make sure it still does.
    assert_eq!(
        extract_hr_court("Amtsgericht Berlin (Charlottenburg), HRB 12345 B"),
        Some("Berlin (Charlottenburg)".into())
    );
}

// ───────────── Issue #30: KGaA legal form ─────────────

#[test]
fn kgaa_legal_forms_are_recognized() {
    assert_eq!(
        extract_legal_form("Henkel GmbH & Co. KGaA"),
        Some("GmbH & Co. KGaA".into())
    );
    assert_eq!(extract_legal_form("Merck KGaA"), Some("KGaA".into()));
    // Regression: plain GmbH & Co. KG must not be swallowed by the KGaA branch.
    assert_eq!(
        extract_legal_form("Muster GmbH & Co. KG"),
        Some("GmbH & Co. KG".into())
    );
}

// ───────────── Issue #31: public extract_tax_number ─────────────

#[test]
fn tax_number_is_publicly_extractable() {
    assert_eq!(
        extract_tax_number("Steuernummer: 28/815/0815 1"),
        Some("28/815/0815 1".into())
    );
}

// ───────────── Issue #32: Steuernummer abbreviations ─────────────

#[test]
fn tax_number_abbreviations() {
    assert!(extract_tax_number("Steuernummer: 28/815/0815 1").is_some());
    assert!(extract_tax_number("St.-Nr.: 28/815/0815 1").is_some());
    assert!(extract_tax_number("StNr. 28/815/0815 1").is_some());
    assert!(extract_tax_number("Steuer-Nr. 097/233/12345").is_some());
    assert!(extract_tax_number("St.Nr. 28 / 012 / 34567").is_some());
}

// ───────────── Issue #24: broaden coverage of critical extractors ─────────────

#[test]
fn person_with_title_and_middle_name() {
    let p = extract_persons("Geschäftsführer: Prof. Dr. Karl-Heinz von Müller");
    assert!(
        p.iter().any(|x| x.last_name.as_deref() == Some("Müller")),
        "got {p:?}"
    );
}

#[test]
fn bic_not_extracted_without_keyword_context() {
    assert_eq!(extract_bic("Ihre DEUTDEDB Referenz lautet XY."), None);
}

#[test]
fn phone_with_slash_format() {
    let phones = extract_phones("Telefon: 030/123 45 67");
    assert!(!phones.is_empty(), "no phone parsed");
    assert!(phones[0].starts_with("+49"), "got {phones:?}");
}

#[test]
fn address_postcode_before_city() {
    let (pc, city, _) = extract_address("10115 Berlin, Hauptstraße 1");
    assert_eq!(pc.as_deref(), Some("10115"));
    assert_eq!(city.as_deref(), Some("Berlin"));
}

#[test]
fn street_multiword_with_suffix() {
    let (_, _, street) = extract_address("Alte Poststraße 3, 45127 Essen");
    assert_eq!(street.as_deref(), Some("Alte Poststraße 3"));
}

#[test]
fn hr_number_hra_variant() {
    assert!(extract_hr_number("Handelsregister HRA 5678").is_some());
}
