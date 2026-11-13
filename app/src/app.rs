//! Hauptfenster: Navigation, Startseite, Adressen, Artikel, Einstellungen.

use crate::belege::BelegState;
use crate::calc::Summen;
use crate::db::*;
use crate::util::*;
use egui::{Color32, CornerRadius, FontId, RichText, Stroke, Ui};

pub const NAVY: Color32 = Color32::from_rgb(15, 23, 42);
pub const NAVY2: Color32 = Color32::from_rgb(30, 41, 59);
pub const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(219, 234, 254);
pub const BG: Color32 = Color32::from_rgb(241, 245, 249);
pub const GRAU: Color32 = Color32::from_rgb(100, 116, 139);
pub const ROT: Color32 = Color32::from_rgb(185, 28, 28);
pub const GRUEN: Color32 = Color32::from_rgb(21, 128, 61);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Page {
    Start,
    Adressen,
    Artikel,
    Belege(usize),
    Mahnwesen,
    Einstellungen,
}

#[derive(Default)]
struct AdressenState {
    suche: String,
    liste: Vec<Adresse>,
    edit: Option<Adresse>,
    neu: bool,
    belege: Vec<(Beleg, Summen)>,
    veraltet: bool,
}

#[derive(Default)]
struct ArtikelState {
    suche: String,
    liste: Vec<Artikel>,
    edit: Option<Artikel>,
    ek: String,
    vk: String,
    mwst: String,
    neu: bool,
    veraltet: bool,
}

struct EinstellState {
    werte: std::collections::BTreeMap<String, String>,
    import_pfad: String,
    geladen: bool,
}

pub struct App {
    pub db: Db,
    pub page: Page,
    pub meldung: Option<(String, bool)>,
    kennzahlen: Kennzahlen,
    pub kennzahlen_veraltet: bool,
    pub mahn: crate::mahn::MahnState,
    offene: Vec<(Beleg, Summen)>,
    adr: AdressenState,
    art: ArtikelState,
    pub bel: BelegState,
    set: EinstellState,
    bild_cache: std::collections::HashMap<String, (std::time::SystemTime, egui::TextureHandle)>,
    about_offen: bool,
    shot: Option<String>,
    frames: u32,
    shot_angefordert: bool,
    zoom: f32,
}

/// Felder der Einstellungsseite: (Bezeichnung, Schluessel, mehrzeilig, Platzhalter-Beispiel)
const FIRMA: [(&str, &str, bool, &str); 13] = [
    ("Firma", "Firma", false, "z. B. RC design GmbH"),
    ("Straße", "Firma_Strasse", false, "z. B. Hauptstraße 12"),
    ("PLZ", "Firma_PLZ", false, "z. B. 8010"),
    ("Ort", "Firma_Ort", false, "z. B. Graz"),
    ("Telefon", "Firma_Telefon", false, "z. B. +43 316 123456"),
    ("E-Mail", "Firma_Email", false, "z. B. office@rc-design.at"),
    ("Internet", "Firma_Internet", false, "z. B. www.rc-design.at"),
    ("UID-Nr.", "Firma_UStID", false, "z. B. ATU12345678"),
    ("Bank", "Firma_Bank", false, "z. B. Raiffeisenbank Graz"),
    ("IBAN", "Firma_IBAN", false, "z. B. AT12 3456 7890 1234 5678"),
    ("BIC", "Firma_BIC", false, "z. B. RZSTAT2G"),
    ("Zahlungsempfänger im QR-Code", "QR_Name", false, "Name wie auf dem Kontoauszug"),
    ("Währung", "Währung", false, "EUR"),
];
const TEXTE: [(&str, &str, bool, &str); 10] = [
    ("Briefkopf (rechts oben)", "Briefkopftext", true, "Firmenname\nStraße\nPLZ Ort\nTel …\nE-Mail …"),
    ("Absenderzeile im Adressfenster", "Adressfenstertext", false, "z. B. RC design · Hauptstraße 12 · 8010 Graz"),
    ("Grußformel", "Brieffloskel", false, "z. B. Mit freundlichen Grüßen"),
    ("Einleitung Angebot", "Text Einleitung ANG", true, "Sehr geehrte Damen und Herren,\nwir danken für Ihre Anfrage und bieten Ihnen wie folgt an:"),
    ("Einleitung Auftrag", "Text Einleitung AUF", true, "Sehr geehrte Damen und Herren,\nwir bestätigen den Auftrag wie folgt:"),
    ("Einleitung Lieferschein", "Text Einleitung LFS", true, "Wir liefern Ihnen folgende Positionen:"),
    ("Einleitung Rechnung", "Text Einleitung RNG", true, "Wir erlauben uns, Ihnen die erbrachten Leistungen wie folgt in Rechnung zu stellen:"),
    ("Zahlungsbedingung", "Zahlungsbedingung", true, "Zahlbar innerhalb von 14 Tagen netto ohne Abzug."),
    ("Angebotsfrist (Wochen)", "Angebotsfrist", false, "z. B. 4"),
    ("Standard-Anrede", "Anrede", false, "z. B. Sehr geehrte Damen und Herren,"),
];
const MAHN: [(&str, &str, bool, &str); 10] = [
    ("Tage bis zur 1. Mahnung (nach Rechnungsdatum)", "Mahnziel 1", false, "z. B. 30"),
    ("Frist nach 1. Mahnung / Zahlungsfrist (Tage)", "Mahnziel 2", false, "z. B. 14"),
    ("Frist nach 2. Mahnung / Zahlungsfrist (Tage)", "Mahnziel 3", false, "z. B. 10"),
    ("Mahngebühr 2. Mahnung", "Mahngebühr 2", false, "z. B. 5,00"),
    ("Mahngebühr 3. Mahnung", "Mahngebühr 3", false, "z. B. 10,00"),
    ("Verzugszinsen 2. Mahnung (% p. a.)", "Verzugszinsen 2", false, "z. B. 4"),
    ("Verzugszinsen 3. Mahnung (% p. a.)", "Verzugszinsen 3", false, "z. B. 8"),
    ("Text 1. Mahnung", "Text 1. Mahnung", true, "Sehr geehrte Damen und Herren,\nvermutlich ist es Ihrer Aufmerksamkeit entgangen, dass die unten angeführte Rechnung noch offen ist. Wir bitten um baldige Überweisung."),
    ("Text 2. Mahnung", "Text 2. Mahnung", true, "Sehr geehrte Damen und Herren,\ntrotz unserer Zahlungserinnerung ist bisher kein Zahlungseingang zu verzeichnen. Wir ersuchen Sie, den offenen Betrag umgehend zu begleichen."),
    ("Text 3. Mahnung", "Text 3. Mahnung", true, "Sehr geehrte Damen und Herren,\ntrotz zweimaliger Mahnung ist die Rechnung weiterhin offen. Sollten wir bis zum genannten Termin keine Zahlung erhalten, sehen wir uns gezwungen, weitere rechtliche Schritte einzuleiten."),
];
const ZAEHLER: [(&str, &str, &str); 5] = [
    ("Letzte Angebotsnummer", "Serien Nr. Angebot", "z. B. 2025001"),
    ("Letzte Auftragsnummer", "Serien Nr. Auftrag", "z. B. 2025001"),
    ("Letzte Lieferscheinnummer", "Serien Nr. Lieferschein", "z. B. 2025001"),
    ("Letzte Rechnungsnummer", "Serien Nr. Rechnung", "z. B. 2025001"),
    ("Letzte Kundennummer", "Kundennummer", "z. B. 1000"),
];

