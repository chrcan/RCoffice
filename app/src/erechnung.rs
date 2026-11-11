//! E-Rechnung: erzeugt eine XRechnung (UBL 2.1, EN 16931, CIUS XRechnung 3.0) aus einer Rechnung.
//!
//! Hinweis: Die Datei folgt dem Aufbau der Norm, wurde aber nicht mit dem amtlichen Validator (KoSIT/Schematron)
//! geprueft. Vor dem produktiven Einsatz bitte mit einem Pruefdienst (z. B. KoSIT-Validator) kontrollieren.

use crate::db::{Adresse, Beleg, Db, Position};
use crate::util::*;
use std::collections::BTreeMap;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn betrag(v: f64) -> String {
    format!("{:.2}", round2(v))
}

fn laendercode(land: &str) -> String {
    let l = land.trim().to_lowercase();
    match l.as_str() {
        "" | "österreich" | "oesterreich" | "austria" | "at" => "AT",
        "deutschland" | "germany" | "de" => "DE",
        "schweiz" | "switzerland" | "ch" => "CH",
        "italien" | "italy" | "it" => "IT",
        "liechtenstein" | "li" => "LI",
        "frankreich" | "fr" => "FR",
        "niederlande" | "nl" => "NL",
        "ungarn" | "hu" => "HU",
        "tschechien" | "cz" => "CZ",
        "slowenien" | "si" => "SI",
        "polen" | "pl" => "PL",
        _ => "AT",
    }
    .to_string()
}

/// UN/ECE-Mengeneinheit aus der Einheit in der Position
pub fn einheit_code(e: &str) -> &'static str {
    match e.trim().to_lowercase().trim_end_matches('.') {
        "std" | "h" | "stunde" | "stunden" => "HUR",
        "kg" => "KGM",
        "g" => "GRM",
        "m" | "lfm" => "MTR",
        "m²" | "m2" | "qm" => "MTK",
        "l" | "liter" => "LTR",
        "tag" | "tage" | "tg" => "DAY",
        "monat" | "monate" | "mon" => "MON",
        "jahr" | "jahre" => "ANN",
        "km" => "KMT",
        _ => "C62",
    }
}

struct Partei {
    name: String,
    strasse: String,
    plz: String,
    ort: String,
    land: String,
}

/// Kunde aus Adressstamm, ersatzweise aus dem Adressfeld des Belegs (letzte Zeile "PLZ Ort").
fn kunde(db: &Db, b: &Beleg) -> Partei {
    if let Some(a) = db.adresse(&b.adresse_id) {
        let name = if !a.firma.trim().is_empty() { a.firma.split_whitespace().collect::<Vec<_>>().join(" ") } else { a.name() };
        return partei_aus(&a, name);
    }
    let zeilen: Vec<&str> = b.adressfeld.lines().map(|z| z.trim()).filter(|z| !z.is_empty()).collect();
    let name = zeilen.first().copied().unwrap_or("").to_string();
    let mut p = Partei { name, strasse: String::new(), plz: String::new(), ort: String::new(), land: "AT".into() };
    for z in zeilen.iter().skip(1) {
        let t = z.trim_start_matches("AT-").trim_start_matches("DE-");
        let (kopf, rest) = t.split_once(' ').unwrap_or((t, ""));
        if kopf.len() >= 4 && kopf.len() <= 5 && kopf.chars().all(|c| c.is_ascii_digit()) {
            p.plz = kopf.to_string();
            p.ort = rest.trim().to_string();
        } else if p.strasse.is_empty() && z.chars().any(|c| c.is_ascii_digit()) {
            p.strasse = z.to_string();
        }
    }
    p
}

fn partei_aus(a: &Adresse, name: String) -> Partei {
    Partei { name, strasse: a.strasse.trim().to_string(), plz: a.plz.trim().to_string(), ort: a.ort.trim().to_string(), land: laendercode(&a.land) }
}

