//! Datenbank (SQLite): Schema, Datentypen und alle Abfragen.

use crate::calc::{summen, Summen};
use crate::util::*;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;

pub const TYPEN: [&str; 4] = ["Angebot", "Auftrag", "Lieferschein", "Rechnung"];

pub fn typ_suffix(typ: &str) -> &'static str {
    match typ {
        "Angebot" => "ABID",
        "Auftrag" => "AUID",
        "Lieferschein" => "LSID",
        _ => "RGID",
    }
}

/// Struct mit lauter Textfeldern + passendem Lesen/Schreiben.
macro_rules! text_struct {
    ($name:ident, $table:expr, { $($f:ident),* $(,)? }) => {
        #[derive(Clone, Default, Debug, PartialEq)]
        pub struct $name { $(pub $f: String),* }
        impl $name {
            pub const COLS: &'static [&'static str] = &[$(stringify!($f)),*];
            #[allow(unused_assignments)]
            pub fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Self> {
                let mut i = 0;
                Ok(Self { $($f: { let v: Option<String> = r.get(i)?; i += 1; v.unwrap_or_default() }),* })
            }
            pub fn values(&self) -> Vec<&String> { vec![$(&self.$f),*] }
            pub fn save(&self, c: &Connection) -> rusqlite::Result<()> {
                let ph = vec!["?"; Self::COLS.len()].join(",");
                let sql = format!("INSERT OR REPLACE INTO {}({}) VALUES({})", $table, Self::COLS.join(","), ph);
                c.execute(&sql, params_from_iter(self.values()))?;
                Ok(())
            }
        }
    };
}

text_struct!(Adresse, "adresse", {
    id, firma_id, kundennr, anrede, briefanrede, titel, vorname, nachname, firma, strasse, plz, ort, land,
    telefon, telefon_ap, fax, email, internet, position, abteilung, kategorie, notizen, sprache, erstellt
});