pub fn theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.panel_fill = BG;
    v.window_fill = Color32::WHITE;
    // Eingabefelder (TextEdit) haben einen sanft graublauen Hintergrund, damit sie sich
    // klar von den weissen Karten abheben - optischer Tiefeneffekt ohne echten Schatten.
    v.extreme_bg_color = Color32::from_rgb(248, 250, 252);
    v.faint_bg_color = Color32::from_rgb(248, 250, 252);
    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke = Stroke::new(1.0, NAVY);
    let r = CornerRadius::same(6);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.corner_radius = r;
    v.widgets.open.corner_radius = r;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(226, 232, 240));
    // Feiner, deutlich sichtbarer Rahmen - slate-300 normal, slate-500 bei Hover, Akzentblau bei Fokus
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(203, 213, 225));
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(100, 116, 139));
    v.widgets.active.bg_stroke = Stroke::new(1.5, ACCENT);
    v.widgets.open.bg_stroke = Stroke::new(1.5, ACCENT);
    ctx.set_visuals(v);
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 5.0);
    style.spacing.interact_size.y = 26.0;
    style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.0));
    style.text_styles.insert(egui::TextStyle::Button, FontId::proportional(14.0));
    style.text_styles.insert(egui::TextStyle::Small, FontId::proportional(12.0));
    style.text_styles.insert(egui::TextStyle::Heading, FontId::proportional(22.0));
    ctx.set_global_style(style);
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| ui.vertical(add).inner)
        .inner
}

pub fn titel(ui: &mut Ui, t: &str, untertitel: &str) {
    ui.label(RichText::new(t).size(22.0).strong().color(NAVY));
    if !untertitel.is_empty() {
        ui.label(RichText::new(untertitel).color(GRAU));
    }
    ui.add_space(8.0);
}

/// Beschriftetes Textfeld in einem Grid
pub fn feld(ui: &mut Ui, label: &str, text: &mut String, breite: f32) {
    ui.label(label);
    // feste Groesse, damit das Feld in einem Grid nicht auf Textbreite zusammengedrueckt wird
    ui.add_sized([breite, 26.0], egui::TextEdit::singleline(text));
}

pub fn status_farbe(status: &str) -> Color32 {
    match status {
        "bezahlt" | "abgerechnet" => GRUEN,
        "offen" => Color32::from_rgb(180, 83, 9),
        "teilbezahlt" | "angenommen" => ACCENT,
        _ => GRAU,
    }
}

