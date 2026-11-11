//! Belege (Angebot, Auftrag, Lieferschein, Rechnung): Liste und Editor.

use crate::app::*;
use crate::calc::{summen, Summen};
use crate::db::*;
use crate::util::*;
use egui::{RichText, Ui};

pub const TITEL: [&str; 4] = ["Angebote", "Aufträge", "Lieferscheine", "Rechnungen"];

#[derive(Default)]
pub struct BelegState {
    pub typ: usize,
    pub suche: String,
    pub nur_offen: bool,
    pub liste: Vec<(Beleg, Summen)>,
    pub veraltet: bool,
    pub edit: Option<BelegEdit>,
    adr_suche: String,
    art_suche: String,
    loesch_frage: bool,
}

#[derive(Clone)]
pub struct PosEdit {
    p: Position,
    menge: String,
    vk: String,
    mwst: String,
    o_menge: String,
    o_vk: String,
}

impl PosEdit {
    fn neu(p: Position) -> Self {
        let (menge, vk, mwst) = (num_edit(p.menge), num_edit(p.vk), num_edit(p.mwst));
        PosEdit { p, o_menge: menge.clone(), o_vk: vk.clone(), menge, vk, mwst }
    }
    /// Zeilensumme: unveraenderte importierte Positionen behalten ihre Originalsumme.
    fn summe(&self) -> f64 {
        if self.menge == self.o_menge && self.vk == self.o_vk {
            self.p.summe
        } else {
            round2(parse_num(&self.menge) * parse_num(&self.vk))
        }
    }
}

pub struct BelegEdit {
    pub b: Beleg,
    pos: Vec<PosEdit>,
    datum: String,
    gueltig: String,
    leistung: String,
    zahldatum: String,
    rabatt: String,
    o_rabatt: String,
    zahlbetrag: String,
    pub neu: bool,
}

impl BelegEdit {
    fn aus(b: Beleg, pos: Vec<Position>, neu: bool) -> Self {
        let rabatt = num_edit(b.rabatt_prozent);
        BelegEdit {
            datum: date_de(&b.datum),
            gueltig: date_de(&b.gueltig_bis),
            leistung: date_de(&b.leistungsdatum),
            zahldatum: date_de(&b.zahldatum),
            o_rabatt: rabatt.clone(),
            rabatt,
            zahlbetrag: String::new(),
            pos: pos.into_iter().map(PosEdit::neu).collect(),
            b,
            neu,
        }
    }

    fn netto_positionen(&self) -> f64 {
        self.pos.iter().map(|p| p.summe()).sum()
    }
    fn rabatt_betrag(&self) -> f64 {
        if self.rabatt == self.o_rabatt {
            self.b.rabatt_betrag
        } else {
            round2(self.netto_positionen() * parse_num(&self.rabatt) / 100.0)
        }
    }
    fn summen(&self) -> Summen {
        let je: Vec<(f64, f64)> = self.pos.iter().map(|p| (parse_num(&p.mwst), p.summe())).collect();
        summen(&je, self.rabatt_betrag())
    }

    /// Liest alle Eingabefelder in Beleg und Positionen zurueck.
    fn zusammen(&self) -> Result<(Beleg, Vec<Position>), String> {
        let mut b = self.b.clone();
        b.datum = date_input(&self.datum);
        if b.datum.is_empty() {
            return Err("Das Datum ist ungültig (Format TT.MM.JJJJ).".into());
        }
        for (text, ziel, name) in [(&self.gueltig, &mut b.gueltig_bis, "Gültig bis"), (&self.leistung, &mut b.leistungsdatum, "Leistungsdatum"), (&self.zahldatum, &mut b.zahldatum, "Zahldatum")] {
            if text.trim().is_empty() {
                ziel.clear();
            } else {
                let d = date_input(text);
                if d.is_empty() {
                    return Err(format!("{} ist ungültig (Format TT.MM.JJJJ).", name));
                }
                *ziel = d;
            }
        }
        b.rabatt_betrag = self.rabatt_betrag();
        b.rabatt_prozent = parse_num(&self.rabatt);
        if b.rabatt_betrag != 0.0 && b.rabatt_text.is_empty() {
            b.rabatt_text = format!("Auf alle oben genannten Positionen gewähren wir Ihnen {}% Rabatt", num_edit(b.rabatt_prozent));
        }
        let pos = self
            .pos
            .iter()
            .map(|pe| {
                let mut p = pe.p.clone();
                p.menge = parse_num(&pe.menge);
                p.vk = parse_num(&pe.vk);
                p.mwst = parse_num(&pe.mwst);
                p.summe = pe.summe();
                p
            })
            .collect();
        Ok((b, pos))
    }
}

