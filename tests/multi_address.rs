use german_impressum_extractor::{Address, extract_address, extract_addresses};

#[test]
fn returns_one_address_per_block_in_order() {
    let text = "\
Standort Nord
Alpenstraße 1
80331 München

Standort Süd
Seeweg 7
79098 Freiburg";
    let addrs = extract_addresses(text);
    assert_eq!(addrs.len(), 2, "got {addrs:?}");
    assert_eq!(
        addrs[0],
        Address {
            postcode: Some("80331".into()),
            city: Some("München".into()),
            street: Some("Alpenstraße 1".into()),
        }
    );
    assert_eq!(
        addrs[1],
        Address {
            postcode: Some("79098".into()),
            city: Some("Freiburg".into()),
            street: Some("Seeweg 7".into()),
        }
    );
}

#[test]
fn single_address_matches_extract_address() {
    let text = "Hauptstraße 12, 10115 Berlin";
    let addrs = extract_addresses(text);
    assert_eq!(addrs.len(), 1);
    let (pc, city, street) = extract_address(text);
    assert_eq!(addrs[0].postcode, pc);
    assert_eq!(addrs[0].city, city);
    assert_eq!(addrs[0].street, street);
}

#[test]
fn partial_block_yields_partial_address() {
    // Only a street, no postcode/city.
    let addrs = extract_addresses("Nur Musterweg 5");
    assert_eq!(
        addrs,
        vec![Address {
            postcode: None,
            city: None,
            street: Some("Musterweg 5".into()),
        }]
    );
}

#[test]
fn identical_address_blocks_are_deduped() {
    let text = "Hauptstraße 1\n10115 Berlin\n\nHauptstraße 1\n10115 Berlin";
    let addrs = extract_addresses(text);
    assert_eq!(addrs.len(), 1, "duplicates not deduped: {addrs:?}");
}

#[test]
fn no_address_yields_empty() {
    assert!(extract_addresses("Kein Adressinhalt hier.").is_empty());
}

#[cfg(feature = "serde")]
#[test]
fn addresses_serde_roundtrip() {
    let a = extract_addresses("Hauptstraße 12, 10115 Berlin");
    let json = serde_json::to_string(&a).unwrap();
    let back: Vec<Address> = serde_json::from_str(&json).unwrap();
    assert_eq!(a, back);
}