impl Adresse {
    /// Anzeigename: Firma, sonst Person
    pub fn name(&self) -> String {
        let person = format!("{} {}", self.vorname, self.nachname).trim().to_string();
        let name = if !self.firma.is_empty() {
            if person.is_empty() { self.firma.clone() } else { format!("{} ({})", self.firma, person) }
        } else {
            person
        };
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty() { format!("(ohne Namen) Kd.-Nr. {}", self.kundennr) } else { name }
    }
    /// Mehrzeiliges Adressfeld fuer Belege
    pub fn adressfeld(&self) -> String {
        let mut l: Vec<String> = Vec::new();
        if !self.firma.is_empty() {
            l.push(self.firma.clone());
        }
        let person = [self.anrede_kurz(), self.titel.clone(), self.vorname.clone(), self.nachname.clone()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ");
        if !person.is_empty() {
            l.push(person);
        }
        if !self.strasse.is_empty() {
            l.push(self.strasse.clone());
        }
        let ort = format!("{} {}", self.plz, self.ort).trim().to_string();
        if !ort.is_empty() {
            l.push(ort);
        }
        if !self.land.is_empty() && !self.land.eq_ignore_ascii_case("Österreich") {
            l.push(self.land.clone());
        }
        l.join("\n")
    }
    fn anrede_kurz(&self) -> String {
        let a = self.anrede.to_lowercase();
        if a.contains("herr") && !a.contains("frau") { "Herrn".into() } else if a.contains("frau") { "Frau".into() } else { String::new() }
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Artikel {
    pub id: String,
    pub nr: String,
    pub name: String,
    pub beschreibung: String,
    pub gruppe: String,
    pub einheit: String,
    pub art: String,
    pub ek: f64,
    pub vk: f64,
    pub mwst: f64,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Beleg {
    pub id: String,
    pub typ: String,
    pub nr: i64,
    pub datum: String,
    pub adresse_id: String,
    pub kundennr: String,
    pub adressfeld: String,
    pub briefanrede: String,
    pub betreff: String,
    pub kopftext: String,
    pub fusstext: String,
    pub status: String,
    pub rabatt_prozent: f64,
    pub rabatt_betrag: f64,
    pub rabatt_text: String,
    pub bezahlt: f64,
    pub zahldatum: String,
    pub mahnstufe: i64,
    pub mahn1: String,
    pub mahn2: String,
    pub mahn3: String,
    pub leistungsdatum: String,
    pub gueltig_bis: String,
    pub aus_angebot: String,
    pub aus_auftrag: String,
    pub aus_lieferschein: String,
    pub zu_auftrag: String,
    pub zu_lieferschein: String,
    pub zu_rechnung: String,
    pub waehrung: String,
    pub sprache: String,
    pub erstellt: String,
    pub bearbeiter: String,
    pub notizen: String,
}

const BELEG_COLS: &str = "id,typ,nr,datum,adresse_id,kundennr,adressfeld,briefanrede,betreff,kopftext,fusstext,status,rabatt_prozent,rabatt_betrag,rabatt_text,bezahlt,zahldatum,mahnstufe,mahn1,mahn2,mahn3,leistungsdatum,gueltig_bis,aus_angebot,aus_auftrag,aus_lieferschein,zu_auftrag,zu_lieferschein,zu_rechnung,waehrung,sprache,erstellt,bearbeiter,notizen";

fn s(r: &rusqlite::Row, i: usize) -> rusqlite::Result<String> {
    Ok(r.get::<_, Option<String>>(i)?.unwrap_or_default())
}
fn f(r: &rusqlite::Row, i: usize) -> rusqlite::Result<f64> {
    Ok(r.get::<_, Option<f64>>(i)?.unwrap_or(0.0))
}

impl Beleg {
    fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Beleg {
            id: s(r, 0)?, typ: s(r, 1)?, nr: r.get::<_, Option<i64>>(2)?.unwrap_or(0), datum: s(r, 3)?, adresse_id: s(r, 4)?, kundennr: s(r, 5)?,
            adressfeld: s(r, 6)?, briefanrede: s(r, 7)?, betreff: s(r, 8)?, kopftext: s(r, 9)?, fusstext: s(r, 10)?, status: s(r, 11)?,
            rabatt_prozent: f(r, 12)?, rabatt_betrag: f(r, 13)?, rabatt_text: s(r, 14)?, bezahlt: f(r, 15)?, zahldatum: s(r, 16)?,
            mahnstufe: r.get::<_, Option<i64>>(17)?.unwrap_or(0), mahn1: s(r, 18)?, mahn2: s(r, 19)?, mahn3: s(r, 20)?, leistungsdatum: s(r, 21)?,
            gueltig_bis: s(r, 22)?, aus_angebot: s(r, 23)?, aus_auftrag: s(r, 24)?, aus_lieferschein: s(r, 25)?, zu_auftrag: s(r, 26)?,
            zu_lieferschein: s(r, 27)?, zu_rechnung: s(r, 28)?, waehrung: s(r, 29)?, sprache: s(r, 30)?, erstellt: s(r, 31)?, bearbeiter: s(r, 32)?, notizen: s(r, 33)?,
        })
    }
    pub fn save(&self, c: &Connection) -> rusqlite::Result<()> {
        let sql = format!("INSERT OR REPLACE INTO beleg({}) VALUES({})", BELEG_COLS, vec!["?"; 34].join(","));
        c.execute(
            &sql,
            params![
                self.id, self.typ, self.nr, self.datum, self.adresse_id, self.kundennr, self.adressfeld, self.briefanrede, self.betreff, self.kopftext,
                self.fusstext, self.status, self.rabatt_prozent, self.rabatt_betrag, self.rabatt_text, self.bezahlt, self.zahldatum, self.mahnstufe,
                self.mahn1, self.mahn2, self.mahn3, self.leistungsdatum, self.gueltig_bis, self.aus_angebot, self.aus_auftrag, self.aus_lieferschein,
                self.zu_auftrag, self.zu_lieferschein, self.zu_rechnung, self.waehrung, self.sprache, self.erstellt, self.bearbeiter, self.notizen
            ],
        )?;
        Ok(())
    }
    /// Anzeigename "Rechnung Nr. 12"
    pub fn titel(&self) -> String {
        format!("{} Nr. {}", self.typ, self.nr)
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Position {
    pub id: String,
    pub beleg_id: String,
    pub pos: i64,
    pub artikel: String,
    pub beschreibung: String,
    pub artikel_nr: String,
    pub menge: f64,
    pub einheit: String,
    pub ek: f64,
    pub vk: f64,
    pub mwst: f64,
    pub summe: f64,
    pub merker: String,
}

impl Position {
    pub fn berechnen(&mut self) {
        self.summe = round2(self.menge * self.vk);
    }
    pub fn save(&self, c: &Connection) -> rusqlite::Result<()> {
        c.execute(
            "INSERT OR REPLACE INTO position(id,beleg_id,pos,artikel,beschreibung,artikel_nr,menge,einheit,ek,vk,mwst,summe,merker) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)",
            params![self.id, self.beleg_id, self.pos, self.artikel, self.beschreibung, self.artikel_nr, self.menge, self.einheit, self.ek, self.vk, self.mwst, self.summe, self.merker],
        )?;
        Ok(())
    }
    fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Position {
            id: s(r, 0)?, beleg_id: s(r, 1)?, pos: r.get::<_, Option<i64>>(2)?.unwrap_or(0), artikel: s(r, 3)?, beschreibung: s(r, 4)?, artikel_nr: s(r, 5)?,
            menge: f(r, 6)?, einheit: s(r, 7)?, ek: f(r, 8)?, vk: f(r, 9)?, mwst: f(r, 10)?, summe: f(r, 11)?, merker: s(r, 12)?,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Kennzahlen {
    pub offene_rechnungen: usize,
    pub offener_betrag: f64,
    pub umsatz_jahr: f64,
    pub umsatz_vorjahr: f64,
    pub offene_angebote: usize,
    pub offene_auftraege: usize,
    pub ueberfaellig: usize,
}

pub struct Db {
    pub conn: Connection,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS einstellung(key TEXT PRIMARY KEY, value TEXT NOT NULL DEFAULT '');
CREATE TABLE IF NOT EXISTS adresse(id TEXT PRIMARY KEY, firma_id TEXT, kundennr TEXT, anrede TEXT, briefanrede TEXT, titel TEXT, vorname TEXT, nachname TEXT,
  firma TEXT, strasse TEXT, plz TEXT, ort TEXT, land TEXT, telefon TEXT, telefon_ap TEXT, fax TEXT, email TEXT, internet TEXT, position TEXT, abteilung TEXT,
  kategorie TEXT, notizen TEXT, sprache TEXT, erstellt TEXT);
CREATE TABLE IF NOT EXISTS artikel(id TEXT PRIMARY KEY, nr TEXT, name TEXT, beschreibung TEXT, gruppe TEXT, einheit TEXT, art TEXT, ek REAL, vk REAL, mwst REAL);
CREATE TABLE IF NOT EXISTS beleg(id TEXT PRIMARY KEY, typ TEXT NOT NULL, nr INTEGER, datum TEXT, adresse_id TEXT, kundennr TEXT, adressfeld TEXT, briefanrede TEXT,
  betreff TEXT, kopftext TEXT, fusstext TEXT, status TEXT, rabatt_prozent REAL, rabatt_betrag REAL, rabatt_text TEXT, bezahlt REAL, zahldatum TEXT, mahnstufe INTEGER,
  mahn1 TEXT, mahn2 TEXT, mahn3 TEXT, leistungsdatum TEXT, gueltig_bis TEXT, aus_angebot TEXT, aus_auftrag TEXT, aus_lieferschein TEXT, zu_auftrag TEXT,
  zu_lieferschein TEXT, zu_rechnung TEXT, waehrung TEXT, sprache TEXT, erstellt TEXT, bearbeiter TEXT, notizen TEXT);
CREATE INDEX IF NOT EXISTS beleg_typ_nr ON beleg(typ, nr);
CREATE INDEX IF NOT EXISTS beleg_adresse ON beleg(adresse_id);
CREATE TABLE IF NOT EXISTS position(id TEXT PRIMARY KEY, beleg_id TEXT NOT NULL, pos INTEGER, artikel TEXT, beschreibung TEXT, artikel_nr TEXT, menge REAL,
  einheit TEXT, ek REAL, vk REAL, mwst REAL, summe REAL, merker TEXT);
CREATE INDEX IF NOT EXISTS position_beleg ON position(beleg_id, pos);
CREATE TABLE IF NOT EXISTS standardtext(nr TEXT PRIMARY KEY, bezeichnung TEXT, text TEXT);
";

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        conn.execute_batch(SCHEMA)?;
        Ok(Db { conn })
    }

    pub fn memory() -> rusqlite::Result<Db> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Db { conn })
    }

    // ---------------------------------------------------------------- Einstellungen
    pub fn get(&self, key: &str) -> String {
        self.conn.query_row("SELECT value FROM einstellung WHERE key=?", [key], |r| r.get::<_, String>(0)).optional().ok().flatten().unwrap_or_default()
    }
    pub fn set(&self, key: &str, value: &str) {
        let _ = self.conn.execute("INSERT OR REPLACE INTO einstellung(key,value) VALUES(?,?)", [key, value]);
    }
    pub fn alle_einstellungen(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        if let Ok(mut st) = self.conn.prepare("SELECT key,value FROM einstellung") {
            if let Ok(rows) = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))) {
                for (k, v) in rows.flatten() {
                    m.insert(k, v);
                }
            }
        }
        m
    }
    pub fn ist_leer(&self) -> bool {
        self.conn.query_row("SELECT COUNT(*) FROM beleg", [], |r| r.get::<_, i64>(0)).unwrap_or(0) == 0
            && self.conn.query_row("SELECT COUNT(*) FROM adresse", [], |r| r.get::<_, i64>(0)).unwrap_or(0) == 0
    }

