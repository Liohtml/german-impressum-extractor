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
    // "De-Mail" must not match inside a larger compound word.
    assert_eq!(extract_de_mail("Absende-Mail: versand@firma.de"), None);
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

use german_impressum_extractor::extract_all;

#[test]
fn extract_all_includes_new_fields() {
    let text = "\
Muster GmbH
Handelsregister HRB 12345
Aufsichtsbehörde: Landesamt für Gesundheit
Zuständige Kammer: IHK Berlin
De-Mail: kontakt@firma.de-mail.de
Online-Streitbeilegung: https://ec.europa.eu/consumers/odr/";
    let d = extract_all(text);
    assert_eq!(d.register_type.as_deref(), Some("HRB"));
    assert_eq!(
        d.supervisory_authority.as_deref(),
        Some("Landesamt für Gesundheit")
    );
    assert_eq!(d.professional_chamber.as_deref(), Some("IHK Berlin"));
    assert_eq!(d.de_mail.as_deref(), Some("kontakt@firma.de-mail.de"));
    assert_eq!(
        d.dispute_resolution_url.as_deref(),
        Some("https://ec.europa.eu/consumers/odr/")
    );
    // Existing fields still work.
    assert_eq!(d.legal_form.as_deref(), Some("GmbH"));
    assert_eq!(d.hr_number.as_deref(), Some("HRB 12345"));
}

#[test]
fn label_regexes_ignore_mid_sentence_prose() {
    // "Berufskammer"/"Aufsichtsbehörde" mid-sentence (not starting a line) must
    // not be captured as a value.
    assert_eq!(
        extract_professional_chamber("Wir sind Mitglied der Berufskammer der Ärzte Bayern."),
        None
    );
    assert_eq!(
        extract_supervisory_authority("Diese Seite unterliegt der Aufsichtsbehörde des Landes."),
        None
    );
    // Legitimate line-start labels still work.
    assert_eq!(
        extract_professional_chamber("Berufskammer: Rechtsanwaltskammer Berlin"),
        Some("Rechtsanwaltskammer Berlin".into())
    );
    assert_eq!(
        extract_supervisory_authority("Aufsichtsbehörde: Landesamt X"),
        Some("Landesamt X".into())
    );
}
