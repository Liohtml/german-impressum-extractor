//! Run with:  cargo run --example basic

use german_impressum_extractor::extract_all;

fn main() {
    let text = "
        Musterreinigung GmbH & Co. KG
        Geschäftsführer: Dr. Hans Müller und Anna Schmidt
        Hauptstraße 12
        10115 Berlin

        Tel.: +49 30 1234567
        Fax: +49 30 1234568
        E-Mail: info [at] musterreinigung [dot] de

        Eingetragen im Handelsregister Berlin HRB 12345 B
        USt-IdNr.: DE 123 456 789
        Steuernummer: 12/345/67890
        Bankverbindung: IBAN DE89 3704 0044 0532 0130 00  BIC COBADEFFXXX
        Gegründet 1985
    ";

    let d = extract_all(text);
    println!("{:#?}", d);
}
