//! Mahnwesen: faellige Mahnungen ermitteln (Fristen wie in AZURO), Mahnung erstellen und buchen, Oberflaeche.
//!
//! Regeln (aus den AZURO-Einstellungen/-Daten abgeleitet):
//! * Mahnziel 1 (14 Tage) = Tage nach Rechnungsdatum bis zur 1. Mahnung (freundliche Erinnerung, keine Gebuehr)
//! * Mahnziel 2 / 3 (je 8 Tage) = Frist nach der 1./2. Mahnung bis zur naechsten Stufe; zugleich Zahlungsfrist in der Mahnung
//! * Mahngebuehr 2 / 3 (5 / 10 EUR) und Verzugszinsen 2 / 3 (% p. a.) ab der 2. bzw. 3. Mahnung

use crate::app::*;
use crate::calc::Summen;
use crate::db::*;
use crate::util::*;
use egui::{RichText, Ui};

#[derive(Clone, Debug)]
pub struct MahnPosten {
    pub beleg: Beleg,
    pub summen: Summen,
    pub offen: f64,
    pub tage: i64,
    /// naechste faellige Stufe (0 = noch nichts faellig)
    pub vorschlag: i64,
}

#[derive(Clone, Debug)]
pub struct MahnGruppe {
    pub adresse_id: String,
    pub kunde: String,
    pub adressfeld: String,
    pub kundennr: String,
    pub briefanrede: String,
    pub posten: Vec<MahnPosten>,
}

impl MahnGruppe {
    pub fn vorschlag(&self) -> i64 {
        self.posten.iter().map(|p| p.vorschlag).max().unwrap_or(0)
    }
    pub fn offen(&self) -> f64 {
        self.posten.iter().map(|p| p.offen).sum()
    }
}

pub fn tage_zwischen(von_iso: &str, bis_iso: &str) -> i64 {
    let p = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    match (p(von_iso), p(bis_iso)) {
        (Some(a), Some(b)) => (b - a).num_days(),
        _ => 0,
    }
}

/// Welche Mahnstufe ist fuer die Rechnung heute faellig? (0 = keine)
pub fn faellige_stufe(b: &Beleg, heute: &str, ziel1: i64, ziel2: i64, ziel3: i64) -> i64 {
    match b.mahnstufe {
        0 if !b.datum.is_empty() && tage_zwischen(&b.datum, heute) > ziel1 => 1,
        1 if !b.mahn1.is_empty() && tage_zwischen(&b.mahn1, heute) > ziel2 => 2,
        2 if !b.mahn2.is_empty() && tage_zwischen(&b.mahn2, heute) > ziel3 => 3,
        _ => 0,
    }
}

impl Db {
    fn mahn_ziel(&self, key: &str, standard: i64) -> i64 {
        let v = parse_num(&self.get(key)) as i64;
        if v > 0 { v } else { standard }
    }

    /// Alle offenen Rechnungen, gruppiert nach Kunde, die am laengsten ueberfaelligen zuerst.
    /// `max_tage` > 0: nur Rechnungen, die hoechstens so alt sind (aeltere Altlasten werden nicht vorgeschlagen).
    pub fn mahnliste(&self, max_tage: i64) -> Vec<MahnGruppe> {
        let heute = today_iso();
        let (z1, z2, z3) = (self.mahn_ziel("Mahnziel 1", 14), self.mahn_ziel("Mahnziel 2", 8), self.mahn_ziel("Mahnziel 3", 8));
        let mut gruppen: Vec<MahnGruppe> = Vec::new();
        for (b, sm) in self.belege("Rechnung", "", true) {
            let offen = round2(sm.brutto - b.bezahlt);
            if offen <= 0.004 || (max_tage > 0 && tage_zwischen(&b.datum, &heute) > max_tage) {
                continue;
            }
            let schluessel = if b.adresse_id.is_empty() { b.adressfeld.lines().next().unwrap_or("").to_string() } else { b.adresse_id.clone() };
            let posten = MahnPosten { vorschlag: faellige_stufe(&b, &heute, z1, z2, z3), tage: tage_zwischen(&b.datum, &heute), offen, summen: sm, beleg: b.clone() };
            if let Some(g) = gruppen.iter_mut().find(|g| (if g.adresse_id.is_empty() { g.kunde.clone() } else { g.adresse_id.clone() }) == schluessel) {
                g.posten.push(posten);
            } else {
                gruppen.push(MahnGruppe {
                    adresse_id: b.adresse_id.clone(),
                    kunde: b.adressfeld.lines().next().unwrap_or("").trim().to_string(),
                    adressfeld: b.adressfeld.clone(),
                    kundennr: b.kundennr.clone(),
                    briefanrede: b.briefanrede.clone(),
                    posten: vec![posten],
                });
            }
        }
        for g in gruppen.iter_mut() {
            g.posten.sort_by(|a, b| a.beleg.datum.cmp(&b.beleg.datum));
        }
        gruppen.sort_by(|a, b| b.posten.iter().map(|p| p.tage).max().cmp(&a.posten.iter().map(|p| p.tage).max()));
        gruppen
    }