impl BelegState {
    pub fn neu_fuer(&mut self, db: &Db, adresse: Option<&Adresse>) {
        let b = db.neuer_beleg(TYPEN[self.typ], adresse);
        self.edit = Some(BelegEdit::aus(b, Vec::new(), true));
        self.adr_suche.clear();
        self.art_suche.clear();
        self.loesch_frage = false;
    }
    pub fn oeffnen(&mut self, db: &Db, id: &str) {
        if let Some(b) = db.beleg(id) {
            let pos = db.positionen(&b.id);
            self.edit = Some(BelegEdit::aus(b, pos, false));
            self.adr_suche.clear();
            self.art_suche.clear();
            self.loesch_frage = false;
        }
    }
}

pub fn belege_ui(app: &mut App, ui: &mut Ui) {
    if app.bel.edit.is_some() {
        editor_ui(app, ui);
    } else {
        liste_ui(app, ui);
    }
}

// ---------------------------------------------------------------------------------- Liste
fn liste_ui(app: &mut App, ui: &mut Ui) {
    let typ = TYPEN[app.bel.typ];
    if app.bel.veraltet {
        app.bel.liste = app.db.belege(typ, &app.bel.suche, app.bel.nur_offen);
        app.bel.veraltet = false;
    }
    titel(ui, TITEL[app.bel.typ], &format!("{} Belege", app.bel.liste.len()));
    let mut neu = false;
    let mut oeffne: Option<String> = None;
    ui.horizontal(|ui| {
        if ui.add(egui::TextEdit::singleline(&mut app.bel.suche).hint_text("Suchen (Nr., Kunde, Betreff …)").desired_width(300.0)).changed() {
            app.bel.veraltet = true;
        }
        if ui.checkbox(&mut app.bel.nur_offen, "nur offene").changed() {
            app.bel.veraltet = true;
        }
        if ui.button(RichText::new(format!("+ Neu: {}", typ)).strong()).clicked() {
            neu = true;
        }
    });
    ui.add_space(6.0);
    card(ui, |ui| {
        use egui_extras::{Column, TableBuilder};
        let n = app.bel.liste.len();
        let summe_netto: f64 = app.bel.liste.iter().map(|(_, s)| s.netto).sum();
        let summe_brutto: f64 = app.bel.liste.iter().map(|(_, s)| s.brutto).sum();
        let hoehe = (ui.available_height() - 40.0).max(120.0);
        TableBuilder::new(ui)
            .id_salt("belegliste")
            .striped(true)
            .resizable(true)
            .max_scroll_height(hoehe)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(70.0))
            .column(Column::initial(90.0))
            .column(Column::initial(280.0).clip(true))
            .column(Column::initial(280.0).clip(true))
            .column(Column::initial(100.0))
            .column(Column::initial(100.0))
            .column(Column::remainder())
            .header(24.0, |mut h| {
                for t in ["Nr.", "Datum", "Kunde", "Betreff", "Status", "Netto", "Brutto"] {
                    h.col(|ui| {
                        ui.label(RichText::new(t).strong().color(GRAU));
                    });
                }
            })
            .body(|body| {
                body.rows(24.0, n, |mut row| {
                    let (b, sm) = &app.bel.liste[row.index()];
                    row.col(|ui| {
                        if ui.link(b.nr.to_string()).clicked() {
                            oeffne = Some(b.id.clone());
                        }
                    });
                    row.col(|ui| {
                        ui.label(date_de(&b.datum));
                    });
                    row.col(|ui| {
                        ui.label(b.adressfeld.lines().next().unwrap_or("").to_string());
                    });
                    row.col(|ui| {
                        ui.label(b.betreff.clone());
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(&b.status).color(status_farbe(&b.status)));
                    });
                    row.col(|ui| {
                        ui.label(money(sm.netto));
                    });
                    row.col(|ui| {
                        ui.label(money(sm.brutto));
                    });
                });
            });
        ui.separator();
        ui.label(RichText::new(format!("Summe der angezeigten Belege: netto {} €  ·  brutto {} €", money(summe_netto), money(summe_brutto))).strong());
    });
    if neu {
        let t = app.bel.typ;
        app.bel.typ = t;
        app.bel.neu_fuer(&app.db, None);
    }
    if let Some(id) = oeffne {
        app.bel.oeffnen(&app.db, &id);
    }
}

