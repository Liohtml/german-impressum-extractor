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