/// Oeffnet eine Datei mit dem Standardprogramm des Betriebssystems.
pub fn oeffnen(pfad: &std::path::Path) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", ""]).arg(pfad).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(pfad).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(pfad).spawn();
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme(&cc.egui_ctx);
        let pfad = db_path();
        let db = match Db::open(&pfad) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Datenbank {} kann nicht geöffnet werden: {}", pfad.display(), e);
                Db::memory().expect("Speicherdatenbank")
            }
        };
        let mut app = App {
            db,
            page: Page::Start,
            meldung: None,
            kennzahlen: Kennzahlen::default(),
            kennzahlen_veraltet: true,
            mahn: crate::mahn::MahnState { veraltet: true, nur_faellige: true, jahre: 2, ..Default::default() },
            offene: Vec::new(),
            adr: AdressenState { veraltet: true, ..Default::default() },
            art: ArtikelState { veraltet: true, ..Default::default() },
            bel: BelegState::default(),
            set: EinstellState { werte: Default::default(), import_pfad: standard_import_pfad(), geladen: false },
            bild_cache: std::collections::HashMap::new(),
            about_offen: false,
            shot: std::env::var("RCOFFICE_SHOT").ok(),
            frames: 0,
            shot_angefordert: false,
            zoom: 1.5,
        };
        // Anzeigegroesse (Standard 150 %): vergroessert Schrift und alle Elemente gleichmaessig
        let z = parse_num(&app.db.get("RC_Zoom")) as f32;
        app.zoom = if (0.7..=3.0).contains(&z) { z } else { 1.5 };
        cc.egui_ctx.set_zoom_factor(app.zoom);
        app.testansicht();
        app
    }

    /// Testhilfe: RCOFFICE_PAGE=start|adressen|artikel|angebote|auftraege|lieferscheine|rechnungen|einstellungen, RCOFFICE_OPEN=Rechnung:12
    fn testansicht(&mut self) {
        if let Ok(p) = std::env::var("RCOFFICE_PAGE") {
            self.page = match p.as_str() {
                "adressen" => Page::Adressen,
                "artikel" => Page::Artikel,
                "angebote" => Page::Belege(0),
                "auftraege" => Page::Belege(1),
                "lieferscheine" => Page::Belege(2),
                "rechnungen" => Page::Belege(3),
                "mahnwesen" => Page::Mahnwesen,
                "einstellungen" => Page::Einstellungen,
                _ => Page::Start,
            };
            if let Page::Belege(i) = self.page {
                self.bel.typ = i;
                self.bel.veraltet = true;
            }
        }
        if let Ok(id) = std::env::var("RCOFFICE_ADR") {
            self.adr.edit = self.db.adresse(&id);
            self.adr.neu = false;
        }
        if let Ok(o) = std::env::var("RCOFFICE_OPEN") {
            if let Some((typ, nr)) = o.split_once(':') {
                if let Some(b) = self.db.beleg_nach_nr(typ, nr.parse().unwrap_or(0)) {
                    let i = TYPEN.iter().position(|t| *t == typ).unwrap_or(3);
                    self.page = Page::Belege(i);
                    self.bel.typ = i;
                    self.bel.oeffnen(&self.db, &b.id);
                }
            }
        }
    }

    pub fn msg(&mut self, text: impl Into<String>, ok: bool) {
        self.meldung = Some((text.into(), ok));
    }

    pub fn alles_neu_laden(&mut self) {
        self.kennzahlen_veraltet = true;
        self.adr.veraltet = true;
        self.art.veraltet = true;
        self.bel.veraltet = true;
        self.set.geladen = false;
    }

    fn nav(&mut self, ui: &mut Ui) {
        let logo_resp = egui::Frame::new()
            .fill(Color32::WHITE)
            .inner_margin(egui::Margin::symmetric(10, 16))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.add(
                    egui::Image::new(egui::include_image!("../assets/rcoffice.svg"))
                        .fit_to_exact_size(egui::vec2(110.0, 22.0)),
                )
            });
        if logo_resp.inner.on_hover_text("Über RC office").clicked() {
            self.about_offen = true;
        }
        ui.add_space(10.0);
        let eintraege: [(&str, Page); 9] = [
            ("Start", Page::Start),
            ("Adressen", Page::Adressen),
            ("Artikel", Page::Artikel),
            ("Angebote", Page::Belege(0)),
            ("Aufträge", Page::Belege(1)),
            ("Lieferscheine", Page::Belege(2)),
            ("Rechnungen", Page::Belege(3)),
            ("Mahnwesen", Page::Mahnwesen),
            ("Einstellungen", Page::Einstellungen),
        ];
        for (name, p) in eintraege {
            let aktiv = self.page == p;
            let text = RichText::new(name).size(15.0).color(if aktiv { Color32::WHITE } else { Color32::from_rgb(203, 213, 225) });
            let btn = egui::Button::new(text).fill(if aktiv { ACCENT } else { Color32::TRANSPARENT }).stroke(Stroke::NONE).min_size(egui::vec2(ui.available_width(), 34.0));
            if ui.add(btn).clicked() {
                self.page = p;
                if let Page::Belege(i) = p {
                    if self.bel.typ != i {
                        self.bel.typ = i;
                        self.bel.edit = None;
                    }
                    self.bel.veraltet = true;
                }
                self.kennzahlen_veraltet = true;
                if p == Page::Mahnwesen {
                    self.mahn.veraltet = true;
                }
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(RichText::new("Version 0.1 · Windows & macOS").size(11.0).color(Color32::from_rgb(100, 116, 139)));
            });
        });
    }

    // ------------------------------------------------------------------ Start
    fn start_ui(&mut self, ui: &mut Ui) {
        if self.kennzahlen_veraltet {
            self.kennzahlen = self.db.kennzahlen();
            self.offene = self.db.belege("Rechnung", "", true);
            self.kennzahlen_veraltet = false;
        }
        titel(ui, "Start", "Überblick über Ihre Geschäftsvorgänge");
        if self.db.ist_leer() {
            self.import_karte(ui);
            ui.add_space(10.0);
        }
        let k = self.kennzahlen.clone();
        ui.horizontal_wrapped(|ui| {
            kachel(ui, "Offene Rechnungen", &k.offene_rechnungen.to_string(), &format!("{} € offen", money(k.offener_betrag)), if k.ueberfaellig > 0 { ROT } else { NAVY });
            kachel(ui, "Überfällig (> 14 Tage)", &k.ueberfaellig.to_string(), "Rechnungen", if k.ueberfaellig > 0 { ROT } else { GRUEN });
            kachel(ui, "Umsatz dieses Jahr", &format!("{} €", money(k.umsatz_jahr)), &format!("Vorjahr {} €", money(k.umsatz_vorjahr)), NAVY);
            kachel(ui, "Offene Angebote", &k.offene_angebote.to_string(), "warten auf Antwort", NAVY);
            kachel(ui, "Offene Aufträge", &k.offene_auftraege.to_string(), "noch nicht abgerechnet", NAVY);
        });
        ui.add_space(12.0);
        card(ui, |ui| {
            ui.label(RichText::new("Offene Rechnungen").strong().size(16.0));
            ui.add_space(4.0);
            if self.offene.is_empty() {
                ui.label(RichText::new("Keine offenen Rechnungen.").color(GRAU));
            }
            let mut oeffne: Option<String> = None;
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                egui::Grid::new("offene").striped(true).num_columns(5).spacing([18.0, 6.0]).show(ui, |ui| {
                    for h in ["Nr.", "Datum", "Kunde", "Status", "offen"] {
                        ui.label(RichText::new(h).strong().color(GRAU));
                    }
                    ui.end_row();
                    for (b, sm) in &self.offene {
                        if ui.link(b.nr.to_string()).clicked() {
                            oeffne = Some(b.id.clone());
                        }
                        ui.label(date_de(&b.datum));
                        ui.label(b.adressfeld.lines().next().unwrap_or("").to_string());
                        ui.label(RichText::new(&b.status).color(status_farbe(&b.status)));
                        ui.label(format!("{} €", money(sm.brutto - b.bezahlt)));
                        ui.end_row();
                    }
                });
            });
            if let Some(id) = oeffne {
                self.page = Page::Belege(3);
                self.bel.typ = 3;
                self.bel.oeffnen(&self.db, &id);
            }
        });
    }

    fn import_karte(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.label(RichText::new("Daten übernehmen").strong().size(16.0));
            ui.label("Die Datenbank ist noch leer. Wählen Sie entweder eine vorhandene Datenbankdatei (rcoffice.db) oder den Ordner mit dem AZURO-Export (.jsonl):");
            self.import_zeile(ui);
        });
    }

    /// Pfadfeld + Picker-Buttons + Importieren. Wird auf der Start- und auf der Einstellungsseite verwendet.
    fn import_zeile(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.set.import_pfad).desired_width(420.0));
            if ui.button("Datei…").clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("RC office Datenbank", &["db"])
                    .add_filter("Alle Dateien", &["*"])
                    .pick_file()
                {
                    self.set.import_pfad = p.display().to_string();
                }
            }
            if ui.button("Ordner…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    self.set.import_pfad = p.display().to_string();
                }
            }
            if ui.button("Importieren").clicked() {
                self.importieren();
            }
        });
    }

    /// Erkennt am Pfad, ob eine Datenbank oder ein AZURO-Ordner geladen werden soll.
    fn importieren(&mut self) {
        let pfad = std::path::PathBuf::from(self.set.import_pfad.trim());
        if !pfad.exists() {
            self.msg(format!("Pfad nicht gefunden: {}", pfad.display()), false);
            return;
        }
        if pfad.is_file() {
            self.datenbank_einspielen(&pfad);
        } else if pfad.is_dir() {
            match crate::import::importieren(&mut self.db, &pfad) {
                Ok(b) => {
                    self.msg(format!("Import abgeschlossen: {}", b.text()), true);
                    self.alles_neu_laden();
                }
                Err(e) => self.msg(format!("Import fehlgeschlagen: {}", e), false),
            }
        }
    }

    /// Oeffnet einen Dateipicker fuer Logo/Briefpapier und konvertiert das Bild nach JPG in den Datenordner.
    fn bild_waehlen(&mut self, zieldatei: &str) {
        let Some(quelle) = rfd::FileDialog::new()
            .add_filter("Bilder", &["jpg", "jpeg", "png", "bmp", "gif", "tif", "tiff", "webp"])
            .add_filter("Alle Dateien", &["*"])
            .pick_file()
        else {
            return;
        };
        let endung = quelle.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
        if endung == "pdf" {
            self.msg("PDF wird nicht unterstützt. Bitte die Seite im PDF-Programm als JPG oder PNG exportieren.", false);
            return;
        }
        let ziel = data_dir().join(zieldatei);
        if let Some(dir) = ziel.parent() { let _ = std::fs::create_dir_all(dir); }
        match bild_konvertieren_zu_jpg(&quelle, &ziel) {
            Ok(_) => {
                self.bild_cache_leeren(zieldatei);
                self.msg(format!("{} gespeichert.", zieldatei), true);
            }
            Err(e) => self.msg(format!("Bild konnte nicht gelesen werden: {}", e), false),
        }
    }

    fn bild_zuruecksetzen(&mut self, zieldatei: &str) {
        let pfad = data_dir().join(zieldatei);
        match std::fs::remove_file(&pfad) {
            Ok(_) => {
                self.bild_cache_leeren(zieldatei);
                self.msg(format!("{} zurückgesetzt (Standardbild wird verwendet).", zieldatei), true);
            }
            Err(e) => self.msg(format!("Zurücksetzen fehlgeschlagen: {}", e), false),
        }
    }

    fn bild_cache_leeren(&mut self, zieldatei: &str) {
        self.bild_cache.remove(zieldatei);
    }

    /// Kopiert die aktuelle Datenbank an einen vom Benutzer gewaehlten Ort (Backup).
    fn datenbank_sichern(&mut self) {
        let heute = today_iso();
        let vorschlag = format!("rcoffice-{}.db", heute);
        let Some(ziel) = rfd::FileDialog::new()
            .set_file_name(&vorschlag)
            .add_filter("RC office Datenbank", &["db"])
            .save_file()
        else {
            return;
        };
        match std::fs::copy(db_path(), &ziel) {
            Ok(_) => self.msg(format!("Sicherung gespeichert: {}", ziel.display()), true),
            Err(e) => self.msg(format!("Sicherung fehlgeschlagen: {}", e), false),
        }
    }

    /// Spielt eine vorhandene rcoffice.db an die Stelle von db_path() ein und oeffnet sie neu.
    fn datenbank_einspielen(&mut self, quelle: &std::path::Path) {
        if !pruefe_sqlite(quelle) {
            self.msg("Die Datei ist keine gültige RC office-Datenbank.".to_string(), false);
            return;
        }
        let ziel = db_path();
        if let Some(dir) = ziel.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Aktuelle Verbindung loslassen, damit die Datei ueberschrieben werden kann
        self.db = Db::memory().expect("Speicherdatenbank");
        let _ = std::fs::remove_file(&ziel);
        let _ = std::fs::remove_file(ziel.with_extension("db-wal"));
        let _ = std::fs::remove_file(ziel.with_extension("db-shm"));
        if let Err(e) = std::fs::copy(quelle, &ziel) {
            self.msg(format!("Kopieren fehlgeschlagen: {}", e), false);
            // Leere Datenbank an der Zielstelle neu anlegen, damit die App benutzbar bleibt
            if let Ok(d) = Db::open(&ziel) { self.db = d; }
            return;
        }
        match Db::open(&ziel) {
            Ok(d) => {
                self.db = d;
                self.alles_neu_laden();
                self.msg(format!("Datenbank geladen: {}", ziel.display()), true);
            }
            Err(e) => self.msg(format!("Datenbank öffnen fehlgeschlagen: {}", e), false),
        }
    }

    // ------------------------------------------------------------------ Adressen
    fn adressen_ui(&mut self, ui: &mut Ui) {
        if self.adr.veraltet {
            self.adr.liste = self.db.adressen(&self.adr.suche);
            self.adr.veraltet = false;
        }
        titel(ui, "Adressen", &format!("{} Einträge", self.adr.liste.len()));
        let mut aktion: Option<AdrAktion> = None;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(380.0, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        let r = ui.add(egui::TextEdit::singleline(&mut self.adr.suche).hint_text("Suchen (Name, Ort, Kundennr. …)").desired_width(230.0));
                        if r.changed() {
                            self.adr.veraltet = true;
                        }
                        if ui.button("+ Neu").clicked() {
                            aktion = Some(AdrAktion::Neu);
                        }
                    });
                    ui.add_space(4.0);
                    let sel_id = self.adr.edit.as_ref().map(|a| a.id.clone()).unwrap_or_default();
                    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 4.0).show(ui, |ui| {
                        for a in &self.adr.liste {
                            let mut text = format!("{}{}", a.name(), if a.ort.is_empty() { String::new() } else { format!(" · {}", a.ort) });
                            if text.chars().count() > 46 {
                                text = text.chars().take(45).collect::<String>() + "…";
                            }
                            if ui.selectable_label(a.id == sel_id && !self.adr.neu, text).clicked() {
                                aktion = Some(AdrAktion::Waehlen(a.id.clone()));
                            }
                        }
                    });
                });
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                if let Some(a) = self.adr.edit.as_mut() {
                    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                        card(ui, |ui| {
                            ui.label(RichText::new(if self.adr.neu { "Neue Adresse".to_string() } else { a.name() }).strong().size(17.0));
                            ui.add_space(6.0);
                            egui::Grid::new("adrform").num_columns(4).spacing([14.0, 7.0]).show(ui, |ui| {
                                feld(ui, "Firma", &mut a.firma, 230.0);
                                feld(ui, "Kundennr.", &mut a.kundennr, 230.0);
                                ui.end_row();
                                feld(ui, "Anrede", &mut a.anrede, 230.0);
                                feld(ui, "Titel", &mut a.titel, 230.0);
                                ui.end_row();
                                feld(ui, "Vorname", &mut a.vorname, 230.0);
                                feld(ui, "Nachname", &mut a.nachname, 230.0);
                                ui.end_row();
                                feld(ui, "Straße", &mut a.strasse, 230.0);
                                feld(ui, "Land", &mut a.land, 230.0);
                                ui.end_row();
                                feld(ui, "PLZ", &mut a.plz, 230.0);
                                feld(ui, "Ort", &mut a.ort, 230.0);
                                ui.end_row();
                                feld(ui, "Telefon", &mut a.telefon, 230.0);
                                feld(ui, "Telefon AP", &mut a.telefon_ap, 230.0);
                                ui.end_row();
                                feld(ui, "Fax", &mut a.fax, 230.0);
                                feld(ui, "E-Mail", &mut a.email, 230.0);
                                ui.end_row();
                                feld(ui, "Internet", &mut a.internet, 230.0);
                                feld(ui, "Kategorie", &mut a.kategorie, 230.0);
                                ui.end_row();
                                feld(ui, "Position", &mut a.position, 230.0);
                                feld(ui, "Abteilung", &mut a.abteilung, 230.0);
                                ui.end_row();
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("Briefanrede");
                                ui.add(egui::TextEdit::singleline(&mut a.briefanrede).desired_width(420.0).hint_text("Sehr geehrter Herr Muster,"));
                            });
                            ui.label("Notizen");
                            ui.add(egui::TextEdit::multiline(&mut a.notizen).desired_rows(3).desired_width(f32::INFINITY));
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if ui.button(RichText::new("Speichern").strong()).clicked() {
                                    aktion = Some(AdrAktion::Speichern);
                                }
                                if !self.adr.neu {
                                    if ui.button("Neues Angebot").clicked() {
                                        aktion = Some(AdrAktion::NeuerBeleg(0));
                                    }
                                    if ui.button("Neue Rechnung").clicked() {
                                        aktion = Some(AdrAktion::NeuerBeleg(3));
                                    }
                                    if ui.button(RichText::new("Löschen").color(ROT)).clicked() {
                                        aktion = Some(AdrAktion::Loeschen);
                                    }
                                }
                            });
                        });
                        if !self.adr.neu && !self.adr.belege.is_empty() {
                            ui.add_space(10.0);
                            card(ui, |ui| {
                                ui.label(RichText::new(format!("Belege ({})", self.adr.belege.len())).strong().size(16.0));
                                egui::Grid::new("adrbelege").striped(true).spacing([16.0, 5.0]).show(ui, |ui| {
                                    for (b, sm) in self.adr.belege.iter().take(60) {
                                        if ui.link(b.titel()).clicked() {
                                            aktion = Some(AdrAktion::BelegOeffnen(b.id.clone(), b.typ.clone()));
                                        }
                                        ui.label(date_de(&b.datum));
                                        ui.label(b.betreff.clone());
                                        ui.label(RichText::new(&b.status).color(status_farbe(&b.status)));
                                        ui.label(format!("{} €", money(sm.brutto)));
                                        ui.end_row();
                                    }
                                });
                            });
                        }
                    });
                } else {
                    ui.add_space(30.0);
                    ui.label(RichText::new("Adresse links auswählen oder „+ Neu“ klicken.").color(GRAU));
                }
            });
        });
        if let Some(a) = aktion {
            self.adr_aktion(a);
        }
    }

    fn adr_aktion(&mut self, a: AdrAktion) {
        match a {
            AdrAktion::Neu => {
                let mut n = Adresse { id: self.db.neue_id("ADR"), kundennr: self.db.naechste_kundennr(), land: "Österreich".into(), erstellt: today_iso(), sprache: "deutsch".into(), ..Default::default() };
                n.briefanrede = String::new();
                self.adr.edit = Some(n);
                self.adr.neu = true;
                self.adr.belege.clear();
            }
            AdrAktion::Waehlen(id) => {
                self.adr.edit = self.db.adresse(&id);
                self.adr.neu = false;
                self.adr.belege = self.db.belege_von_adresse(&id);
            }
            AdrAktion::Speichern => {
                if let Some(a) = self.adr.edit.clone() {
                    if a.firma.trim().is_empty() && a.nachname.trim().is_empty() {
                        self.msg("Bitte Firma oder Nachname eingeben.", false);
                        return;
                    }
                    let mut a = a;
                    if a.firma_id.is_empty() {
                        a.firma_id = a.id.clone();
                    }
                    match self.db.adresse_speichern(&a) {
                        Ok(_) => {
                            self.msg(format!("Adresse „{}“ gespeichert.", a.name()), true);
                            self.adr.neu = false;
                            self.adr.edit = Some(a);
                            self.adr.veraltet = true;
                        }
                        Err(e) => self.msg(format!("Fehler beim Speichern: {}", e), false),
                    }
                }
            }
            AdrAktion::Loeschen => {
                if let Some(a) = self.adr.edit.take() {
                    self.db.adresse_loeschen(&a.id);
                    self.msg(format!("Adresse „{}“ gelöscht.", a.name()), true);
                    self.adr.veraltet = true;
                    self.adr.belege.clear();
                }
            }
            AdrAktion::NeuerBeleg(typ) => {
                if let Some(a) = self.adr.edit.clone() {
                    self.page = Page::Belege(typ);
                    self.bel.typ = typ;
                    self.bel.neu_fuer(&self.db, Some(&a));
                }
            }
            AdrAktion::BelegOeffnen(id, typ) => {
                let t = TYPEN.iter().position(|x| *x == typ).unwrap_or(3);
                self.page = Page::Belege(t);
                self.bel.typ = t;
                self.bel.oeffnen(&self.db, &id);
            }
        }
    }

    // ------------------------------------------------------------------ Artikel
    fn artikel_ui(&mut self, ui: &mut Ui) {
        if self.art.veraltet {
            self.art.liste = self.db.artikel_liste(&self.art.suche);
            self.art.veraltet = false;
        }
        titel(ui, "Artikel", &format!("{} Artikel und Leistungen", self.art.liste.len()));
        let mut aktion: Option<ArtAktion> = None;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(420.0, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.add(egui::TextEdit::singleline(&mut self.art.suche).hint_text("Suchen …").desired_width(270.0)).changed() {
                            self.art.veraltet = true;
                        }
                        if ui.button("+ Neu").clicked() {
                            aktion = Some(ArtAktion::Neu);
                        }
                    });
                    let sel = self.art.edit.as_ref().map(|a| a.id.clone()).unwrap_or_default();
                    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() - 4.0).show(ui, |ui| {
                        for a in &self.art.liste {
                            let t = format!("{}  {}  ·  {} €", a.nr, a.name, money(a.vk));
                            if ui.selectable_label(a.id == sel && !self.art.neu, t).clicked() {
                                aktion = Some(ArtAktion::Waehlen(a.id.clone()));
                            }
                        }
                    });
                });
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                if let Some(a) = self.art.edit.as_mut() {
                    card(ui, |ui| {
                        egui::Grid::new("artform").num_columns(4).spacing([14.0, 7.0]).show(ui, |ui| {
                            feld(ui, "Artikelnr.", &mut a.nr, 120.0);
                            feld(ui, "Gruppe", &mut a.gruppe, 200.0);
                            ui.end_row();
                            feld(ui, "Bezeichnung", &mut a.name, 300.0);
                            feld(ui, "Einheit", &mut a.einheit, 120.0);
                            ui.end_row();
                            feld(ui, "Art", &mut a.art, 200.0);
                            feld(ui, "USt. %", &mut self.art.mwst, 80.0);
                            ui.end_row();
                            feld(ui, "Einkauf netto", &mut self.art.ek, 100.0);
                            feld(ui, "Verkauf netto", &mut self.art.vk, 100.0);
                            ui.end_row();
                        });
                        let vk = parse_num(&self.art.vk);
                        let mw = parse_num(&self.art.mwst);
                        ui.label(RichText::new(format!("Verkauf brutto: {} €", money(vk * (1.0 + mw / 100.0)))).color(GRAU));
                        ui.label("Beschreibung (wird in die Position übernommen)");
                        ui.add(egui::TextEdit::multiline(&mut a.beschreibung).desired_rows(4).desired_width(f32::INFINITY));
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Speichern").strong()).clicked() {
                                aktion = Some(ArtAktion::Speichern);
                            }
                            if !self.art.neu && ui.button(RichText::new("Löschen").color(ROT)).clicked() {
                                aktion = Some(ArtAktion::Loeschen);
                            }
                        });
                    });
                } else {
                    ui.add_space(30.0);
                    ui.label(RichText::new("Artikel links auswählen oder „+ Neu“ klicken.").color(GRAU));
                }
            });
        });
        if let Some(a) = aktion {
            match a {
                ArtAktion::Neu => {
                    let n = Artikel { id: self.db.neue_id("ART"), nr: self.db.naechste_artikelnr(), mwst: 20.0, einheit: "Stk.".into(), ..Default::default() };
                    self.art.mwst = num_edit(n.mwst);
                    self.art.ek.clear();
                    self.art.vk.clear();
                    self.art.edit = Some(n);
                    self.art.neu = true;
                }
                ArtAktion::Waehlen(id) => {
                    if let Some(x) = self.art.liste.iter().find(|x| x.id == id).cloned() {
                        self.art.ek = num_edit(x.ek);
                        self.art.vk = num_edit(x.vk);
                        self.art.mwst = num_edit(x.mwst);
                        self.art.edit = Some(x);
                        self.art.neu = false;
                    }
                }
                ArtAktion::Speichern => {
                    if let Some(mut x) = self.art.edit.clone() {
                        if x.name.trim().is_empty() {
                            self.msg("Bitte eine Bezeichnung eingeben.", false);
                            return;
                        }
                        x.ek = parse_num(&self.art.ek);
                        x.vk = parse_num(&self.art.vk);
                        x.mwst = parse_num(&self.art.mwst);
                        match self.db.artikel_speichern(&x) {
                            Ok(_) => {
                                self.msg(format!("Artikel „{}“ gespeichert.", x.name), true);
                                self.art.edit = Some(x);
                                self.art.neu = false;
                                self.art.veraltet = true;
                            }
                            Err(e) => self.msg(format!("Fehler: {}", e), false),
                        }
                    }
                }
                ArtAktion::Loeschen => {
                    if let Some(x) = self.art.edit.take() {
                        self.db.artikel_loeschen(&x.id);
                        self.msg(format!("Artikel „{}“ gelöscht.", x.name), true);
                        self.art.veraltet = true;
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------ Einstellungen
    fn einstellungen_ui(&mut self, ui: &mut Ui) {
        if !self.set.geladen {
            self.set.werte = self.db.alle_einstellungen().into_iter().collect();
            self.set.geladen = true;
        }
        titel(ui, "Einstellungen", "Änderungen werden automatisch gespeichert");
        // Alle Felder mit Auto-Save: innerhalb der Scroll-Closure werden geaenderte
        // Werte gesammelt und am Ende ausserhalb in die Datenbank geschrieben.
        let aend: Vec<(&'static str, String)> = egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut aend: Vec<(&'static str, String)> = Vec::new();
            card(ui, |ui| {
                ui.label(RichText::new("Firmendaten").strong().size(16.0));
                egui::Grid::new("firma").num_columns(2).spacing([14.0, 7.0]).show(ui, |ui| {
                    for (label, key, _, hint) in FIRMA {
                        ui.label(label);
                        let v = self.set.werte.entry(key.to_string()).or_default();
                        let r = ui.add(egui::TextEdit::singleline(v).hint_text(hint).desired_width(380.0));
                        if r.changed() { aend.push((key, v.clone())); }
                        ui.end_row();
                    }
                });
            });
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.label(RichText::new("Darstellung").strong().size(16.0));
                ui.horizontal(|ui| {
                    ui.label("Anzeigegröße (Schrift und Elemente)");
                    let mut pct = (self.zoom * 100.0).round() as i32;
                    if ui.add(egui::Slider::new(&mut pct, 100..=250).suffix(" %")).changed() {
                        self.zoom = pct as f32 / 100.0;
                        ui.ctx().set_zoom_factor(self.zoom);
                        self.db.set("RC_Zoom", &num_edit(self.zoom as f64));
                    }
                    if ui.button("150 % (Standard)").clicked() {
                        self.zoom = 1.5;
                        ui.ctx().set_zoom_factor(1.5);
                        self.db.set("RC_Zoom", "1,5");
                    }
                });
            });
            ui.add_space(10.0);
            let mut bild_aktion: Option<(&'static str, bool)> = None; // (Zielname, true=waehlen / false=zuruecksetzen)
            card(ui, |ui| {
                ui.label(RichText::new("Briefpapier und Logo").strong().size(16.0));
                for (label, key) in [("Briefpapier-Hintergrund im PDF drucken", "RC_Briefpapier"), ("Firmenlogo im PDF drucken", "RC_Logo"), ("Zahlungs-QR-Code auf Rechnungen drucken", "RC_QR")] {
                    let v = self.set.werte.entry(key.to_string()).or_default();
                    let mut an = v != "0";
                    if ui.checkbox(&mut an, label).changed() {
                        *v = if an { "1".into() } else { "0".into() };
                        aend.push((key, v.clone()));
                    }
                }
                ui.add_space(6.0);
                let logo_eigene = data_dir().join("Logo.jpg").exists();
                let bp_eigene = data_dir().join("Briefpapier.jpg").exists();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Logo:").strong());
                    ui.label(if logo_eigene { "eigene Datei" } else { "Standardbild" });
                    if ui.button("Logo auswählen…").clicked() { bild_aktion = Some(("Logo.jpg", true)); }
                    if logo_eigene && ui.button("Zurücksetzen").clicked() { bild_aktion = Some(("Logo.jpg", false)); }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Briefpapier:").strong());
                    ui.label(if bp_eigene { "eigene Datei" } else { "Standardbild" });
                    if ui.button("Briefpapier auswählen…").clicked() { bild_aktion = Some(("Briefpapier.jpg", true)); }
                    if bp_eigene && ui.button("Zurücksetzen").clicked() { bild_aktion = Some(("Briefpapier.jpg", false)); }
                });
                ui.label(RichText::new("Akzeptiert: JPG, PNG, BMP, GIF, TIFF, WebP. Wird intern als JPG gespeichert. Briefpapier bitte im A4-Hochformat.").color(GRAU));
                ui.add_space(8.0);
                ui.label(RichText::new("Vorschau (Beispielrechnung):").color(GRAU));
                self.vorschau_a4(ui);
            });
            if let Some((ziel, waehlen)) = bild_aktion {
                if waehlen { self.bild_waehlen(ziel); } else { self.bild_zuruecksetzen(ziel); }
            }
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.label(RichText::new("Texte für Belege").strong().size(16.0));
                egui::Grid::new("texte").num_columns(2).spacing([14.0, 7.0]).show(ui, |ui| {
                    for (label, key, mehrzeilig, hint) in TEXTE {
                        ui.label(label);
                        let v = self.set.werte.entry(key.to_string()).or_default();
                        let r = if mehrzeilig {
                            ui.add(egui::TextEdit::multiline(v).hint_text(hint).desired_rows(3).desired_width(520.0))
                        } else {
                            ui.add(egui::TextEdit::singleline(v).hint_text(hint).desired_width(520.0))
                        };
                        if r.changed() { aend.push((key, v.clone())); }
                        ui.end_row();
                    }
                });
            });
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.label(RichText::new("Mahnwesen").strong().size(16.0));
                egui::Grid::new("mahnset").num_columns(2).spacing([14.0, 7.0]).show(ui, |ui| {
                    for (label, key, mehrzeilig, hint) in MAHN {
                        ui.label(label);
                        let v = self.set.werte.entry(key.to_string()).or_default();
                        let r = if mehrzeilig {
                            ui.add(egui::TextEdit::multiline(v).hint_text(hint).desired_rows(3).desired_width(520.0))
                        } else {
                            ui.add(egui::TextEdit::singleline(v).hint_text(hint).desired_width(120.0))
                        };
                        if r.changed() { aend.push((key, v.clone())); }
                        ui.end_row();
                    }
                });
            });
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.label(RichText::new("Nummernkreise").strong().size(16.0));
                ui.label(RichText::new("Die nächste Nummer ist immer die höchste vorhandene bzw. die hier eingetragene + 1.").color(GRAU));
                egui::Grid::new("zaehler").num_columns(2).spacing([14.0, 7.0]).show(ui, |ui| {
                    for (label, key, hint) in ZAEHLER {
                        ui.label(label);
                        let v = self.set.werte.entry(key.to_string()).or_default();
                        let r = ui.add(egui::TextEdit::singleline(v).hint_text(hint).desired_width(120.0));
                        if r.changed() { aend.push((key, v.clone())); }
                        ui.end_row();
                    }
                });
            });
            ui.add_space(14.0);
            card(ui, |ui| {
                ui.label(RichText::new("Daten").strong().size(16.0));
                ui.label(format!("Datenbankdatei: {}", db_path().display()));
                ui.horizontal(|ui| {
                    if ui.button("Datenordner im Finder öffnen").clicked() {
                        oeffnen(&data_dir());
                    }
                    if ui.button("Sicherungskopie erstellen…").clicked() {
                        self.datenbank_sichern();
                    }
                });
                ui.add_space(6.0);
                ui.label("Daten laden (vorhandene rcoffice.db oder AZURO-Export-Ordner, ersetzt alle vorhandenen Daten!):");
                self.import_zeile(ui);
            });
            aend
        }).inner;
        // Auto-Save: alle in diesem Frame geaenderten Einstellungswerte persistieren
        for (k, v) in &aend {
            self.db.set(k, v);
        }
    }

    /// Kleine A4-Vorschau einer Rechnung mit aktuellem Briefpapier/Logo.
    fn vorschau_a4(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let bp_an = self.set.werte.get("RC_Briefpapier").map(|v| v != "0").unwrap_or(true);
        let logo_an = self.set.werte.get("RC_Logo").map(|v| v != "0").unwrap_or(true);
        let bp_tex = if bp_an { self.bild_textur("Briefpapier.jpg", crate::pdf::BRIEFPAPIER, &ctx) } else { None };
        let logo_tex = if logo_an { self.bild_textur("Logo.jpg", crate::pdf::LOGO, &ctx) } else { None };

        let w = 200.0;
        let h = w * 1.414;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 3.0, Color32::WHITE);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        if let Some(bp) = &bp_tex {
            painter.image(bp.id(), rect, uv, Color32::WHITE);
        }
        if let Some(lo) = &logo_tex {
            let [iw, ih] = lo.size();
            let lw = w * 0.26;
            let lh = lw * ih as f32 / iw.max(1) as f32;
            let logo_rect = egui::Rect::from_min_size(
                egui::pos2(rect.right() - lw - w * 0.06, rect.top() + h * 0.045),
                egui::vec2(lw, lh),
            );
            painter.image(lo.id(), logo_rect, uv, Color32::WHITE);
        }
        let dunkel = Color32::from_gray(60);
        let hell = Color32::from_gray(170);
        // Adressblock oben links
        for i in 0..4 {
            let y = rect.top() + h * (0.21 + i as f32 * 0.018);
            let len = w * 0.32 * (1.0 - i as f32 * 0.12);
            painter.line_segment([egui::pos2(rect.left() + w * 0.08, y), egui::pos2(rect.left() + w * 0.08 + len, y)], Stroke::new(1.2, dunkel));
        }
        // Infoblock rechts (Rechnungsnummer/Datum)
        for i in 0..3 {
            let y = rect.top() + h * (0.21 + i as f32 * 0.02);
            painter.line_segment([egui::pos2(rect.right() - w * 0.28, y), egui::pos2(rect.right() - w * 0.08, y)], Stroke::new(0.8, hell));
        }
        // Titel RECHNUNG
        painter.text(egui::pos2(rect.left() + w * 0.08, rect.top() + h * 0.32), egui::Align2::LEFT_TOP, "RECHNUNG", egui::FontId::proportional(11.0), dunkel);
        // Tabellenkopf
        let tab_y = rect.top() + h * 0.40;
        painter.line_segment([egui::pos2(rect.left() + w * 0.08, tab_y), egui::pos2(rect.right() - w * 0.08, tab_y)], Stroke::new(1.0, dunkel));
        // Tabellenzeilen
        for i in 0..5 {
            let y = tab_y + h * (0.03 + i as f32 * 0.032);
            painter.line_segment([egui::pos2(rect.left() + w * 0.08, y), egui::pos2(rect.right() - w * 0.08, y)], Stroke::new(0.5, hell));
        }
        // Summenblock
        for i in 0..3 {
            let y = rect.top() + h * (0.72 + i as f32 * 0.025);
            painter.line_segment([egui::pos2(rect.right() - w * 0.34, y), egui::pos2(rect.right() - w * 0.08, y)], Stroke::new(0.9, dunkel));
        }
        // Gruß + Fuss
        painter.line_segment([egui::pos2(rect.left() + w * 0.08, rect.top() + h * 0.85), egui::pos2(rect.left() + w * 0.35, rect.top() + h * 0.85)], Stroke::new(0.8, dunkel));
        painter.rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_gray(200)), egui::StrokeKind::Outside);
    }

    /// Laedt (oder liest aus Cache) das Logo/Briefpapier als egui-Textur. Faellt bei Fehler aufs eingebaute Bild zurueck.
    fn bild_textur(&mut self, dateiname: &str, eingebaut: &[u8], ctx: &egui::Context) -> Option<egui::TextureHandle> {
        let pfad = data_dir().join(dateiname);
        let mtime = std::fs::metadata(&pfad).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        if let Some((t, tex)) = self.bild_cache.get(dateiname) {
            if *t == mtime { return Some(tex.clone()); }
        }
        let bytes = std::fs::read(&pfad).unwrap_or_else(|_| eingebaut.to_vec());
        let img = image::load_from_memory(&bytes).or_else(|_| image::load_from_memory(eingebaut)).ok()?.to_rgba8();
        let (w, h) = (img.width() as usize, img.height() as usize);
        let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], &img);
        let tex = ctx.load_texture(dateiname, color_image, egui::TextureOptions::LINEAR);
        self.bild_cache.insert(dateiname.to_string(), (mtime, tex.clone()));
        Some(tex)
    }
}

