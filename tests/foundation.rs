use german_impressum_extractor::{extract_address, extract_all};

#[test]
fn address_picks_the_block_where_street_and_postcode_coexist() {
    // Naive first-match would take the street from the first block and the
    // postcode from the second, producing a mixed, wrong address.
    let text = "\
Kontaktbüro
Musterweg 5

Hauptsitz
Beispielstraße 12
10115 Berlin";
    let (pc, city, street) = extract_address(text);
    assert_eq!(pc.as_deref(), Some("10115"));
    assert_eq!(city.as_deref(), Some("Berlin"));
    assert_eq!(street.as_deref(), Some("Beispielstraße 12"));
}

#[test]
fn single_block_address_unchanged() {
    let (pc, city, street) = extract_address("Hauptstraße 12, 10115 Berlin");
    assert_eq!(pc.as_deref(), Some("10115"));
    assert_eq!(city.as_deref(), Some("Berlin"));
    assert_eq!(street.as_deref(), Some("Hauptstraße 12"));
}

#[test]
fn extract_all_still_works_end_to_end() {
    let d = extract_all("Muster GmbH\nHauptstraße 12, 10115 Berlin\nUSt-IdNr.: DE123456789");
    assert_eq!(d.postcode.as_deref(), Some("10115"));
    assert_eq!(d.vat_id.as_deref(), Some("DE123456789"));
}

#[cfg(feature = "html")]
#[test]
fn html_extraction_matches_text_equivalent() {
    use german_impressum_extractor::extract_all_html;
    let html = "\
<h1>Muster GmbH</h1>
<p>Hauptstra&szlig;e 12, 10115 Berlin</p>
<dl><dt>USt-IdNr.</dt><dd>DE123456789</dd></dl>";
    let d = extract_all_html(html);
    assert_eq!(d.legal_form.as_deref(), Some("GmbH"));
    assert_eq!(d.postcode.as_deref(), Some("10115"));
    assert_eq!(d.city.as_deref(), Some("Berlin"));
    assert_eq!(d.street.as_deref(), Some("Hauptstraße 12"));
    assert_eq!(d.vat_id.as_deref(), Some("DE123456789"));
}
