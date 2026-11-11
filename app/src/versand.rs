//! E-Mail-Versand ueber das Standard-Mailprogramm (wie in AZURO: "Emailversand = E-Mail Programm").
//! Die App erstellt das PDF, oeffnet eine neue Nachricht mit Empfaenger, Betreff und Text und zeigt das PDF im
//! Dateimanager an, damit es nur noch in die Mail gezogen werden muss (ueber mailto: sind keine Anhaenge moeglich).

use crate::db::{Beleg, Db};
use std::path::Path;

/// Prozent-Kodierung fuer mailto:-Adressen (UTF-8, alles ausser A-Z a-z 0-9 - _ . ~ wird kodiert)
pub fn url_kodieren(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Betreff und Text der Mail aus den AZURO-Textvorlagen (Platzhalter ABNR, AUFNR, RGNR, BENUTZER).
pub fn mailtext(db: &Db, b: &Beleg) -> (String, String) {
    let (key, platzhalter) = match b.typ.as_str() {
        "Angebot" => ("Text Email Angebot", "ABNR"),
        "Auftrag" => ("Text Email Auftrag", "AUFNR"),
        "Lieferschein" => ("Text Email Lieferschein", "LFSNR"),
        _ => ("Text Email Rechnung", "RGNR"),
    };
    let firma = db.get("Firma");
    let mut text = db.get(key);
    if text.trim().is_empty() {
        text = format!("anbei erhalten Sie {} Nr. {}\n\nMit freundlichen Grüßen\n\n{}", b.typ, platzhalter, firma);
    }
    let text = text.replace(platzhalter, &b.nr.to_string()).replace("BENUTZER", &firma);
    let anrede = if b.briefanrede.trim().is_empty() { String::new() } else { format!("{}\n\n", b.briefanrede.trim()) };
    let betreff = format!("{} Nr. {}{}", b.typ, b.nr, if firma.is_empty() { String::new() } else { format!(" – {}", firma) });
    (betreff, format!("{}{}", anrede, text.replace("\r\n", "\n")))
}

pub fn mailto_url(an: &str, betreff: &str, text: &str) -> String {
    format!("mailto:{}?subject={}&body={}", url_kodieren(an.trim()).replace("%40", "@"), url_kodieren(betreff), url_kodieren(text))
}

/// Oeffnet eine mailto:-Adresse im Standard-Mailprogramm.
pub fn mailprogramm_oeffnen(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// Zeigt eine Datei im Dateimanager markiert an.
pub fn datei_anzeigen(pfad: &Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(format!("/select,{}", pfad.display())).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(pfad).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(pfad.parent().unwrap_or(pfad)).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kodierung() {
        assert_eq!(url_kodieren("a b&c"), "a%20b%26c");
        assert_eq!(url_kodieren("Grüße"), "Gr%C3%BC%C3%9Fe");
        assert_eq!(url_kodieren("x@y.at"), "x%40y.at");
    }

    #[test]
    fn text_aus_vorlage() {
        let db = Db::memory().unwrap();
        db.set("Firma", "rc design");
        db.set("Text Email Rechnung", "anbei die Rechnung Nr. RGNR\n\nGruß\nBENUTZER");
        let mut b = db.neuer_beleg("Rechnung", None);
        b.briefanrede = "Sehr geehrter Herr X,".into();
        let (betreff, text) = mailtext(&db, &b);
        assert_eq!(betreff, format!("Rechnung Nr. {} – rc design", b.nr));
        assert!(text.starts_with("Sehr geehrter Herr X,\n\nanbei die Rechnung Nr. 1"));
        assert!(text.ends_with("Gruß\nrc design"));
        assert!(mailto_url("a@b.at", "S", "T x").starts_with("mailto:a@b.at?subject=S&body=T%20x"));
    }
}
