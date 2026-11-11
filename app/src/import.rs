//! Import der aus AZURO/FileMaker exportierten Tabellen (JSON-Lines, siehe _tools/fmp2jsonl).
//! Liest Adressen, Artikel, Angebote, Auftraege, Lieferscheine, Rechnungen, Positionen, Standardtexte
//! und die Firmen-Einstellungen. Bestehende Daten werden ersetzt.

use crate::calc::summen;
use crate::db::{Adresse, Artikel, Beleg, Db, Position};
use crate::util::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

type Row = HashMap<String, String>;

fn lese(dir: &Path, tabelle: &str) -> Result<Vec<Row>, String> {
    let p = dir.join(format!("{}.jsonl", tabelle));
    let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {}", p.display(), e))?;
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue; // Zeile 1 = Spaltenbeschreibung
        }
        let v: Value = serde_json::from_str(line).map_err(|e| format!("{} Zeile {}: {}", p.display(), i + 1, e))?;
        if let Value::Object(m) = v {
            rows.push(m.into_iter().map(|(k, v)| (k, v.as_str().unwrap_or("").to_string())).collect());
        }
    }
    Ok(rows)
}

fn g(r: &Row, k: &str) -> String {
    r.get(k).map(|v| clean_text(v)).unwrap_or_default()
}
fn n(r: &Row, k: &str) -> f64 {
    r.get(k).map(|v| parse_num(v)).unwrap_or(0.0)
}
fn d(r: &Row, k: &str) -> String {
    r.get(k).map(|v| date_iso(v)).unwrap_or_default()
}
fn erster<'a>(r: &Row, keys: &[&'a str]) -> String {
    for k in keys {
        let v = g(r, k);
        if !v.is_empty() {
            return v;
        }
    }
    String::new()
}

#[derive(Default, Debug)]
pub struct Bericht {
    pub adressen: usize,
    pub artikel: usize,
    pub belege: HashMap<String, usize>,
    pub positionen: usize,
    pub einstellungen: usize,
    pub rechnungen_geprueft: usize,
    pub rechnungen_abweichend: usize,
    pub waisen_positionen: usize,
}

impl Bericht {
    pub fn text(&self) -> String {
        let mut t = format!("{} Adressen, {} Artikel", self.adressen, self.artikel);
        for typ in crate::db::TYPEN {
            t += &format!(", {} {}", self.belege.get(typ).copied().unwrap_or(0), typ);
        }
        t += &format!(", {} Positionen, {} Einstellungen. ", self.positionen, self.einstellungen);
        t += &format!("Kontrolle: {} von {} Rechnungen stimmen auf den Cent mit AZURO überein.", self.rechnungen_geprueft - self.rechnungen_abweichend, self.rechnungen_geprueft);
        if self.waisen_positionen > 0 {
            t += &format!(" {} Positionen ohne Beleg wurden ausgelassen.", self.waisen_positionen);
        }
        t
    }
}