enum AdrAktion {
    Neu,
    Waehlen(String),
    Speichern,
    Loeschen,
    NeuerBeleg(usize),
    BelegOeffnen(String, String),
}

enum ArtAktion {
    Neu,
    Waehlen(String),
    Speichern,
    Loeschen,
}

/// Liest ein Bild in beliebigem Format und speichert es als JPG an `dst`. Alpha wird auf Weiss gelegt.
fn bild_konvertieren_zu_jpg(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    let img = image::open(src).map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let mut rgb: image::RgbImage = image::ImageBuffer::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p.0[3] as u32;
        let inv = 255 - a;
        let r = ((p.0[0] as u32 * a + 255 * inv) / 255) as u8;
        let g = ((p.0[1] as u32 * a + 255 * inv) / 255) as u8;
        let b = ((p.0[2] as u32 * a + 255 * inv) / 255) as u8;
        rgb.put_pixel(x, y, image::Rgb([r, g, b]));
    }
    let mut datei = std::fs::File::create(dst).map_err(|e| e.to_string())?;
    use image::ImageEncoder;
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut datei, 90)
        .write_image(&rgb, w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Pruefung per Datei-Header: SQLite-Datenbanken beginnen mit "SQLite format 3\0".
fn pruefe_sqlite(pfad: &std::path::Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(pfad) else { return false; };
    let mut buf = [0u8; 16];
    if f.read(&mut buf).unwrap_or(0) < 16 {
        return false;
    }
    &buf == b"SQLite format 3\0"
}

fn kachel(ui: &mut Ui, titel: &str, wert: &str, zusatz: &str, farbe: Color32) {
    egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.set_min_width(170.0);
            ui.label(RichText::new(titel).color(GRAU).size(12.5));
            ui.label(RichText::new(wert).size(24.0).strong().color(farbe));
            ui.label(RichText::new(zusatz).color(GRAU).size(12.5));
        });
    });
}