// ---------------------------------------------------------------------------------- Editor
enum Ed {
    Zurueck,
    Speichern,
    Pdf,
    Mail,
    XRechnung,
    Folge(&'static str),
    Kopieren,
    Loeschen,
    LoeschenBestaetigt,
    Adresse(Adresse),
    Artikel(Artikel),
    PosNeu,
    PosWeg(usize),
    PosHoch(usize),
    PosRunter(usize),
    Zahlung,
    Mahnung,
}

const STATI: [&[&str]; 4] = [&["offen", "angenommen", "abgerechnet", "abgelehnt"], &["offen", "abgerechnet"], &["offen", "abgerechnet"], &["offen", "teilbezahlt", "bezahlt"]];

fn folge_typen(typ: &str) -> Vec<&'static str> {
    match typ {
        "Angebot" => vec!["Auftrag", "Lieferschein", "Rechnung"],
        "Auftrag" => vec!["Lieferschein", "Rechnung"],
        "Lieferschein" => vec!["Rechnung"],
        _ => vec![],
    }
}

fn editor_ui(app: &mut App, ui: &mut Ui) {
    let typ_idx = app.bel.typ;
    let typ = TYPEN[typ_idx];
    let adr_treffer: Vec<Adresse> = if app.bel.adr_suche.trim().len() >= 2 { app.db.adressen(&app.bel.adr_suche).into_iter().take(7).collect() } else { Vec::new() };
    let art_treffer: Vec<Artikel> = if app.bel.art_suche.trim().len() >= 1 { app.db.artikel_liste(&app.bel.art_suche).into_iter().take(7).collect() } else { Vec::new() };
    let mut aktion: Option<Ed> = None;
    let st = &mut app.bel;
    let frage = st.loesch_frage;
    let ed = match st.edit.as_mut() {
        Some(e) => e,
        None => return,
    };
    let sm = ed.summen();

    // ---- Kopfzeile
    ui.horizontal(|ui| {
        if ui.button("← Liste").clicked() {
            aktion = Some(Ed::Zurueck);
        }
        ui.label(RichText::new(format!("{} Nr. {}", typ, ed.b.nr)).size(22.0).strong().color(NAVY));
        if ed.neu {
            ui.label(RichText::new("(neu, noch nicht gespeichert)").color(GRAU));
        } else {
            ui.label(RichText::new(&ed.b.status).color(status_farbe(&ed.b.status)).strong());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if frage {
                if ui.button(RichText::new("Ja, endgültig löschen").color(ROT).strong()).clicked() {
                    aktion = Some(Ed::LoeschenBestaetigt);
                }
                ui.label(RichText::new("Beleg wirklich löschen?").color(ROT));
            } else {
                if !ed.neu && ui.button(RichText::new("Löschen").color(ROT)).clicked() {
                    aktion = Some(Ed::Loeschen);
                }
                if !ed.neu && ui.button("Kopieren").on_hover_text(format!("Diesen {} als neuen {} mit nächster Nr. duplizieren (Positionen und Adresse werden übernommen und sind änderbar)", typ, typ)).clicked() {
                    aktion = Some(Ed::Kopieren);
                }
                for t in folge_typen(typ).into_iter().rev() {
                    if ui.button(format!("→ {}", t)).on_hover_text(format!("Aus diesem Beleg einen {} erstellen (Positionen werden übernommen)", t)).clicked() {
                        aktion = Some(Ed::Folge(t));
                    }
                }
                if typ == "Rechnung" && ui.button("E-Rechnung").on_hover_text("XRechnung (XML, UBL 2.1) erzeugen").clicked() {
                    aktion = Some(Ed::XRechnung);
                }
                if ui.button("E-Mail").on_hover_text("PDF erstellen und neue Nachricht im Mailprogramm öffnen").clicked() {
                    aktion = Some(Ed::Mail);
                }
                if ui.button("PDF").clicked() {
                    aktion = Some(Ed::Pdf);
                }
                if ui.button(RichText::new("Speichern").strong()).clicked() {
                    aktion = Some(Ed::Speichern);
                }
            }
        });
    });
    ui.add_space(6.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        // ---- Kunde und Kopfdaten
        card(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(440.0);
                    ui.label(RichText::new("Kunde").strong());
                    ui.add(egui::TextEdit::singleline(&mut st.adr_suche).hint_text("Adresse suchen und übernehmen …").desired_width(420.0));
                    for a in &adr_treffer {
                        let t = format!("{} · {} {}", a.name(), a.plz, a.ort);
                        if ui.selectable_label(false, t).clicked() {
                            aktion = Some(Ed::Adresse(a.clone()));
                        }
                    }
                    ui.label("Adressfeld");
                    ui.add(egui::TextEdit::multiline(&mut ed.b.adressfeld).desired_rows(5).desired_width(420.0));
                });
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    egui::Grid::new("kopfdaten").num_columns(2).spacing([12.0, 7.0]).show(ui, |ui| {
                        feld(ui, "Datum", &mut ed.datum, 120.0);
                        ui.end_row();
                        feld(ui, "Kundennr.", &mut ed.b.kundennr, 120.0);
                        ui.end_row();
                        if typ == "Angebot" {
                            feld(ui, "Gültig bis", &mut ed.gueltig, 120.0);
                            ui.end_row();
                        }
                        if typ == "Rechnung" {
                            feld(ui, "Leistungsdatum", &mut ed.leistung, 120.0);
                            ui.end_row();
                        }
                        ui.label("Status");
                        egui::ComboBox::from_id_salt("status").selected_text(ed.b.status.clone()).show_ui(ui, |ui| {
                            for s in STATI[typ_idx] {
                                ui.selectable_value(&mut ed.b.status, s.to_string(), *s);
                            }
                        });
                        ui.end_row();
                        let herkunft = [("Angebot", &ed.b.aus_angebot), ("Auftrag", &ed.b.aus_auftrag), ("Lieferschein", &ed.b.aus_lieferschein)];
                        let text: Vec<String> = herkunft.iter().filter(|(t, v)| !v.is_empty() && *t != typ).map(|(t, v)| format!("{} {}", t, v)).collect();
                        if !text.is_empty() {
                            ui.label("Aus");
                            ui.label(text.join(", "));
                            ui.end_row();
                        }
                        let folge = [("Auftrag", &ed.b.zu_auftrag), ("Lieferschein", &ed.b.zu_lieferschein), ("Rechnung", &ed.b.zu_rechnung)];
                        let text: Vec<String> = folge.iter().filter(|(_, v)| !v.is_empty()).map(|(t, v)| format!("{} {}", t, v)).collect();
                        if !text.is_empty() {
                            ui.label("Wurde zu");
                            ui.label(text.join(", "));
                            ui.end_row();
                        }
                    });
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Betreff");
                ui.add(egui::TextEdit::singleline(&mut ed.b.betreff).desired_width(560.0));
            });
            ui.horizontal(|ui| {
                ui.label("Anrede");
                ui.add(egui::TextEdit::singleline(&mut ed.b.briefanrede).desired_width(560.0));
            });
            ui.label("Einleitungstext");
            ui.add(egui::TextEdit::multiline(&mut ed.b.kopftext).desired_rows(3).desired_width(f32::INFINITY));
        });
        ui.add_space(10.0);

        // ---- Positionen
        card(ui, |ui| {
            ui.label(RichText::new("Positionen").strong().size(16.0));
            ui.add_space(4.0);
            let n = ed.pos.len();
            for i in 0..n {
                let pe = &mut ed.pos[i];
                egui::Frame::new().fill(egui::Color32::from_rgb(248, 250, 252)).stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(226, 232, 240))).corner_radius(egui::CornerRadius::same(8)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}.", i + 1)).strong().color(GRAU));
                        ui.add(egui::TextEdit::singleline(&mut pe.p.artikel).hint_text("Artikel / Bezeichnung").desired_width(320.0));
                        ui.label("Menge");
                        ui.add(egui::TextEdit::singleline(&mut pe.menge).desired_width(60.0));
                        ui.add(egui::TextEdit::singleline(&mut pe.p.einheit).hint_text("Einheit").desired_width(62.0));
                        ui.label("à");
                        ui.add(egui::TextEdit::singleline(&mut pe.vk).desired_width(80.0));
                        ui.label("€  USt.");
                        ui.add(egui::TextEdit::singleline(&mut pe.mwst).desired_width(40.0));
                        ui.label("%");
                        ui.label(RichText::new(format!("= {} €", money(pe.summe()))).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(RichText::new("✕").color(ROT)).clicked() {
                                aktion = Some(Ed::PosWeg(i));
                            }
                            if ui.small_button("↓").clicked() {
                                aktion = Some(Ed::PosRunter(i));
                            }
                            if ui.small_button("↑").clicked() {
                                aktion = Some(Ed::PosHoch(i));
                            }
                        });
                    });
                    let zeilen = if pe.p.beschreibung.trim().is_empty() { 1 } else { 2 };
                    ui.add(egui::TextEdit::multiline(&mut pe.p.beschreibung).hint_text("Beschreibung (optional)").desired_rows(zeilen).desired_width(f32::INFINITY));
                });
                ui.add_space(4.0);
            }
            ui.horizontal(|ui| {
                if ui.button("+ Leere Position").clicked() {
                    aktion = Some(Ed::PosNeu);
                }
                ui.add(egui::TextEdit::singleline(&mut st.art_suche).hint_text("Artikel aus Stamm einfügen …").desired_width(260.0));
            });
            for a in &art_treffer {
                if ui.selectable_label(false, format!("{}  {}  ·  {} €/{}", a.nr, a.name, money(a.vk), a.einheit)).clicked() {
                    aktion = Some(Ed::Artikel(a.clone()));
                }
            }
        });
        ui.add_space(10.0);

        // ---- Rabatt, Schlusstext, Summen
        ui.horizontal_top(|ui| {
            card(ui, |ui| {
                ui.set_width(560.0);
                ui.horizontal(|ui| {
                    ui.label("Rabatt auf alle Positionen in %");
                    ui.add(egui::TextEdit::singleline(&mut ed.rabatt).desired_width(70.0));
                });
                ui.label("Schlusstext / Grußformel");
                ui.add(egui::TextEdit::multiline(&mut ed.b.fusstext).desired_rows(4).desired_width(f32::INFINITY));
                ui.label("Interne Notizen");
                ui.add(egui::TextEdit::multiline(&mut ed.b.notizen).desired_rows(2).desired_width(f32::INFINITY));
            });
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.set_width(360.0);
                let w = if ed.b.waehrung.is_empty() { "€".to_string() } else { ed.b.waehrung.clone() };
                let zeile = |ui: &mut Ui, k: &str, v: f64, fett: bool| {
                    ui.horizontal(|ui| {
                        ui.label(if fett { RichText::new(k).strong() } else { RichText::new(k) });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let t = format!("{} {}", money(v), w);
                            ui.label(if fett { RichText::new(t).strong().size(17.0) } else { RichText::new(t) });
                        });
                    });
                };
                if sm.rabatt != 0.0 {
                    zeile(ui, "Summe Positionen", sm.positionen, false);
                    zeile(ui, "Rabatt", -sm.rabatt, false);
                }
                zeile(ui, "Gesamt netto", sm.netto, false);
                for (satz, betrag) in &sm.mwst {
                    zeile(ui, &format!("zzgl. {}% USt.", num_edit(*satz)), *betrag, false);
                }
                ui.separator();
                zeile(ui, "Gesamtbetrag brutto", sm.brutto, true);
                if typ == "Rechnung" {
                    ui.separator();
                    zeile(ui, "bereits bezahlt", ed.b.bezahlt, false);
                    zeile(ui, "offener Betrag", sm.brutto - ed.b.bezahlt, true);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Zahlung");
                        ui.add(egui::TextEdit::singleline(&mut ed.zahlbetrag).hint_text("Betrag").desired_width(90.0));
                        ui.add(egui::TextEdit::singleline(&mut ed.zahldatum).hint_text("Datum").desired_width(90.0));
                        if ui.button("buchen").clicked() {
                            aktion = Some(Ed::Zahlung);
                        }
                    });
                    ui.horizontal(|ui| {
                        let stufe = ed.b.mahnstufe;
                        ui.label(format!("Mahnstufe {}", stufe));
                        for (i, d) in [&ed.b.mahn1, &ed.b.mahn2, &ed.b.mahn3].iter().enumerate() {
                            if !d.is_empty() {
                                ui.label(RichText::new(format!("{}. Mahnung {}", i + 1, date_de(d))).color(GRAU).size(12.0));
                            }
                        }
                        if stufe < 3 && ui.small_button("Mahnung vermerken").clicked() {
                            aktion = Some(Ed::Mahnung);
                        }
                    });
                }
            });
        });
        ui.add_space(20.0);
    });

    if let Some(a) = aktion {
        ed_aktion(app, a);
    }
}