/// Erzeugt die XRechnung als XML-Text. Gibt eine Liste fehlender Pflichtangaben als Fehlertext zurueck.
pub fn xrechnung_xml(db: &Db, b: &Beleg, pos: &[Position]) -> Result<String, String> {
    if b.typ != "Rechnung" {
        return Err("Eine E-Rechnung kann nur aus einer Rechnung erstellt werden.".into());
    }
    let cfg = db.alle_einstellungen();
    let c = |k: &str| cfg.get(k).map(|v| v.trim().to_string()).unwrap_or_default();
    let verk = Partei { name: c("Firma"), strasse: c("Firma_Strasse"), plz: c("Firma_PLZ"), ort: c("Firma_Ort"), land: laendercode(&c("Firma_Standort")) };
    let kauf = kunde(db, b);
    let uid = c("Firma_UStID").replace(' ', "").to_uppercase();
    let iban = c("Firma_IBAN").replace(' ', "").to_uppercase();
    let bic = c("Firma_BIC").replace(' ', "").to_uppercase();

    // ---- Pflichtangaben pruefen
    let mut fehlt: Vec<&str> = Vec::new();
    if verk.name.is_empty() { fehlt.push("Firmenname (Einstellungen)"); }
    if verk.strasse.is_empty() || verk.plz.is_empty() || verk.ort.is_empty() { fehlt.push("Firmenanschrift (Einstellungen)"); }
    if uid.is_empty() { fehlt.push("UID-Nummer (Einstellungen)"); }
    if iban.is_empty() { fehlt.push("IBAN (Einstellungen)"); }
    if c("Firma_Email").is_empty() { fehlt.push("Firmen-E-Mail (Einstellungen)"); }
    if kauf.name.is_empty() { fehlt.push("Kundenname"); }
    if kauf.ort.is_empty() { fehlt.push("Ort des Kunden"); }
    if pos.is_empty() { fehlt.push("mindestens eine Position"); }
    if b.datum.is_empty() { fehlt.push("Rechnungsdatum"); }
    if !fehlt.is_empty() {
        return Err(format!("Für die E-Rechnung fehlen Angaben: {}.", fehlt.join(", ")));
    }

    let waehrung = match b.waehrung.trim() { "" | "€" => "EUR".to_string(), w => w.to_uppercase() };
    let faellig = add_days_iso(&b.datum, 14);

    // ---- Positionen und Belegrabatt je Steuersatz (Rabatt wird anteilig auf die Saetze verteilt)
    let netto_pos: f64 = pos.iter().map(|p| p.summe).sum();
    let rabatt = round2(b.rabatt_betrag);
    let mut je_satz: BTreeMap<i64, f64> = BTreeMap::new(); // Satz in 1/100 % -> Summe
    for p in pos {
        *je_satz.entry((p.mwst * 100.0).round() as i64).or_insert(0.0) += p.summe;
    }
    let mut abzug: BTreeMap<i64, f64> = BTreeMap::new();
    if rabatt != 0.0 && netto_pos.abs() > 1e-9 {
        let mut rest = rabatt;
        let n = je_satz.len();
        for (i, (satz, summe)) in je_satz.iter().enumerate() {
            let a = if i + 1 == n { round2(rest) } else { round2(rabatt * summe / netto_pos) };
            rest -= a;
            abzug.insert(*satz, a);
        }
    }
    let mut steuer: Vec<(i64, f64, f64)> = Vec::new(); // (Satz, Bemessung, Steuer)
    for (satz, summe) in &je_satz {
        let bem = round2(summe - abzug.get(satz).copied().unwrap_or(0.0));
        steuer.push((*satz, bem, round2(bem * (*satz as f64 / 100.0) / 100.0)));
    }
    let netto = round2(netto_pos - rabatt);
    let steuer_sum = round2(steuer.iter().map(|s| s.2).sum());
    let brutto = round2(netto + steuer_sum);
    let offen = round2(brutto - b.bezahlt);
    let kat = |satz: i64| if satz == 0 { "Z" } else { "S" };
    let pct = |satz: i64| format!("{}", satz as f64 / 100.0);

    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<ubl:Invoice xmlns:ubl=\"urn:oasis:names:specification:ubl:schema:xsd:Invoice-2\" xmlns:cac=\"urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2\" xmlns:cbc=\"urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2\">\n");
    x.push_str("  <cbc:CustomizationID>urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0</cbc:CustomizationID>\n");
    x.push_str("  <cbc:ProfileID>urn:fdc:peppol.eu:2017:poacc:billing:01:1.0</cbc:ProfileID>\n");
    x.push_str(&format!("  <cbc:ID>{}</cbc:ID>\n", b.nr));
    x.push_str(&format!("  <cbc:IssueDate>{}</cbc:IssueDate>\n", b.datum));
    x.push_str(&format!("  <cbc:DueDate>{}</cbc:DueDate>\n", faellig));
    x.push_str("  <cbc:InvoiceTypeCode>380</cbc:InvoiceTypeCode>\n");
    if !b.betreff.trim().is_empty() {
        x.push_str(&format!("  <cbc:Note>{}</cbc:Note>\n", esc(b.betreff.trim())));
    }
    x.push_str(&format!("  <cbc:DocumentCurrencyCode>{}</cbc:DocumentCurrencyCode>\n", waehrung));
    x.push_str(&format!("  <cbc:BuyerReference>{}</cbc:BuyerReference>\n", esc(if b.kundennr.is_empty() { "n/a" } else { &b.kundennr })));

    // Verkaeufer
    x.push_str("  <cac:AccountingSupplierParty>\n    <cac:Party>\n");
    if !c("Firma_Email").is_empty() {
        x.push_str(&format!("      <cbc:EndpointID schemeID=\"EM\">{}</cbc:EndpointID>\n", esc(&c("Firma_Email"))));
    }
    x.push_str(&format!("      <cac:PartyName><cbc:Name>{}</cbc:Name></cac:PartyName>\n", esc(&verk.name)));
    x.push_str(&format!("      <cac:PostalAddress>\n        <cbc:StreetName>{}</cbc:StreetName>\n        <cbc:CityName>{}</cbc:CityName>\n        <cbc:PostalZone>{}</cbc:PostalZone>\n        <cac:Country><cbc:IdentificationCode>{}</cbc:IdentificationCode></cac:Country>\n      </cac:PostalAddress>\n", esc(&verk.strasse), esc(&verk.ort), esc(&verk.plz), verk.land));
    x.push_str(&format!("      <cac:PartyTaxScheme>\n        <cbc:CompanyID>{}</cbc:CompanyID>\n        <cac:TaxScheme><cbc:ID>VAT</cbc:ID></cac:TaxScheme>\n      </cac:PartyTaxScheme>\n", esc(&uid)));
    x.push_str(&format!("      <cac:PartyLegalEntity><cbc:RegistrationName>{}</cbc:RegistrationName></cac:PartyLegalEntity>\n", esc(&verk.name)));
    x.push_str(&format!("      <cac:Contact>\n        <cbc:Name>{}</cbc:Name>\n        <cbc:Telephone>{}</cbc:Telephone>\n        <cbc:ElectronicMail>{}</cbc:ElectronicMail>\n      </cac:Contact>\n", esc(&verk.name), esc(&c("Firma_Telefon")), esc(&c("Firma_Email"))));
    x.push_str("    </cac:Party>\n  </cac:AccountingSupplierParty>\n");

    // Kaeufer
    x.push_str("  <cac:AccountingCustomerParty>\n    <cac:Party>\n");
    if let Some(a) = db.adresse(&b.adresse_id) {
        if !a.email.trim().is_empty() {
            x.push_str(&format!("      <cbc:EndpointID schemeID=\"EM\">{}</cbc:EndpointID>\n", esc(a.email.trim())));
        }
    }
    x.push_str(&format!("      <cac:PartyName><cbc:Name>{}</cbc:Name></cac:PartyName>\n", esc(&kauf.name)));
    x.push_str(&format!("      <cac:PostalAddress>\n        <cbc:StreetName>{}</cbc:StreetName>\n        <cbc:CityName>{}</cbc:CityName>\n        <cbc:PostalZone>{}</cbc:PostalZone>\n        <cac:Country><cbc:IdentificationCode>{}</cbc:IdentificationCode></cac:Country>\n      </cac:PostalAddress>\n", esc(&kauf.strasse), esc(&kauf.ort), esc(&kauf.plz), kauf.land));
    x.push_str(&format!("      <cac:PartyLegalEntity><cbc:RegistrationName>{}</cbc:RegistrationName></cac:PartyLegalEntity>\n", esc(&kauf.name)));
    x.push_str("    </cac:Party>\n  </cac:AccountingCustomerParty>\n");

    // Zahlung
    x.push_str("  <cac:PaymentMeans>\n    <cbc:PaymentMeansCode>58</cbc:PaymentMeansCode>\n");
    x.push_str(&format!("    <cbc:PaymentID>{}</cbc:PaymentID>\n", b.nr));
    x.push_str(&format!("    <cac:PayeeFinancialAccount>\n      <cbc:ID>{}</cbc:ID>\n", esc(&iban)));
    x.push_str(&format!("      <cbc:Name>{}</cbc:Name>\n", esc(&verk.name)));
    if !bic.is_empty() {
        x.push_str(&format!("      <cac:FinancialInstitutionBranch><cbc:ID>{}</cbc:ID></cac:FinancialInstitutionBranch>\n", esc(&bic)));
    }
    x.push_str("    </cac:PayeeFinancialAccount>\n  </cac:PaymentMeans>\n");
    let bed = c("Zahlungsbedingung").replace('\n', " ");
    if !bed.is_empty() {
        x.push_str(&format!("  <cac:PaymentTerms><cbc:Note>{}</cbc:Note></cac:PaymentTerms>\n", esc(&bed)));
    }

    // Belegrabatt
    for (satz, a) in &abzug {
        if *a != 0.0 {
            x.push_str(&format!(
                "  <cac:AllowanceCharge>\n    <cbc:ChargeIndicator>false</cbc:ChargeIndicator>\n    <cbc:AllowanceChargeReason>{}</cbc:AllowanceChargeReason>\n    <cbc:Amount currencyID=\"{}\">{}</cbc:Amount>\n    <cac:TaxCategory><cbc:ID>{}</cbc:ID><cbc:Percent>{}</cbc:Percent><cac:TaxScheme><cbc:ID>VAT</cbc:ID></cac:TaxScheme></cac:TaxCategory>\n  </cac:AllowanceCharge>\n",
                esc(if b.rabatt_text.trim().is_empty() { "Rabatt" } else { b.rabatt_text.trim() }), waehrung, betrag(*a), kat(*satz), pct(*satz)
            ));
        }
    }

    // Steuer
    x.push_str(&format!("  <cac:TaxTotal>\n    <cbc:TaxAmount currencyID=\"{}\">{}</cbc:TaxAmount>\n", waehrung, betrag(steuer_sum)));
    for (satz, bem, st) in &steuer {
        x.push_str(&format!(
            "    <cac:TaxSubtotal>\n      <cbc:TaxableAmount currencyID=\"{w}\">{}</cbc:TaxableAmount>\n      <cbc:TaxAmount currencyID=\"{w}\">{}</cbc:TaxAmount>\n      <cac:TaxCategory><cbc:ID>{}</cbc:ID><cbc:Percent>{}</cbc:Percent><cac:TaxScheme><cbc:ID>VAT</cbc:ID></cac:TaxScheme></cac:TaxCategory>\n    </cac:TaxSubtotal>\n",
            betrag(*bem), betrag(*st), kat(*satz), pct(*satz), w = waehrung
        ));
    }
    x.push_str("  </cac:TaxTotal>\n");
    x.push_str(&format!(
        "  <cac:LegalMonetaryTotal>\n    <cbc:LineExtensionAmount currencyID=\"{w}\">{}</cbc:LineExtensionAmount>\n    <cbc:TaxExclusiveAmount currencyID=\"{w}\">{}</cbc:TaxExclusiveAmount>\n    <cbc:TaxInclusiveAmount currencyID=\"{w}\">{}</cbc:TaxInclusiveAmount>\n    <cbc:AllowanceTotalAmount currencyID=\"{w}\">{}</cbc:AllowanceTotalAmount>\n    <cbc:PrepaidAmount currencyID=\"{w}\">{}</cbc:PrepaidAmount>\n    <cbc:PayableAmount currencyID=\"{w}\">{}</cbc:PayableAmount>\n  </cac:LegalMonetaryTotal>\n",
        betrag(netto_pos), betrag(netto), betrag(brutto), betrag(rabatt), betrag(b.bezahlt), betrag(offen), w = waehrung
    ));

    // Positionen
    for (i, p) in pos.iter().enumerate() {
        let menge = if p.menge == 0.0 { 1.0 } else { p.menge };
        let preis = if (round2(menge * p.vk) - round2(p.summe)).abs() < 0.005 { p.vk } else { p.summe / menge };
        let name = if p.artikel.trim().is_empty() { format!("Position {}", i + 1) } else { p.artikel.split_whitespace().collect::<Vec<_>>().join(" ") };
        let satz = (p.mwst * 100.0).round() as i64;
        x.push_str(&format!("  <cac:InvoiceLine>\n    <cbc:ID>{}</cbc:ID>\n", i + 1));
        x.push_str(&format!("    <cbc:InvoicedQuantity unitCode=\"{}\">{}</cbc:InvoicedQuantity>\n", einheit_code(&p.einheit), menge));
        x.push_str(&format!("    <cbc:LineExtensionAmount currencyID=\"{}\">{}</cbc:LineExtensionAmount>\n", waehrung, betrag(p.summe)));
        x.push_str("    <cac:Item>\n");
        if !p.beschreibung.trim().is_empty() {
            x.push_str(&format!("      <cbc:Description>{}</cbc:Description>\n", esc(p.beschreibung.trim())));
        }
        x.push_str(&format!("      <cbc:Name>{}</cbc:Name>\n", esc(&name)));
        x.push_str(&format!("      <cac:ClassifiedTaxCategory><cbc:ID>{}</cbc:ID><cbc:Percent>{}</cbc:Percent><cac:TaxScheme><cbc:ID>VAT</cbc:ID></cac:TaxScheme></cac:ClassifiedTaxCategory>\n    </cac:Item>\n", kat(satz), pct(satz)));
        x.push_str(&format!("    <cac:Price><cbc:PriceAmount currencyID=\"{}\">{}</cbc:PriceAmount></cac:Price>\n  </cac:InvoiceLine>\n", waehrung, format!("{:.4}", preis).trim_end_matches('0').trim_end_matches('.')));
    }
    x.push_str("</ubl:Invoice>\n");
    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_mit_rechnung(rabatt: f64) -> (Db, Beleg, Vec<Position>) {
        let mut db = Db::memory().unwrap();
        for (k, v) in [("Firma", "Test & Co"), ("Firma_Strasse", "Weg 1"), ("Firma_PLZ", "5724"), ("Firma_Ort", "Stuhlfelden"), ("Firma_UStID", "ATU 12345678"), ("Firma_IBAN", "AT61 1904 3002 3457 3201"), ("Firma_Email", "a@b.at"), ("Firma_Standort", "Österreich")] {
            db.set(k, v);
        }
        let a = Adresse { id: "A".into(), firma: "Kunde <GmbH>".into(), strasse: "Str. 2".into(), plz: "5730".into(), ort: "Mittersill".into(), land: "Österreich".into(), kundennr: "9".into(), ..Default::default() };
        db.adresse_speichern(&a).unwrap();
        let mut b = db.neuer_beleg("Rechnung", Some(&a));
        b.rabatt_betrag = rabatt;
        let mut p1 = Position { artikel: "Logo".into(), menge: 2.0, vk: 100.0, mwst: 20.0, einheit: "Std.".into(), ..Default::default() };
        p1.berechnen();
        let mut p2 = Position { artikel: "Buch".into(), menge: 1.0, vk: 50.0, mwst: 10.0, einheit: "Stk.".into(), ..Default::default() };
        p2.berechnen();
        let pos = vec![p1, p2];
        db.beleg_speichern(&b, &pos).unwrap();
        (db, b, pos)
    }

    #[test]
    fn xml_ohne_rabatt() {
        let (db, b, pos) = db_mit_rechnung(0.0);
        let x = xrechnung_xml(&db, &b, &pos).unwrap();
        assert!(x.contains("Test &amp; Co") && x.contains("Kunde &lt;GmbH&gt;"));
        assert!(x.contains("unitCode=\"HUR\"") && x.contains("unitCode=\"C62\""));
        assert!(x.contains("<cbc:TaxAmount currencyID=\"EUR\">45.00</cbc:TaxAmount>"), "20% von 200 + 10% von 50");
        assert!(x.contains("<cbc:PayableAmount currencyID=\"EUR\">295.00</cbc:PayableAmount>"));
        assert!(x.contains("ATU12345678"));
    }

    #[test]
    fn xml_mit_rabatt_verteilt_auf_saetze() {
        let (db, b, pos) = db_mit_rechnung(25.0);
        let x = xrechnung_xml(&db, &b, &pos).unwrap();
        // Rabatt 25 auf 250: 20 % -> 20,00 ; 10 % -> 5,00 ; Netto 225
        assert!(x.contains("<cbc:TaxExclusiveAmount currencyID=\"EUR\">225.00</cbc:TaxExclusiveAmount>"));
        assert!(x.contains("<cbc:AllowanceTotalAmount currencyID=\"EUR\">25.00</cbc:AllowanceTotalAmount>"));
        assert!(x.contains("<cbc:TaxableAmount currencyID=\"EUR\">180.00</cbc:TaxableAmount>"));
    }

    #[test]
    fn fehlende_pflichtangaben() {
        let db = Db::memory().unwrap();
        let b = db.neuer_beleg("Rechnung", None);
        let e = xrechnung_xml(&db, &b, &[]).unwrap_err();
        assert!(e.contains("UID-Nummer") && e.contains("IBAN") && e.contains("Position"));
        let ang = db.neuer_beleg("Angebot", None);
        assert!(xrechnung_xml(&db, &ang, &[]).is_err());
    }

    #[test]
    fn einheiten() {
        assert_eq!(einheit_code("Std."), "HUR");
        assert_eq!(einheit_code("Stck."), "C62");
        assert_eq!(einheit_code("m²"), "MTK");
    }
}