    /// Vermerkt die Mahnung (Stufe und Datum) bei allen angegebenen Rechnungen.
    pub fn mahnung_buchen(&self, belege: &[Beleg], stufe: i64, datum_iso: &str) -> rusqlite::Result<()> {
        for b in belege {
            let mut b = b.clone();
            b.mahnstufe = stufe;
            match stufe {
                1 => b.mahn1 = datum_iso.to_string(),
                2 => b.mahn2 = datum_iso.to_string(),
                _ => b.mahn3 = datum_iso.to_string(),
            }
            b.save(&self.conn)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MahnState {
    pub gruppen: Vec<MahnGruppe>,
    pub stufen: Vec<i64>,
    pub veraltet: bool,
    pub nur_faellige: bool,
    pub jahre: i64,
}

pub fn mahnwesen_ui(app: &mut App, ui: &mut Ui) {
    if app.mahn.veraltet {
        app.mahn.gruppen = app.db.mahnliste(app.mahn.jahre * 365);
        app.mahn.stufen = app.mahn.gruppen.iter().map(|g| g.vorschlag().max(1)).collect();
        app.mahn.veraltet = false;
    }
    let faellig: usize = app.mahn.gruppen.iter().filter(|g| g.vorschlag() > 0).count();
    titel(ui, "Mahnwesen", &format!("{} Kunden mit offenen Rechnungen, bei {} ist eine Mahnung fällig", app.mahn.gruppen.len(), faellig));
    ui.horizontal(|ui| {
        ui.checkbox(&mut app.mahn.nur_faellige, "nur Kunden mit fälliger Mahnung");
        ui.label("Rechnungen der letzten");
        if ui.add(egui::DragValue::new(&mut app.mahn.jahre).range(0..=40).suffix(" Jahre")).on_hover_text("0 = alle, auch sehr alte Restposten").changed() {
            app.mahn.veraltet = true;
        }
        ui.label(RichText::new(format!("Offen gesamt: {} €", money(app.mahn.gruppen.iter().map(|g| g.offen()).sum()))).strong());
    });
    ui.add_space(6.0);
    let mut aktion: Option<(usize, bool)> = None; // (Index, mit_buchen)
    let mut oeffne: Option<String> = None;
    let nur = app.mahn.nur_faellige;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (i, g) in app.mahn.gruppen.iter().enumerate() {
            if nur && g.vorschlag() == 0 {
                continue;
            }
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&g.kunde).strong().size(16.0));
                    ui.label(RichText::new(format!("· offen {} €", money(g.offen()))).color(if g.vorschlag() > 0 { ROT } else { GRAU }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("Mahnung erstellen und vermerken").strong()).on_hover_text("Erstellt das PDF und trägt Stufe und Datum bei allen aufgelisteten Rechnungen ein").clicked() {
                            aktion = Some((i, true));
                        }
                        if ui.button("Nur Vorschau (PDF)").clicked() {
                            aktion = Some((i, false));
                        }
                        egui::ComboBox::from_id_salt(("stufe", i)).selected_text(format!("Stufe {}", app.mahn.stufen[i])).show_ui(ui, |ui| {
                            for s in 1..=3 {
                                ui.selectable_value(&mut app.mahn.stufen[i], s, format!("Stufe {}", s));
                            }
                        });
                        ui.label("Mahnstufe");
                    });
                });
                egui::Grid::new(("mg", i)).striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
                    for h in ["Rechnung", "Datum", "Tage", "Rechnungsbetrag", "offen", "Mahnstufe", "fällig"] {
                        ui.label(RichText::new(h).color(GRAU));
                    }
                    ui.end_row();
                    for p in &g.posten {
                        if ui.link(p.beleg.nr.to_string()).clicked() {
                            oeffne = Some(p.beleg.id.clone());
                        }
                        ui.label(date_de(&p.beleg.datum));
                        ui.label(p.tage.to_string());
                        ui.label(format!("{} €", money(p.summen.brutto)));
                        ui.label(format!("{} €", money(p.offen)));
                        ui.label(if p.beleg.mahnstufe > 0 { format!("{}. Mahnung", p.beleg.mahnstufe) } else { "–".to_string() });
                        ui.label(if p.vorschlag > 0 { RichText::new(format!("→ {}. Mahnung", p.vorschlag)).color(ROT).strong() } else { RichText::new("noch nicht").color(GRAU) });
                        ui.end_row();
                    }
                });
            });
            ui.add_space(8.0);
        }
    });
    if let Some(id) = oeffne {
        app.page = Page::Belege(3);
        app.bel.typ = 3;
        app.bel.oeffnen(&app.db, &id);
    }
    if let Some((i, buchen)) = aktion {
        let g = app.mahn.gruppen[i].clone();
        let stufe = app.mahn.stufen[i];
        let heute = today_iso();
        let dir = data_dir().join("PDF");
        let _ = std::fs::create_dir_all(&dir);
        let name: String = g.kunde.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).take(30).collect();
        let pfad = dir.join(format!("Mahnung_{}_{}_{}.pdf", stufe, name, heute));
        match crate::pdf::mahnung_pdf(&app.db, &g, stufe, &heute, &pfad) {
            Ok(_) => {
                oeffnen(&pfad);
                if buchen {
                    let belege: Vec<Beleg> = g.posten.iter().map(|p| p.beleg.clone()).collect();
                    match app.db.mahnung_buchen(&belege, stufe, &heute) {
                        Ok(_) => {
                            app.msg(format!("{}. Mahnung für {} erstellt und bei {} Rechnung(en) vermerkt.", stufe, g.kunde, belege.len()), true);
                            app.mahn.veraltet = true;
                            app.kennzahlen_veraltet = true;
                        }
                        Err(e) => app.msg(format!("PDF erstellt, aber Vermerk fehlgeschlagen: {}", e), false),
                    }
                } else {
                    app.msg(format!("Vorschau erstellt: {}", pfad.display()), true);
                }
            }
            Err(e) => app.msg(format!("PDF-Fehler: {}", e), false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rechnung(datum: &str, stufe: i64, m1: &str, m2: &str) -> Beleg {
        Beleg { datum: datum.into(), mahnstufe: stufe, mahn1: m1.into(), mahn2: m2.into(), ..Default::default() }
    }

    #[test]
    fn fristen() {
        // 14 Tage nach Rechnungsdatum -> noch nicht, am 15. Tag -> 1. Mahnung
        assert_eq!(faellige_stufe(&rechnung("2026-01-01", 0, "", ""), "2026-01-15", 14, 8, 8), 0);
        assert_eq!(faellige_stufe(&rechnung("2026-01-01", 0, "", ""), "2026-01-16", 14, 8, 8), 1);
        // nach der 1. Mahnung 8 Tage Frist
        assert_eq!(faellige_stufe(&rechnung("2026-01-01", 1, "2026-01-20", ""), "2026-01-28", 14, 8, 8), 0);
        assert_eq!(faellige_stufe(&rechnung("2026-01-01", 1, "2026-01-20", ""), "2026-01-29", 14, 8, 8), 2);
        assert_eq!(faellige_stufe(&rechnung("2026-01-01", 2, "2026-01-20", "2026-02-01"), "2026-02-10", 14, 8, 8), 3);
    }

    #[test]
    fn liste_und_buchen() {
        let mut db = Db::memory().unwrap();
        let a = Adresse { id: "A".into(), firma: "Kunde".into(), kundennr: "7".into(), ..Default::default() };
        db.adresse_speichern(&a).unwrap();
        let mut b = db.neuer_beleg("Rechnung", Some(&a));
        b.datum = add_days_iso(&today_iso(), -40);
        let p = Position { artikel: "x".into(), menge: 1.0, vk: 100.0, mwst: 20.0, summe: 100.0, ..Default::default() };
        db.beleg_speichern(&b, &[p]).unwrap();
        let l = db.mahnliste(0);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].posten[0].vorschlag, 1);
        assert_eq!(l[0].offen(), 120.0);
        db.mahnung_buchen(&[l[0].posten[0].beleg.clone()], 1, &today_iso()).unwrap();
        let l2 = db.mahnliste(0);
        assert_eq!(l2[0].posten[0].beleg.mahnstufe, 1);
        assert_eq!(l2[0].posten[0].vorschlag, 0, "direkt nach der Mahnung ist nichts faellig");
    }
}