    // ---------------------------------------------------------------- Adressen
    pub fn adressen(&self, suche: &str) -> Vec<Adresse> {
        let like = format!("%{}%", suche.trim().to_lowercase());
        let sql = format!(
            "SELECT {} FROM adresse WHERE lower(coalesce(firma,'')||' '||coalesce(nachname,'')||' '||coalesce(vorname,'')||' '||coalesce(ort,'')||' '||coalesce(kundennr,'')||' '||coalesce(email,'')) LIKE ? ORDER BY lower(coalesce(nullif(firma,''),nachname)), lower(nachname)",
            Adresse::COLS.join(",")
        );
        let mut out = Vec::new();
        if let Ok(mut st) = self.conn.prepare(&sql) {
            if let Ok(rows) = st.query_map([like], Adresse::from_row) {
                out.extend(rows.flatten());
            }
        }
        out
    }
    pub fn adresse(&self, id: &str) -> Option<Adresse> {
        let sql = format!("SELECT {} FROM adresse WHERE id=?", Adresse::COLS.join(","));
        self.conn.query_row(&sql, [id], Adresse::from_row).optional().ok().flatten()
    }
    pub fn adresse_speichern(&self, a: &Adresse) -> rusqlite::Result<()> {
        a.save(&self.conn)
    }
    pub fn adresse_loeschen(&self, id: &str) {
        let _ = self.conn.execute("DELETE FROM adresse WHERE id=?", [id]);
    }
    pub fn naechste_kundennr(&self) -> String {
        let max: i64 = self.conn.query_row("SELECT COALESCE(MAX(CAST(kundennr AS INTEGER)),0) FROM adresse", [], |r| r.get(0)).unwrap_or(0);
        let basis = parse_num(&self.get("Kundennummer")) as i64;
        (max.max(basis) + 1).to_string()
    }
    pub fn neue_id(&self, prefix: &str) -> String {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        format!("{}{:x}", prefix, n)
    }