pub fn importieren(db: &mut Db, dir: &Path) -> Result<Bericht, String> {
    let adressen = lese(dir, "Adressen")?;
    let artikel = lese(dir, "Artikel")?;
    let positionen = lese(dir, "Position")?;
    let global = lese(dir, "Global").unwrap_or_default();
    let standardtexte = lese(dir, "Standardtexte").unwrap_or_default();
    let mut bericht = Bericht::default();

    let tx = db.conn.transaction().map_err(|e| e.to_string())?;
    for t in ["adresse", "artikel", "beleg", "position", "standardtext", "einstellung"] {
        tx.execute(&format!("DELETE FROM {}", t), []).map_err(|e| e.to_string())?;
    }

    // ----- Adressen
    for r in &adressen {
        let a = adresse_aus_row(r);
        if a.id.is_empty() {
            continue;
        }
        a.save(&tx).map_err(|e| e.to_string())?;
        bericht.adressen += 1;
    }

    // ----- Artikel
    for r in &artikel {
        let a = Artikel {
            id: g(r, "ID"),
            nr: g(r, "Artikelnr."),
            name: g(r, "Artikel"),
            beschreibung: erster(r, &["Artikelbeschreibung", "Artikelbezeichnung"]),
            gruppe: g(r, "Artikelgruppe"),
            einheit: g(r, "Einheit"),
            art: g(r, "Produktart"),
            ek: n(r, "EK"),
            vk: n(r, "VK"),
            mwst: n(r, "MwSt Satz"),
        };
        let a = Artikel { id: if a.id.is_empty() { format!("ART{}", a.nr) } else { a.id.clone() }, ..a };
        tx.execute(
            "INSERT OR REPLACE INTO artikel(id,nr,name,beschreibung,gruppe,einheit,art,ek,vk,mwst) VALUES(?,?,?,?,?,?,?,?,?,?)",
            rusqlite::params![a.id, a.nr, a.name, a.beschreibung, a.gruppe, a.einheit, a.art, a.ek, a.vk, a.mwst],
        )
        .map_err(|e| e.to_string())?;
        bericht.artikel += 1;
    }

    // ----- Belege
    let mut alle_belege: Vec<Beleg> = Vec::new();
    let mut kontrolle: HashMap<String, (f64, f64)> = HashMap::new();
    for (typ, tabelle, nrfeld) in [("Angebot", "Angebot", "Angebots Nr."), ("Auftrag", "Auftrag", "Auftrags Nr."), ("Lieferschein", "Lieferschein", "Lieferschein Nr."), ("Rechnung", "Rechnung", "Rechnungs Nr.")] {
        for r in lese(dir, tabelle)? {
            let b = Beleg {
                id: g(&r, "ID"),
                typ: typ.into(),
                nr: n(&r, nrfeld) as i64,
                datum: d(&r, "Datum"),
                adresse_id: g(&r, "ID_Adresse"),
                kundennr: g(&r, "Kundennummer"),
                adressfeld: g(&r, "Adressfeld"),
                briefanrede: g(&r, "Anrede"),
                betreff: g(&r, "Betreffzeile"),
                kopftext: g(&r, "Kopf Text"),
                fusstext: g(&r, "Fußtext"),
                status: g(&r, "Status"),
                rabatt_prozent: n(&r, "Rabatt"),
                rabatt_betrag: n(&r, "Rabattbetrag"),
                rabatt_text: g(&r, "Rabatttext"),
                bezahlt: n(&r, "Betrag bezahlt"),
                zahldatum: d(&r, "Zahldatum"),
                mahnstufe: n(&r, "Mahnstufe") as i64,
                mahn1: d(&r, "Datum 1. Mahn."),
                mahn2: d(&r, "Datum 2. Mahn."),
                mahn3: d(&r, "Datum 3. Mahn."),
                leistungsdatum: d(&r, "Leistungsdatum"),
                gueltig_bis: d(&r, "gültig bis"),
                aus_angebot: g(&r, "aus Angebot"),
                aus_auftrag: g(&r, "aus Auftrag"),
                aus_lieferschein: g(&r, "aus Liefersch."),
                zu_auftrag: g(&r, "wurde zu Auftr."),
                zu_lieferschein: g(&r, "wurde zu Liefersch."),
                zu_rechnung: g(&r, "wurde zu RG"),
                waehrung: g(&r, "Währung"),
                sprache: g(&r, "Sprache"),
                erstellt: d(&r, "Erstell.Datum"),
                bearbeiter: g(&r, "Bearbeiterkürzel"),
                notizen: g(&r, "Notizen"),
            };
            if b.id.is_empty() {
                continue;
            }
            b.save(&tx).map_err(|e| e.to_string())?;
            *bericht.belege.entry(typ.to_string()).or_insert(0) += 1;
            // Kontrollwerte aus AZURO fuer die Rechnungspruefung
            if typ == "Rechnung" {
                alle_belege.push(b.clone());
                kontrolle.insert(b.id.clone(), (n(&r, "Endpreis netto"), n(&r, "Endpreis brutto")));
            }
        }
    }

    // ----- Positionen
    let ids: std::collections::HashSet<String> = tx
        .prepare("SELECT id FROM beleg")
        .map_err(|e| e.to_string())?
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    let mut je_beleg: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
    for r in &positionen {
        let beleg_id = erster(r, &["ID_Rechnung", "ID_Lieferschein", "ID_Auftrag", "ID_Angebot"]);
        if !ids.contains(&beleg_id) {
            bericht.waisen_positionen += 1;
            continue;
        }
        let p = Position {
            id: g(r, "ID_Position"),
            beleg_id: beleg_id.clone(),
            pos: n(r, "Position") as i64,
            artikel: g(r, "Artikel"),
            beschreibung: g(r, "Art.Beschreibung"),
            artikel_nr: g(r, "Artikelnr."),
            menge: n(r, "Menge"),
            einheit: g(r, "Einheit"),
            ek: n(r, "EK"),
            vk: n(r, "VK"),
            mwst: n(r, "MwSt Satz"),
            summe: n(r, "VK Summe"),
            merker: g(r, "Merker"),
        };
        p.save(&tx).map_err(|e| e.to_string())?;
        je_beleg.entry(beleg_id).or_default().push((p.mwst, p.summe));
        bericht.positionen += 1;
    }

    // ----- Standardtexte
    for r in &standardtexte {
        tx.execute(
            "INSERT OR REPLACE INTO standardtext(nr,bezeichnung,text) VALUES(?,?,?)",
            [g(r, "Nummer"), g(r, "Bezeichnung"), g(r, "Standard Brieftext")],
        )
        .map_err(|e| e.to_string())?;
    }

    // ----- Einstellungen (ohne Passwoerter/Codes/Container-Verweise)
    if let Some(r) = global.first() {
        let sperr = ["pass", "code", "freischalt", "pin", "kennwort"];
        for (k, v) in r {
            let kl = k.to_lowercase();
            let v = clean_text(v);
            let container_ref = v.len() == 6 && v.starts_with('C') && v.chars().skip(1).all(|c| c.is_ascii_hexdigit());
            if v.is_empty() || container_ref || sperr.iter().any(|s| kl.contains(s)) {
                continue;
            }
            tx.execute("INSERT OR REPLACE INTO einstellung(key,value) VALUES(?,?)", [k.as_str(), v.as_str()]).map_err(|e| e.to_string())?;
            bericht.einstellungen += 1;
        }
    }

    // ----- Inhaber (Mitarbeiter) - Empfaengername im Zahlungs-QR-Code
    if let Ok(ma) = lese(dir, "Mitarbeiter") {
        if let Some(n) = ma.iter().map(|r| g(r, "Gesamter_Name")).find(|n| !n.is_empty()) {
            tx.execute("INSERT OR REPLACE INTO einstellung(key,value) VALUES('Inhaber',?)", [n]).map_err(|e| e.to_string())?;
        }
    }

    // ----- Kontrolle: Rechnungssummen gegen AZURO
    for b in &alle_belege {
        let sm = summen(je_beleg.get(&b.id).map(|v| v.as_slice()).unwrap_or(&[]), b.rabatt_betrag);
        let (netto, brutto) = kontrolle.get(&b.id).copied().unwrap_or((0.0, 0.0));
        bericht.rechnungen_geprueft += 1;
        if (sm.netto - netto).abs() > 0.011 || (sm.brutto - brutto).abs() > 0.011 {
            bericht.rechnungen_abweichend += 1;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(bericht)
}

/// Zerlegt ein mehrzeiliges Adressfeld ("Name / Strasse / PLZ Ort / Land") in Strasse, PLZ, Ort.
fn adressfeld_zerlegen(feld: &str) -> (String, String, String) {
    let (mut strasse, mut plz, mut ort) = (String::new(), String::new(), String::new());
    for z in feld.lines().map(|z| z.trim()).filter(|z| !z.is_empty()).skip(1) {
        let t = z.trim_start_matches("AT-").trim_start_matches("DE-");
        let (kopf, rest) = t.split_once(' ').unwrap_or((t, ""));
        if kopf.len() >= 4 && kopf.len() <= 5 && kopf.chars().all(|c| c.is_ascii_digit()) && plz.is_empty() {
            plz = kopf.to_string();
            ort = rest.trim().to_string();
        } else if strasse.is_empty() && z.chars().any(|c| c.is_ascii_digit()) && plz.is_empty() {
            strasse = z.to_string();
        }
    }
    (strasse, plz, ort)
}

/// Eine Adresszeile aus AZURO -> Adresse. Ist keine Firma/kein Nachname gepflegt, dienen
/// "Firma_oder_Name" und das Postfach-Adressfeld als Ersatz (so stehen auch diese Adressen mit Namen im Stamm).
fn adresse_aus_row(r: &Row) -> Adresse {
    let brief = g(r, "Anrede");
    let bl = brief.to_lowercase();
    let mut a = Adresse {
        id: g(r, "ID"),
        firma_id: g(r, "ID_Firma"),
        kundennr: g(r, "Kundennummer"),
        anrede: if bl.contains("frau") { "Frau".into() } else if (bl.contains("herr") && !bl.contains("herren")) { "Herr".into() } else { String::new() },
        briefanrede: if brief.trim() == "," { String::new() } else { brief },
        titel: g(r, "Titel"),
        vorname: g(r, "Vorname"),
        nachname: g(r, "Nachname"),
        firma: g(r, "Firma"),
        strasse: g(r, "Firma Straße"),
        plz: g(r, "Firma PLZ"),
        ort: g(r, "Firma Ort"),
        land: g(r, "Firma Land"),
        telefon: g(r, "Firma Telefon"),
        telefon_ap: g(r, "Telefon AP"),
        fax: g(r, "Firma Faxnummer"),
        email: g(r, "EMail Adresse"),
        internet: g(r, "Internet"),
        position: g(r, "Position"),
        abteilung: g(r, "Abteilung"),
        kategorie: g(r, "Liste 2"),
        notizen: g(r, "Notizen"),
        sprache: g(r, "Sprache"),
        erstellt: d(r, "Erstellungsdatum"),
    };
    if a.firma.is_empty() && a.nachname.is_empty() {
        a.firma = g(r, "Firma_oder_Name").split_whitespace().collect::<Vec<_>>().join(" ");
    }
    let pf = g(r, "Adressfeld PF.");
    if a.strasse.is_empty() && a.plz.is_empty() && a.ort.is_empty() && !pf.is_empty() {
        let (strasse, plz, ort) = adressfeld_zerlegen(&pf);
        a.strasse = strasse;
        a.plz = plz;
        a.ort = ort;
        if a.land.is_empty() && pf.to_uppercase().contains("ÖSTERREICH") {
            a.land = "Österreich".into();
        }
    }
    a
}

/// Stellt Adressen aus dem AZURO-Export wieder her, ohne vorhandene Daten zu ueberschreiben:
/// fehlende Adressen werden angelegt, bei vorhandenen Adressen ohne Namen werden nur leere Felder gefuellt.
/// Rueckgabe: (neu angelegt, ergaenzt)
pub fn adressen_nachtragen(db: &mut Db, dir: &Path) -> Result<(Vec<String>, Vec<String>), String> {
    let rows = lese(dir, "Adressen")?;
    let tx = db.conn.transaction().map_err(|e| e.to_string())?;
    let (mut neu, mut ergaenzt) = (Vec::new(), Vec::new());
    for r in &rows {
        let a = adresse_aus_row(r);
        if a.id.is_empty() {
            continue;
        }
        let vorhanden: Option<(String, String)> = tx
            .query_row("SELECT coalesce(firma,''), coalesce(nachname,'') FROM adresse WHERE id=?", [&a.id], |x| Ok((x.get(0)?, x.get(1)?)))
            .ok();
        match vorhanden {
            None => {
                a.save(&tx).map_err(|e| e.to_string())?;
                neu.push(format!("{} (ID {})", a.name(), a.id));
            }
            Some((firma, nachname)) if firma.trim().is_empty() && nachname.trim().is_empty() && !a.firma.is_empty() => {
                tx.execute(
                    "UPDATE adresse SET firma=?, strasse=CASE WHEN coalesce(strasse,'')='' THEN ? ELSE strasse END, plz=CASE WHEN coalesce(plz,'')='' THEN ? ELSE plz END, ort=CASE WHEN coalesce(ort,'')='' THEN ? ELSE ort END, land=CASE WHEN coalesce(land,'')='' THEN ? ELSE land END WHERE id=?",
                    rusqlite::params![a.firma, a.strasse, a.plz, a.ort, a.land, a.id],
                )
                .map_err(|e| e.to_string())?;
                ergaenzt.push(format!("{} (ID {})", a.name(), a.id));
            }
            _ => {}
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok((neu, ergaenzt))
}