fn ed_aktion(app: &mut App, a: Ed) {
    match a {
        Ed::Zurueck => {
            app.bel.edit = None;
            app.bel.veraltet = true;
        }
        Ed::PosNeu => {
            if let Some(ed) = app.bel.edit.as_mut() {
                ed.pos.push(PosEdit::neu(Position { mwst: 20.0, einheit: "Stk.".into(), menge: 1.0, ..Default::default() }));
            }
        }
        Ed::PosWeg(i) => {
            if let Some(ed) = app.bel.edit.as_mut() {
                if i < ed.pos.len() {
                    ed.pos.remove(i);
                }
            }
        }
        Ed::PosHoch(i) => {
            if let Some(ed) = app.bel.edit.as_mut() {
                if i > 0 && i < ed.pos.len() {
                    ed.pos.swap(i, i - 1);
                }
            }
        }
        Ed::PosRunter(i) => {
            if let Some(ed) = app.bel.edit.as_mut() {
                if i + 1 < ed.pos.len() {
                    ed.pos.swap(i, i + 1);
                }
            }
        }
        Ed::Artikel(a) => {
            if let Some(ed) = app.bel.edit.as_mut() {
                let p = Position { artikel: a.name.clone(), beschreibung: a.beschreibung.clone(), artikel_nr: a.nr.clone(), menge: 1.0, einheit: a.einheit.clone(), ek: a.ek, vk: a.vk, mwst: if a.mwst != 0.0 { a.mwst } else { 20.0 }, summe: a.vk, ..Default::default() };
                ed.pos.push(PosEdit::neu(p));
            }
            app.bel.art_suche.clear();
        }
        Ed::Adresse(a) => {
            if let Some(ed) = app.bel.edit.as_mut() {
                app.db.adresse_uebernehmen(&mut ed.b, &a);
            }
            app.bel.adr_suche.clear();
        }
        Ed::Loeschen => app.bel.loesch_frage = true,
        Ed::LoeschenBestaetigt => {
            app.bel.loesch_frage = false;
            if let Some(ed) = app.bel.edit.take() {
                match app.db.beleg_loeschen(&ed.b.id) {
                    Ok(_) => app.msg(format!("{} Nr. {} wurde gelöscht.", ed.b.typ, ed.b.nr), true),
                    Err(e) => app.msg(format!("Löschen fehlgeschlagen: {}", e), false),
                }
                app.bel.veraltet = true;
            }
        }
        Ed::Speichern => {
            speichern(app);
        }
        Ed::Pdf => {
            if let Some(b) = speichern(app) {
                if let Some(pfad) = pdf_erstellen(app, &b) {
                    // Kopie auf den Desktop legen, von dort oeffnen.
                    let (ziel, meldung) = match desktop_dir().and_then(|d| pfad.file_name().map(|n| d.join(n))) {
                        Some(d) => match std::fs::copy(&pfad, &d) {
                            Ok(_) => (d.clone(), format!("PDF auf Desktop gespeichert: {}", d.display())),
                            Err(e) => (pfad.clone(), format!("PDF erstellt: {} (Desktop-Kopie fehlgeschlagen: {})", pfad.display(), e)),
                        },
                        None => (pfad.clone(), format!("PDF erstellt: {}", pfad.display())),
                    };
                    app.msg(meldung, true);
                    oeffnen(&ziel);
                }
            }
        }
        Ed::Mail => {
            if let Some(b) = speichern(app) {
                if let Some(pfad) = pdf_erstellen(app, &b) {
                    let an = app.db.adresse(&b.adresse_id).map(|a| a.email).unwrap_or_default();
                    let (betreff, text) = crate::versand::mailtext(&app.db, &b);
                    crate::versand::mailprogramm_oeffnen(&crate::versand::mailto_url(&an, &betreff, &text));
                    crate::versand::datei_anzeigen(&pfad);
                    let hinweis = if an.trim().is_empty() { " (beim Kunden ist keine E-Mail-Adresse hinterlegt)" } else { "" };
                    app.msg(format!("Mail vorbereitet{} – das PDF liegt markiert im Ordner und muss nur noch an die Mail angehängt werden.", hinweis), true);
                }
            }
        }
        Ed::XRechnung => {
            if let Some(b) = speichern(app) {
                let pos = app.db.positionen(&b.id);
                match crate::erechnung::xrechnung_xml(&app.db, &b, &pos) {
                    Ok(xml) => {
                        let dir = data_dir().join("PDF");
                        let _ = std::fs::create_dir_all(&dir);
                        let pfad = dir.join(format!("Rechnung_{}_XRechnung.xml", b.nr));
                        match std::fs::write(&pfad, xml) {
                            Ok(_) => {
                                crate::versand::datei_anzeigen(&pfad);
                                app.msg(format!("XRechnung erstellt: {}", pfad.display()), true);
                            }
                            Err(e) => app.msg(format!("Datei konnte nicht geschrieben werden: {}", e), false),
                        }
                    }
                    Err(e) => app.msg(e, false),
                }
            }
        }
        Ed::Folge(neuer_typ) => {
            if let Some(b) = speichern(app) {
                match app.db.folgebeleg(&b, neuer_typ) {
                    Ok(neu) => {
                        let idx = TYPEN.iter().position(|t| *t == neuer_typ).unwrap_or(3);
                        app.page = Page::Belege(idx);
                        app.bel.typ = idx;
                        app.bel.oeffnen(&app.db, &neu.id);
                        app.bel.veraltet = true;
                        app.msg(format!("{} Nr. {} wurde aus {} Nr. {} erstellt.", neuer_typ, neu.nr, b.typ, b.nr), true);
                    }
                    Err(e) => app.msg(format!("Fehler: {}", e), false),
                }
            }
        }
        Ed::Kopieren => {
            if let Some(b) = speichern(app) {
                match app.db.beleg_kopieren(&b) {
                    Ok(neu) => {
                        app.bel.oeffnen(&app.db, &neu.id);
                        app.bel.veraltet = true;
                        app.msg(format!("{} Nr. {} wurde als Kopie von Nr. {} angelegt – Adresse und Positionen können jetzt angepasst werden.", neu.typ, neu.nr, b.nr), true);
                    }
                    Err(e) => app.msg(format!("Kopieren fehlgeschlagen: {}", e), false),
                }
            }
        }
        Ed::Zahlung => {
            if let Some(b0) = speichern(app) {
                let pos = app.db.positionen(&b0.id);
                let brutto = app.db.summen_von(&b0, &pos).brutto;
                let (betrag, datum) = match app.bel.edit.as_ref() {
                    Some(ed) => (parse_num(&ed.zahlbetrag), date_input(&ed.zahldatum)),
                    None => return,
                };
                if betrag == 0.0 {
                    app.msg("Bitte den Zahlungsbetrag eingeben.", false);
                    return;
                }
                let datum = if datum.is_empty() { today_iso() } else { datum };
                let mut b = b0;
                match app.db.zahlung_buchen(&mut b, brutto, betrag, &datum) {
                    Ok(_) => {
                        app.msg(format!("Zahlung {} € gebucht – Status: {}.", money(betrag), b.status), true);
                        app.bel.oeffnen(&app.db, &b.id);
                        app.bel.veraltet = true;
                    }
                    Err(e) => app.msg(format!("Fehler: {}", e), false),
                }
            }
        }
        Ed::Mahnung => {
            if let Some(ed) = app.bel.edit.as_mut() {
                ed.b.mahnstufe = (ed.b.mahnstufe + 1).min(3);
                let heute = today_iso();
                match ed.b.mahnstufe {
                    1 => ed.b.mahn1 = heute,
                    2 => ed.b.mahn2 = heute,
                    _ => ed.b.mahn3 = heute,
                }
                ed.gueltig = date_de(&ed.b.gueltig_bis);
            }
            if let Some(b) = speichern(app) {
                app.msg(format!("Mahnstufe {} vermerkt.", b.mahnstufe), true);
            }
        }
    }
}