    // ---------------------------------------------------------------- Artikel
    pub fn artikel_liste(&self, suche: &str) -> Vec<Artikel> {
        let like = format!("%{}%", suche.trim().to_lowercase());
        let mut out = Vec::new();
        if let Ok(mut st) = self.conn.prepare(
            "SELECT id,nr,name,beschreibung,gruppe,einheit,art,ek,vk,mwst FROM artikel WHERE lower(coalesce(nr,'')||' '||coalesce(name,'')||' '||coalesce(gruppe,'')||' '||coalesce(beschreibung,'')) LIKE ? ORDER BY CAST(nr AS INTEGER), lower(name)",
        ) {
            if let Ok(rows) = st.query_map([like], |r| {
                Ok(Artikel { id: s(r, 0)?, nr: s(r, 1)?, name: s(r, 2)?, beschreibung: s(r, 3)?, gruppe: s(r, 4)?, einheit: s(r, 5)?, art: s(r, 6)?, ek: f(r, 7)?, vk: f(r, 8)?, mwst: f(r, 9)? })
            }) {
                out.extend(rows.flatten());
            }
        }
        out
    }
    pub fn artikel_speichern(&self, a: &Artikel) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO artikel(id,nr,name,beschreibung,gruppe,einheit,art,ek,vk,mwst) VALUES(?,?,?,?,?,?,?,?,?,?)",
            params![a.id, a.nr, a.name, a.beschreibung, a.gruppe, a.einheit, a.art, a.ek, a.vk, a.mwst],
        )?;
        Ok(())
    }
    pub fn artikel_loeschen(&self, id: &str) {
        let _ = self.conn.execute("DELETE FROM artikel WHERE id=?", [id]);
    }
    pub fn naechste_artikelnr(&self) -> String {
        let max: i64 = self.conn.query_row("SELECT COALESCE(MAX(CAST(nr AS INTEGER)),0) FROM artikel", [], |r| r.get(0)).unwrap_or(0);
        (max.max(parse_num(&self.get("Serien Nr. Artikel")) as i64) + 1).to_string()
    }

    // ---------------------------------------------------------------- Belege
    /// Netto je Steuersatz aller Belege (Beleg-ID -> [(Satz, Summe)])
    fn satz_summen(&self) -> HashMap<String, Vec<(f64, f64)>> {
        let mut m: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
        if let Ok(mut st) = self.conn.prepare("SELECT beleg_id, COALESCE(mwst,0), COALESCE(SUM(summe),0) FROM position GROUP BY beleg_id, mwst") {
            if let Ok(rows) = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?, r.get::<_, f64>(2)?))) {
                for (id, satz, sum) in rows.flatten() {
                    m.entry(id).or_default().push((satz, sum));
                }
            }
        }
        m
    }

    pub fn belege(&self, typ: &str, suche: &str, nur_offen: bool) -> Vec<(Beleg, Summen)> {
        let like = format!("%{}%", suche.trim().to_lowercase());
        let mut sql = format!(
            "SELECT {} FROM beleg WHERE typ=? AND lower(coalesce(nr,'')||' '||coalesce(adressfeld,'')||' '||coalesce(betreff,'')||' '||coalesce(kundennr,'')||' '||coalesce(status,'')) LIKE ?",
            BELEG_COLS
        );
        if nur_offen {
            sql.push_str(" AND status IN ('offen','teilbezahlt')");
        }
        sql.push_str(" ORDER BY datum DESC, nr DESC");
        let sums = self.satz_summen();
        let mut out = Vec::new();
        if let Ok(mut st) = self.conn.prepare(&sql) {
            if let Ok(rows) = st.query_map(params![typ, like], Beleg::from_row) {
                for b in rows.flatten() {
                    let sm = summen(sums.get(&b.id).map(|v| v.as_slice()).unwrap_or(&[]), b.rabatt_betrag);
                    out.push((b, sm));
                }
            }
        }
        out
    }

    pub fn beleg(&self, id: &str) -> Option<Beleg> {
        let sql = format!("SELECT {} FROM beleg WHERE id=?", BELEG_COLS);
        self.conn.query_row(&sql, [id], Beleg::from_row).optional().ok().flatten()
    }
    pub fn beleg_nach_nr(&self, typ: &str, nr: i64) -> Option<Beleg> {
        let sql = format!("SELECT {} FROM beleg WHERE typ=? AND nr=?", BELEG_COLS);
        self.conn.query_row(&sql, params![typ, nr], Beleg::from_row).optional().ok().flatten()
    }
    pub fn belege_von_adresse(&self, adresse_id: &str) -> Vec<(Beleg, Summen)> {
        let sql = format!("SELECT {} FROM beleg WHERE adresse_id=? ORDER BY datum DESC, nr DESC", BELEG_COLS);
        let sums = self.satz_summen();
        let mut out = Vec::new();
        if let Ok(mut st) = self.conn.prepare(&sql) {
            if let Ok(rows) = st.query_map([adresse_id], Beleg::from_row) {
                for b in rows.flatten() {
                    let sm = summen(sums.get(&b.id).map(|v| v.as_slice()).unwrap_or(&[]), b.rabatt_betrag);
                    out.push((b, sm));
                }
            }
        }
        out
    }
    pub fn positionen(&self, beleg_id: &str) -> Vec<Position> {
        let mut out = Vec::new();
        if let Ok(mut st) = self.conn.prepare("SELECT id,beleg_id,pos,artikel,beschreibung,artikel_nr,menge,einheit,ek,vk,mwst,summe,merker FROM position WHERE beleg_id=? ORDER BY pos") {
            if let Ok(rows) = st.query_map([beleg_id], Position::from_row) {
                out.extend(rows.flatten());
            }
        }
        out
    }

    pub fn summen_von(&self, b: &Beleg, pos: &[Position]) -> Summen {
        let _ = b;
        let je: Vec<(f64, f64)> = pos.iter().map(|p| (p.mwst, p.summe)).collect();
        summen(&je, b.rabatt_betrag)
    }

    pub fn beleg_speichern(&mut self, b: &Beleg, pos: &[Position]) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        b.save(&tx)?;
        tx.execute("DELETE FROM position WHERE beleg_id=?", [&b.id])?;
        for (i, p) in pos.iter().enumerate() {
            let mut p = p.clone();
            p.beleg_id = b.id.clone();
            p.pos = i as i64 + 1;
            p.id = format!("POS{}_{}", p.pos, b.id);
            p.save(&tx)?;
        }
        tx.commit()
    }

    pub fn beleg_loeschen(&mut self, id: &str) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM position WHERE beleg_id=?", [id])?;
        tx.execute("DELETE FROM beleg WHERE id=?", [id])?;
        tx.commit()
    }

    pub fn naechste_belegnr(&self, typ: &str) -> i64 {
        let max: i64 = self.conn.query_row("SELECT COALESCE(MAX(nr),0) FROM beleg WHERE typ=?", [typ], |r| r.get(0)).unwrap_or(0);
        let basis = parse_num(&self.get(&format!("Serien Nr. {}", typ))) as i64;
        max.max(basis) + 1
    }

    /// Neuer, noch nicht gespeicherter Beleg mit Standardtexten aus den Einstellungen.
    pub fn neuer_beleg(&self, typ: &str, adresse: Option<&Adresse>) -> Beleg {
        let nr = self.naechste_belegnr(typ);
        let kopf_key = match typ {
            "Angebot" => "Text Einleitung ANG",
            "Auftrag" => "Text Einleitung AUF",
            "Lieferschein" => "Text Einleitung LFS",
            _ => "Text Einleitung RNG",
        };
        let datum = today_iso();
        let frist_wochen = parse_num(&self.get("Angebotsfrist")) as i64;
        let gruss = self.get("Brieffloskel");
        let mut b = Beleg {
            id: format!("N{}{}", nr, typ_suffix(typ)),
            typ: typ.to_string(),
            nr,
            datum: datum.clone(),
            kopftext: self.get(kopf_key),
            fusstext: gruss,
            status: "offen".into(),
            waehrung: { let w = self.get("Währung"); if w.is_empty() { "€".into() } else { w } },
            sprache: "deutsch".into(),
            erstellt: datum.clone(),
            ..Default::default()
        };
        if typ == "Angebot" {
            b.gueltig_bis = add_days_iso(&datum, 7 * if frist_wochen > 0 { frist_wochen } else { 4 });
        }
        if let Some(a) = adresse {
            self.adresse_uebernehmen(&mut b, a);
        }
        b
    }

    pub fn adresse_uebernehmen(&self, b: &mut Beleg, a: &Adresse) {
        b.adresse_id = a.id.clone();
        b.kundennr = a.kundennr.clone();
        b.adressfeld = a.adressfeld();
        b.briefanrede = if a.briefanrede.is_empty() { self.get("Anrede") } else { a.briefanrede.clone() };
    }

    /// Erzeugt aus einem Beleg den Folgebeleg (Angebot -> Auftrag -> Lieferschein -> Rechnung).
    /// Positionen werden kopiert, Quelle und Ziel verknuepft. Gibt den neuen Beleg zurueck.
    pub fn folgebeleg(&mut self, quelle: &Beleg, neuer_typ: &str) -> rusqlite::Result<Beleg> {
        let pos = self.positionen(&quelle.id);
        let mut neu = self.neuer_beleg(neuer_typ, None);
        neu.adresse_id = quelle.adresse_id.clone();
        neu.kundennr = quelle.kundennr.clone();
        neu.adressfeld = quelle.adressfeld.clone();
        neu.briefanrede = quelle.briefanrede.clone();
        neu.betreff = quelle.betreff.clone();
        neu.rabatt_prozent = quelle.rabatt_prozent;
        neu.rabatt_betrag = quelle.rabatt_betrag;
        neu.rabatt_text = quelle.rabatt_text.clone();
        neu.waehrung = quelle.waehrung.clone();
        neu.sprache = quelle.sprache.clone();
        match quelle.typ.as_str() {
            "Angebot" => neu.aus_angebot = quelle.nr.to_string(),
            "Auftrag" => {
                neu.aus_auftrag = quelle.nr.to_string();
                neu.aus_angebot = quelle.aus_angebot.clone();
            }
            "Lieferschein" => {
                neu.aus_lieferschein = quelle.nr.to_string();
                neu.aus_auftrag = quelle.aus_auftrag.clone();
                neu.aus_angebot = quelle.aus_angebot.clone();
            }
            _ => {}
        }
        if neuer_typ == "Rechnung" {
            neu.leistungsdatum = quelle.datum.clone();
        }
        let mut q = quelle.clone();
        match neuer_typ {
            "Auftrag" => {
                q.zu_auftrag = neu.nr.to_string();
                if q.status == "offen" {
                    q.status = "angenommen".into();
                }
            }
            "Lieferschein" => q.zu_lieferschein = neu.nr.to_string(),
            "Rechnung" => {
                q.zu_rechnung = neu.nr.to_string();
                q.status = "abgerechnet".into();
            }
            _ => {}
        }
        let tx = self.conn.transaction()?;
        neu.save(&tx)?;
        for (i, p) in pos.iter().enumerate() {
            let mut p = p.clone();
            p.beleg_id = neu.id.clone();
            p.pos = i as i64 + 1;
            p.id = format!("POS{}_{}", p.pos, neu.id);
            p.save(&tx)?;
        }
        q.save(&tx)?;
        tx.commit()?;
        Ok(neu)
    }

    /// Kopie eines Belegs anlegen (gleicher Typ, naechste freie Nummer).
    /// Adresse, Betreff, Rabatt, Waehrung, Sprache und Positionen werden uebernommen.
    /// Herkunfts- und Folge-Verknuepfungen (aus_* / zu_*) bleiben leer, Status = "offen",
    /// Datum = heute. Die Quelle wird nicht veraendert.
    pub fn beleg_kopieren(&mut self, quelle: &Beleg) -> rusqlite::Result<Beleg> {
        let pos = self.positionen(&quelle.id);
        let mut neu = self.neuer_beleg(&quelle.typ, None);
        neu.adresse_id = quelle.adresse_id.clone();
        neu.kundennr = quelle.kundennr.clone();
        neu.adressfeld = quelle.adressfeld.clone();
        neu.briefanrede = quelle.briefanrede.clone();
        neu.betreff = quelle.betreff.clone();
        neu.kopftext = quelle.kopftext.clone();
        neu.fusstext = quelle.fusstext.clone();
        neu.rabatt_prozent = quelle.rabatt_prozent;
        neu.rabatt_betrag = quelle.rabatt_betrag;
        neu.rabatt_text = quelle.rabatt_text.clone();
        neu.waehrung = quelle.waehrung.clone();
        neu.sprache = quelle.sprache.clone();
        let tx = self.conn.transaction()?;
        neu.save(&tx)?;
        for (i, p) in pos.iter().enumerate() {
            let mut p = p.clone();
            p.beleg_id = neu.id.clone();
            p.pos = i as i64 + 1;
            p.id = format!("POS{}_{}", p.pos, neu.id);
            p.save(&tx)?;
        }
        tx.commit()?;
        Ok(neu)
    }

    /// Zahlung auf eine Rechnung buchen; Status wird aus Betrag/Bezahlt abgeleitet.
    pub fn zahlung_buchen(&self, b: &mut Beleg, brutto: f64, betrag: f64, datum: &str) -> rusqlite::Result<()> {
        b.bezahlt = round2(b.bezahlt + betrag);
        b.zahldatum = datum.to_string();
        b.status = status_fuer(brutto, b.bezahlt);
        b.save(&self.conn)
    }

    pub fn kennzahlen(&self) -> Kennzahlen {
        let sums = self.satz_summen();
        let jahr = chrono::Local::now().format("%Y").to_string().parse::<i32>().unwrap_or(2026);
        let heute = today_iso();
        let mut k = Kennzahlen::default();
        let sql = format!("SELECT {} FROM beleg", BELEG_COLS);
        if let Ok(mut st) = self.conn.prepare(&sql) {
            if let Ok(rows) = st.query_map([], Beleg::from_row) {
                for b in rows.flatten() {
                    match b.typ.as_str() {
                        "Rechnung" => {
                            let sm = summen(sums.get(&b.id).map(|v| v.as_slice()).unwrap_or(&[]), b.rabatt_betrag);
                            let y = year_of(&b.datum);
                            if y == jahr { k.umsatz_jahr += sm.netto; }
                            if y == jahr - 1 { k.umsatz_vorjahr += sm.netto; }
                            if b.status != "bezahlt" {
                                k.offene_rechnungen += 1;
                                k.offener_betrag += sm.brutto - b.bezahlt;
                                if !b.datum.is_empty() && add_days_iso(&b.datum, 14) < heute { k.ueberfaellig += 1; }
                            }
                        }
                        "Angebot" if b.status == "offen" => k.offene_angebote += 1,
                        "Auftrag" if b.status == "offen" => k.offene_auftraege += 1,
                        _ => {}
                    }
                }
            }
        }
        k.offener_betrag = round2(k.offener_betrag);
        k.umsatz_jahr = round2(k.umsatz_jahr);
        k.umsatz_vorjahr = round2(k.umsatz_vorjahr);
        k
    }
}