/// Sucht beim ersten Start einen Ordner "daten/export" neben bzw. oberhalb der Programmdatei.
fn standard_import_pfad() -> String {
    let mut kandidaten = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        let mut p = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..6 {
            if let Some(d) = p {
                kandidaten.push(d.join("daten").join("export"));
                p = d.parent().map(|x| x.to_path_buf());
            } else {
                break;
            }
        }
    }
    kandidaten.into_iter().find(|k| k.join("Rechnung.jsonl").exists()).map(|k| k.display().to_string()).unwrap_or_default()
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        if let Some(pfad) = self.shot.clone() {
            self.frames += 1;
            if self.frames >= 45 && !self.shot_angefordert {
                self.shot_angefordert = true;
                ui.ctx().request_screenshot(move |img| {
                    let (w, h) = (img.size[0] as u32, img.size[1] as u32);
                    let mut raw = Vec::with_capacity((w * h * 4) as usize);
                    for p in img.pixels.iter() {
                        raw.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
                    }
                    if let Some(buf) = image::RgbaImage::from_raw(w, h, raw) {
                        let _ = buf.save(&pfad);
                    }
                    std::process::exit(0);
                });
            }
            ui.ctx().request_repaint();
        }
        egui::Panel::left("nav").exact_size(200.0).resizable(false).frame(egui::Frame::new().fill(NAVY).inner_margin(egui::Margin::same(0))).show_inside(ui, |ui| self.nav(ui));
        if let Some((text, ok)) = self.meldung.clone() {
            egui::Panel::bottom("meldung").exact_size(32.0).frame(egui::Frame::new().fill(if ok { Color32::from_rgb(220, 252, 231) } else { Color32::from_rgb(254, 226, 226) }).inner_margin(egui::Margin::symmetric(14, 6))).show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&text).color(if ok { GRUEN } else { ROT }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("×").clicked() {
                            self.meldung = None;
                        }
                    });
                });
            });
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(16))).show_inside(ui, |ui| match self.page {
            Page::Start => egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.start_ui(ui)).inner,
            Page::Adressen => self.adressen_ui(ui),
            Page::Artikel => self.artikel_ui(ui),
            Page::Belege(_) => crate::belege::belege_ui(self, ui),
            Page::Mahnwesen => crate::mahn::mahnwesen_ui(self, ui),
            Page::Einstellungen => self.einstellungen_ui(ui),
        });
        if self.about_offen {
            let ctx = ui.ctx().clone();
            let mut offen = true;
            let mut schliessen = false;
            egui::Window::new("Über RC office")
                .open(&mut offen)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .default_width(380.0)
                .show(&ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        ui.label(RichText::new("RC office").size(28.0).strong().color(NAVY));
                        ui.label(RichText::new("Angebote · Aufträge · Lieferscheine · Rechnungen").color(GRAU));
                        ui.add_space(14.0);
                        ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(GRAU));
                        ui.add_space(18.0);
                        ui.label(RichText::new("Programmiert von").color(GRAU).size(12.5));
                        ui.label(RichText::new("RC design visual concepts").size(18.0).strong().color(ACCENT));
                        ui.add_space(14.0);
                        ui.label(RichText::new("www.rc-design.at").color(GRAU).size(12.5));
                        ui.add_space(14.0);
                        if ui.button("Schließen").clicked() { schliessen = true; }
                        ui.add_space(4.0);
                    });
                });
            self.about_offen = offen && !schliessen;
        }
    }
}
