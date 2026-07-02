use german_impressum_extractor::{
    extract_de_mail, extract_professional_chamber, extract_supervisory_authority,
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
