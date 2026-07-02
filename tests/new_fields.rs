use german_impressum_extractor::{
    extract_de_mail, extract_dispute_resolution_url, extract_professional_chamber,
    extract_register_type, extract_supervisory_authority,
};

#[test]
fn supervisory_authority_labeled() {
    assert_eq!(
        extract_supervisory_authority("Aufsichtsbehörde: Landesärztekammer Hessen"),
        Some("Landesärztekammer Hessen".into())
    );
    assert_eq!(extract_supervisory_authority("Kein Hinweis hier"), None);
}

#[test]
fn professional_chamber_labeled() {
    assert_eq!(
        extract_professional_chamber("Zuständige Kammer: Rechtsanwaltskammer München"),
        Some("Rechtsanwaltskammer München".into())
    );
    assert_eq!(
        extract_professional_chamber("Berufskammer Steuerberaterkammer Berlin"),
        Some("Steuerberaterkammer Berlin".into())
    );
    assert_eq!(extract_professional_chamber("nichts davon"), None);
}

#[test]
fn de_mail_labeled_only() {
    assert_eq!(
        extract_de_mail("De-Mail: kontakt@firma.de-mail.de"),
        Some("kontakt@firma.de-mail.de".into())
    );
    // A normal e-mail label must NOT be picked up as De-Mail.
    assert_eq!(extract_de_mail("E-Mail: info@firma.de"), None);
}

#[test]
fn odr_url_detected() {
    assert_eq!(
        extract_dispute_resolution_url(
            "Plattform der EU zur OS: https://ec.europa.eu/consumers/odr/ — bitte beachten"
        ),
        Some("https://ec.europa.eu/consumers/odr/".into())
    );
    assert_eq!(extract_dispute_resolution_url("keine url hier"), None);
}

#[test]
fn register_type_from_hr() {
    assert_eq!(
        extract_register_type("Amtsgericht Berlin HRB 12345 B"),
        Some("HRB".into())
    );
    assert_eq!(
        extract_register_type("Handelsregister HRA 5678"),
        Some("HRA".into())
    );
    assert_eq!(extract_register_type("kein register hier"), None);
}