pub fn status_fuer(brutto: f64, bezahlt: f64) -> String {
    if bezahlt >= brutto - 0.005 && brutto > 0.0 {
        "bezahlt".into()
    } else if bezahlt > 0.005 {
        "teilbezahlt".into()
    } else {
        "offen".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> Db {
        let db = Db::memory().unwrap();
        db.set("Angebotsfrist", "4");
        db.set("Serien Nr. Rechnung", "10");
        db
    }

    #[test]
    fn nummern_und_folgebeleg() {
        let mut db = demo();
        let mut a = Adresse { id: "A1".into(), firma: "Muster GmbH".into(), strasse: "Weg 1".into(), plz: "5724".into(), ort: "Stuhlfelden".into(), kundennr: "1".into(), ..Default::default() };
        a.vorname = "Max".into();
        a.nachname = "Muster".into();
        db.adresse_speichern(&a).unwrap();
        let mut ang = db.neuer_beleg("Angebot", Some(&a));
        assert_eq!(ang.nr, 1);
        assert!(ang.adressfeld.contains("Muster GmbH") && ang.adressfeld.contains("5724 Stuhlfelden"));
        let mut p = Position { artikel: "Logo".into(), menge: 2.0, vk: 100.0, mwst: 20.0, einheit: "Stk.".into(), ..Default::default() };
        p.berechnen();
        db.beleg_speichern(&ang, &[p]).unwrap();
        ang = db.beleg(&ang.id).unwrap();
        let auftrag = db.folgebeleg(&ang, "Auftrag").unwrap();
        assert_eq!(db.beleg(&ang.id).unwrap().status, "angenommen");
        let rg = db.folgebeleg(&auftrag, "Rechnung").unwrap();
        assert_eq!(rg.nr, 11, "Rechnungsnummer setzt hinter dem Zaehler aus den Einstellungen fort");
        assert_eq!(rg.aus_auftrag, "1");
        assert_eq!(rg.aus_angebot, "1");
        let pos = db.positionen(&rg.id);
        assert_eq!(pos.len(), 1);
        let sm = db.summen_von(&rg, &pos);
        assert_eq!((sm.netto, sm.mwst_summe, sm.brutto), (200.0, 40.0, 240.0));
        assert_eq!(db.beleg(&auftrag.id).unwrap().status, "abgerechnet");
    }

    #[test]
    fn zahlungen() {
        let db = demo();
        let mut b = db.neuer_beleg("Rechnung", None);
        db.zahlung_buchen(&mut b, 100.0, 40.0, "2026-01-01").unwrap();
        assert_eq!(b.status, "teilbezahlt");
        db.zahlung_buchen(&mut b, 100.0, 60.0, "2026-01-02").unwrap();
        assert_eq!(b.status, "bezahlt");
    }
}