/// Speichert den geoeffneten Beleg; liest ihn danach neu aus der Datenbank. Gibt den gespeicherten Beleg zurueck.
fn speichern(app: &mut App) -> Option<Beleg> {
    let (b, pos) = match app.bel.edit.as_ref()?.zusammen() {
        Ok(x) => x,
        Err(e) => {
            app.msg(e, false);
            return None;
        }
    };
    if b.adressfeld.trim().is_empty() {
        app.msg("Bitte zuerst einen Kunden wählen oder das Adressfeld ausfüllen.", false);
        return None;
    }
    match app.db.beleg_speichern(&b, &pos) {
        Ok(_) => {
            let id = b.id.clone();
            let zahlbetrag = app.bel.edit.as_ref().map(|e| (e.zahlbetrag.clone(), e.zahldatum.clone())).unwrap_or_default();
            app.bel.oeffnen(&app.db, &id);
            if let Some(ed) = app.bel.edit.as_mut() {
                ed.zahlbetrag = zahlbetrag.0;
                if ed.b.zahldatum.is_empty() {
                    ed.zahldatum = zahlbetrag.1;
                }
            }
            app.bel.veraltet = true;
            app.msg(format!("{} Nr. {} gespeichert.", b.typ, b.nr), true);
            app.db.beleg(&id)
        }
        Err(e) => {
            app.msg(format!("Speichern fehlgeschlagen: {}", e), false);
            None
        }
    }
}

/// Erstellt das PDF des Belegs im PDF-Ordner und gibt den Pfad zurueck.
fn pdf_erstellen(app: &mut App, b: &Beleg) -> Option<std::path::PathBuf> {
    let pos = app.db.positionen(&b.id);
    let sm = app.db.summen_von(b, &pos);
    let dir = data_dir().join("PDF");
    let _ = std::fs::create_dir_all(&dir);
    let pfad = dir.join(format!("{}_{}.pdf", b.typ, b.nr));
    match crate::pdf::beleg_pdf(&app.db, b, &pos, &sm, &pfad) {
        Ok(_) => Some(pfad),
        Err(e) => {
            app.msg(format!("PDF-Fehler: {}", e), false);
            None
        }
    }
}
