//! PDF-Ausgabe von Belegen und Mahnungen (A4) im Layout von AZURO office.
//!
//! Alle Positionen sind in Punkt (pt) von der oberen linken Ecke angegeben und stammen aus dem Original-PDF
//! einer AZURO-Rechnung (Logo und Briefkopf rechts, Titel in Grossbuchstaben, Infoblock rechts, Tabelle mit
//! Spalten Pos./Art.Nr./Beschreibung/Menge/Einheit/USt./Preis/Summe, Summen und Zahlungs-QR auf der letzten Seite).
//! Es werden nur die eingebauten Standardschriften verwendet - das laeuft ohne Zusatzdateien auf Windows und macOS.

use crate::calc::Summen;
use crate::db::{Beleg, Db, Position};
use crate::util::*;
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

const PT: f32 = 0.352_778; // mm je pt
const SEITE_H: f32 = 841.89;
const X_L: f32 = 60.0;
const X_R: f32 = 540.0;
/// Unterhalb dieser Linie beginnt die Fusszeile; Positionszeilen enden davor.
const LIMIT: f32 = 735.0;
/// Hoehe, die eine Seite hoechstens fuer Summen/Gruss nutzen darf
const LIMIT_SUMMEN: f32 = 745.0;
/// Verhaeltnis Oberkante Textbox -> Grundlinie (an das Original angeglichen)
const ASC: f32 = 1.07;

pub(crate) const BRIEFPAPIER: &[u8] = include_bytes!("../assets/briefpapier.jpg");
pub(crate) const LOGO: &[u8] = include_bytes!("../assets/logo.jpg");

// ------------------------------------------------------------------------------------------------ JPEG
/// JPEG, das unveraendert (ohne Neukodierung) in das PDF eingebettet wird.
#[derive(Clone)]
pub struct Jpeg {
    bytes: Vec<u8>,
    w: usize,
    h: usize,
    komponenten: u8,
}

/// Liest Breite/Hoehe/Farbkomponenten aus dem JPEG-Kopf (SOF-Marker).
pub fn jpeg_info(b: &[u8]) -> Option<(usize, usize, u8)> {
    if b.len() < 4 || b[0] != 0xff || b[1] != 0xd8 {
        return None;
    }
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xff {
            i += 1;
            continue;
        }
        let m = b[i + 1];
        if m == 0xff {
            i += 1;
            continue;
        }
        if m == 0xd8 || m == 0x01 || (0xd0..=0xd7).contains(&m) {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        if (0xc0..=0xcf).contains(&m) && m != 0xc4 && m != 0xc8 && m != 0xcc {
            if i + 10 > b.len() {
                return None;
            }
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as usize;
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as usize;
            return Some((w, h, b[i + 9]));
        }
        i += 2 + len;
    }
    None
}

/// Eigene Datei im Datenordner (z. B. `Briefpapier.jpg`) hat Vorrang vor dem eingebauten Standardbild.
fn bild_laden(dateiname: &str, eingebaut: &[u8]) -> Option<Jpeg> {
    let bytes = std::fs::read(data_dir().join(dateiname)).ok().filter(|b| jpeg_info(b).is_some()).unwrap_or_else(|| eingebaut.to_vec());
    let (w, h, komponenten) = jpeg_info(&bytes)?;
    Some(Jpeg { bytes, w, h, komponenten })
}

// ------------------------------------------------------------------------------------------------ Schriftbreiten
/// Zeichenbreiten der Helvetica (1/1000 em) fuer exakte Rechtsbuendigkeit und Zeilenumbruch.
fn zeichen_breite(c: char, fett: bool) -> f32 {
    let c = match c {
        'ä' => 'a',
        'ö' => 'o',
        'ü' => 'u',
        'Ä' => 'A',
        'Ö' => 'O',
        'Ü' => 'U',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'á' | 'à' | 'â' => 'a',
        'í' | 'ì' | 'î' => 'i',
        'ó' | 'ò' | 'ô' => 'o',
        'ú' | 'ù' | 'û' => 'u',
        _ => c,
    };
    let w: u16 = if !fett {
        match c {
            ' ' | '!' | ',' | '.' | '/' | ':' | ';' | 'I' | '[' | '\\' | ']' | 'f' | 't' | '·' => 278,
            '"' => 355,
            '#' | '$' | '0'..='9' | '?' | 'a' | 'b' | 'd' | 'e' | 'g' | 'h' | 'n' | 'o' | 'p' | 'q' | 'u' | '_' | '€' | '–' => 556,
            '%' => 889,
            '&' | 'A' | 'B' | 'E' | 'K' | 'P' | 'S' | 'V' | 'X' | 'Y' => 667,
            '\'' => 191,
            '(' | ')' | '-' | '`' | 'r' => 333,
            '*' => 389,
            '+' | '<' | '=' | '>' | '~' => 584,
            '@' => 1015,
            'C' | 'D' | 'H' | 'N' | 'R' | 'U' | 'w' => 722,
            'F' | 'T' | 'Z' | 'L' => if c == 'L' { 556 } else { 611 },
            'G' | 'O' | 'Q' => 778,
            'J' | 'c' | 'k' | 's' | 'v' | 'x' | 'y' | 'z' => 500,
            'M' => 833,
            'm' => 833,
            'W' => 944,
            '^' => 469,
            'i' | 'j' | 'l' => 222,
            '{' | '}' => 334,
            '|' => 260,
            'ß' => 611,
            _ => 556,
        }
    } else {
        match c {
            ' ' | ',' | '.' | '/' | 'I' | '[' | '\\' | ']' | '|' | 'i' | 'j' | 'l' => if c == '|' { 280 } else { 278 },
            '!' | '(' | ')' | '-' | ':' | ';' | 't' | 'f' => 333,
            '"' => 474,
            '#' | '$' | '0'..='9' | '_' | '€' | 'a' | 'c' | 'k' | 's' | 'v' | 'x' | 'y' | '–' => 556,
            '%' => 889,
            '&' | 'A' | 'B' | 'C' | 'D' | 'H' | 'K' | 'N' | 'R' | 'U' | 'w' => if c == '&' || c == 'w' { 722 } else { 722 },
            'E' | 'P' | 'S' | 'V' | 'X' | 'Y' | 'L' => if c == 'L' { 611 } else { 667 },
            'F' | 'T' | 'Z' | '?' => 611,
            'G' | 'O' | 'Q' => 778,
            'J' | 'M' | 'm' | 'W' | '@' => 833,
            'b' | 'd' | 'e' | 'g' | 'h' | 'n' | 'o' | 'p' | 'q' | 'u' => 611,
            'r' => 389,
            _ => 556,
        }
    };
    w as f32 / 1000.0
}

/// Textbreite in pt
pub fn breite(s: &str, pt: f32, fett: bool) -> f32 {
    s.chars().map(|c| zeichen_breite(c, fett)).sum::<f32>() * pt
}

/// Bricht Text in Zeilen mit maximaler Breite in pt. Absaetze (\n) bleiben erhalten (auch Leerzeilen).
pub fn umbrechen(text: &str, pt: f32, max_pt: f32, fett: bool) -> Vec<String> {
    let mut out = Vec::new();
    for absatz in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let mut zeile = String::new();
        for wort in absatz.split_whitespace() {
            let probe = if zeile.is_empty() { wort.to_string() } else { format!("{} {}", zeile, wort) };
            if breite(&probe, pt, fett) <= max_pt || zeile.is_empty() {
                zeile = probe;
            } else {
                out.push(std::mem::take(&mut zeile));
                zeile = wort.to_string();
            }
        }
        out.push(zeile);
    }
    while out.last().map(|l| l.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out
}

// ------------------------------------------------------------------------------------------------ Zahlungs-QR (EPC / GiroCode)
/// Inhalt des EPC-QR-Codes ("GiroCode") wie ihn AZURO/Banking-Apps verwenden.
pub fn epc_payload(name: &str, iban: &str, bic: &str, betrag: f64, verwendungszweck: &str) -> String {
    [
        "BCD".to_string(),
        "002".to_string(),
        "1".to_string(),
        "SCT".to_string(),
        bic.replace(' ', ""),
        name.chars().take(70).collect(),
        iban.replace(' ', ""),
        format!("EUR{:.2}", round2(betrag)),
        String::new(),
        String::new(),
        verwendungszweck.chars().take(140).collect(),
    ]
    .join("\n")
}

/// QR-Matrix (Seitenlaenge, dunkle Module zeilenweise)
pub fn qr_matrix(daten: &str) -> Option<(usize, Vec<bool>)> {
    use qrcode::{Color as QC, EcLevel, QrCode};
    let code = QrCode::with_error_correction_level(daten.as_bytes(), EcLevel::M).ok()?;
    let n = code.width();
    Some((n, code.to_colors().into_iter().map(|c| c == QC::Dark).collect()))
}

// ------------------------------------------------------------------------------------------------ Seite
struct Pdf {
    doc: PdfDocumentReference,
    layer: PdfLayerReference,
    normal: IndirectFontRef,
    fett: IndirectFontRef,
    nr: u32,
    kennung: String, // "Rechnung Nr. 844" fuer Folgeseiten
    fuss: Vec<String>,
    brief: Vec<String>,
    papier: Option<Jpeg>,
    logo: Option<Jpeg>,
}

impl Pdf {
    fn mm_x(x: f32) -> Mm {
        Mm(x * PT)
    }
    fn mm_y(y: f32) -> Mm {
        Mm((SEITE_H - y) * PT)
    }

    /// Text mit Oberkante `top` (wie im Original-PDF gemessen)
    fn text(&self, s: &str, size: f32, x: f32, top: f32, fett: bool) {
        if s.is_empty() {
            return;
        }
        self.layer.use_text(s, size, Self::mm_x(x), Self::mm_y(top + size * ASC), if fett { &self.fett } else { &self.normal });
    }
    fn rechts(&self, s: &str, size: f32, x_rechts: f32, top: f32, fett: bool) {
        self.text(s, size, x_rechts - breite(s, size, fett), top, fett);
    }
    fn mitte(&self, s: &str, size: f32, x_mitte: f32, top: f32, fett: bool) {
        self.text(s, size, x_mitte - breite(s, size, fett) / 2.0, top, fett);
    }
    fn linie(&self, x1: f32, x2: f32, y: f32, dicke: f32) {
        self.layer.set_outline_thickness(dicke);
        self.layer.set_outline_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
        self.layer.add_line(Line { points: vec![(Point::new(Self::mm_x(x1), Self::mm_y(y)), false), (Point::new(Self::mm_x(x2), Self::mm_y(y)), false)], is_closed: false });
    }
    fn bild(&self, j: &Jpeg, x: f32, top: f32, breite_pt: f32) {
        let cs = match j.komponenten {
            1 => ColorSpace::Greyscale,
            4 => ColorSpace::Cmyk,
            _ => ColorSpace::Rgb,
        };
        let xobj = ImageXObject {
            width: Px(j.w),
            height: Px(j.h),
            color_space: cs,
            bits_per_component: ColorBits::Bit8,
            interpolate: true,
            image_data: j.bytes.clone(),
            image_filter: Some(ImageFilter::DCT),
            smask: None,
            clipping_bbox: None,
        };
        let hoehe_pt = breite_pt * j.h as f32 / j.w as f32;
        let dpi = j.w as f32 * 25.4 / (breite_pt * PT);
        Image::from(xobj).add_to_layer(self.layer.clone(), ImageTransform { translate_x: Some(Self::mm_x(x)), translate_y: Some(Self::mm_y(top + hoehe_pt)), dpi: Some(dpi), ..Default::default() });
    }
    fn flaeche(&self, punkte: &[(f32, f32)], r: f32, g: f32, b: f32) {
        self.layer.set_fill_color(Color::Rgb(Rgb::new(r, g, b, None)));
        let ring: Vec<(Point, bool)> = punkte.iter().map(|(x, y)| (Point::new(Self::mm_x(*x), Self::mm_y(*y)), false)).collect();
        self.layer.add_polygon(Polygon { rings: vec![ring], mode: path::PaintMode::Fill, winding_order: path::WindingOrder::NonZero });
    }
    fn rechteck(&self, x: f32, y: f32, w: f32, h: f32, farbe: (f32, f32, f32)) {
        self.flaeche(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], farbe.0, farbe.1, farbe.2);
    }

    /// Hintergrund, Logo rechts, Briefkopf rechts und Fusszeile - auf jeder Seite gleich.
    fn rahmen(&self) {
        if let Some(p) = &self.papier {
            self.bild(p, 0.0, 0.0, 595.276);
        }
        if let Some(l) = &self.logo {
            self.bild(l, 372.0, 79.9, 168.0);
        }
        let mut top = 147.3;
        for z in &self.brief {
            self.text(z, 9.0, 372.0, top, false);
            top += 11.0;
        }
        for (i, z) in self.fuss.iter().enumerate() {
            self.mitte(z, 8.0, 297.64, 770.2 + i as f32 * 11.0, false);
        }
        // Falzmarke fuer Fensterkuverts: 15 mm langer Strich bei 99 mm von oben (1/3 A4),
        // knapp am linken Papierrand. Haarlinie 0.5 pt.
        // 99 mm = 280.63 pt, 15 mm = 42.52 pt.
        self.linie(5.0, 47.5, 280.63, 0.5);
    }

    fn neue_seite(&mut self) {
        let (p, l) = self.doc.add_page(Mm(210.0), Mm(297.0), "Ebene");
        self.layer = self.doc.get_page(p).get_layer(l);
        self.nr += 1;
        self.rahmen();
        self.text(&self.kennung, 8.0, 61.0, 152.3, false);
        self.text(&format!("Seite {}", self.nr), 8.0, 61.0, 162.3, false);
    }
}

fn vorbereiten(db: &Db, titel: &str, kennung: &str) -> Result<(Pdf, std::collections::HashMap<String, String>), String> {
    let (doc, p1, l1) = PdfDocument::new(titel, Mm(210.0), Mm(297.0), "Ebene");
    let normal = doc.add_builtin_font(BuiltinFont::Helvetica).map_err(|e| e.to_string())?;
    let fett = doc.add_builtin_font(BuiltinFont::HelveticaBold).map_err(|e| e.to_string())?;
    let layer = doc.get_page(p1).get_layer(l1);
    let cfg = db.alle_einstellungen();
    let c = |k: &str| cfg.get(k).cloned().unwrap_or_default();
    // Fusszeile: Einstellung "Fusszeile" (zwei Zeilen); sonst aus den Firmendaten zusammengesetzt
    let mut fuss: Vec<String> = c("Fusszeile").lines().map(|z| z.trim().to_string()).filter(|z| !z.is_empty()).collect();
    if fuss.is_empty() {
        let l1 = [c("Firma"), format!("{}, {} {}", c("Firma_Strasse"), c("Firma_PLZ"), c("Firma_Ort")), c("Firma_Telefon")].iter().filter(|s| !s.trim().is_empty() && !s.starts_with(", ")).cloned().collect::<Vec<_>>().join(" | ");
        let l2 = [c("Firma_Bank"), if c("Firma_IBAN").is_empty() { String::new() } else { format!("IBAN: {}", c("Firma_IBAN")) }, if c("Firma_BIC").is_empty() { String::new() } else { format!("BIC: {}", c("Firma_BIC")) }, if c("Firma_UStID").is_empty() { String::new() } else { format!("UID: {}", c("Firma_UStID")) }].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" | ");
        fuss = [l1, l2].into_iter().filter(|l| !l.trim().is_empty()).collect();
    }
    let brief: Vec<String> = c("Briefkopftext").replace("\r\n", "\n").lines().map(|z| z.trim().to_string()).collect();
    let papier = if c("RC_Briefpapier") == "0" { None } else { bild_laden("Briefpapier.jpg", BRIEFPAPIER) };
    let logo = if c("RC_Logo") == "0" { None } else { bild_laden("Logo.jpg", LOGO) };
    let p = Pdf { doc, layer, normal, fett, nr: 1, kennung: kennung.to_string(), fuss, brief, papier, logo };
    p.rahmen();
    Ok((p, cfg))
}

fn fmt_menge(m: f64) -> String {
    if (m - m.round()).abs() < 1e-9 { format!("{}", m as i64) } else { num_edit(m) }
}

/// Adresszeilen (ohne Leerzeilen) und Absenderzeile im Adressfenster
fn adressblock(p: &Pdf, absender: &str, adressfeld: &str) {
    p.text(absender, 7.0, X_L, 141.2, false);
    let mut top = 156.3;
    for z in adressfeld.lines().map(|z| z.trim()).filter(|z| !z.is_empty()).take(8) {
        p.text(z, 9.0, X_L, top, false);
        top += 12.1;
    }
}

/// Titel (Grossbuchstaben), Betreff und Infoblock rechts
fn titelblock(p: &Pdf, titel: &str, betreff: &str, infos: &[(String, String)]) {
    p.text(&titel.to_uppercase(), 14.0, X_L, 265.6, false);
    if !betreff.trim().is_empty() {
        p.text(betreff.trim(), 9.0, X_L, 288.1, true);
    }
    for (i, (k, v)) in infos.iter().enumerate() {
        let top = 269.3 + i as f32 * 10.0;
        p.text(k, 8.0, 373.0, top, false);
        p.rechts(v, 8.0, 539.0, top, false);
    }
}

/// Tabellenkopf mit zwei Linien; gibt die Oberkante der ersten Zeile zurueck.
fn tabellenkopf(p: &Pdf, y: f32, spalten: &[(&str, f32, char)]) -> f32 {
    p.linie(X_L, X_R, y, 0.5);
    p.linie(X_L, X_R, y + 17.5, 0.5);
    for (t, x, ausr) in spalten {
        match ausr {
            'r' => p.rechts(t, 7.0, *x, y + 5.1, false),
            'm' => p.mitte(t, 7.0, *x, y + 5.1, false),
            _ => p.text(t, 7.0, *x, y + 5.1, false),
        }
    }
    y + 28.0
}

const BELEG_SPALTEN: [(&str, f32, char); 8] = [("Pos.", 60.0, 'l'), ("Art.Nr.", 84.0, 'l'), ("Beschreibung", 132.0, 'l'), ("Menge", 342.0, 'm'), ("Einheit", 372.0, 'l'), ("USt.", 429.0, 'r'), ("Preis", 480.0, 'r'), ("Summe", 540.0, 'r')];

// ------------------------------------------------------------------------------------------------ Belege
pub fn beleg_pdf(db: &Db, b: &Beleg, pos: &[Position], sm: &Summen, ziel: &Path) -> Result<(), String> {
    let benennung = |typ: &str| {
        let n = db.get(&format!("Benennung {}", typ));
        if n.trim().is_empty() { typ.to_string() } else { n }
    };
    let titel = benennung(&b.typ);
    let kennung = format!("{} Nr. {}", titel, b.nr);
    let (mut p, cfg) = vorbereiten(db, &kennung, &kennung)?;
    let c = |k: &str| cfg.get(k).cloned().unwrap_or_default();
    let waehrung_txt = if b.waehrung.trim().is_empty() { "€".to_string() } else { b.waehrung.clone() };

    adressblock(&p, &c("Adressfenstertext"), &b.adressfeld);
    let mut infos = vec![(format!("{} Nr.", titel), b.nr.to_string()), ("Kunden Nr.".to_string(), b.kundennr.clone()), ("Datum".to_string(), date_de(&b.datum))];
    if b.typ == "Angebot" && !b.gueltig_bis.is_empty() {
        infos.push(("Gültig bis".to_string(), date_de(&b.gueltig_bis)));
    }
    titelblock(&p, &titel, &b.betreff, &infos);

    // Anrede und Einleitung
    let mut top = 325.3;
    if !b.briefanrede.trim().is_empty() {
        p.text(b.briefanrede.trim(), 8.0, X_L, top, false);
    }
    top = 342.3;
    let mut letzte = 325.3;
    for z in umbrechen(&b.kopftext, 8.0, X_R - X_L, false) {
        p.text(&z, 8.0, X_L, top, false);
        letzte = top;
        top += 10.0;
    }

    // Tabelle
    let mut y = tabellenkopf(&p, letzte + 22.8, &BELEG_SPALTEN);
    for ps in pos {
        let name: Vec<String> = umbrechen(&ps.artikel, 8.0, 190.0, true);
        let beschr: Vec<String> = umbrechen(&ps.beschreibung, 8.0, 190.0, false);
        let zeilen = name.len().max(1) + beschr.len();
        let hoehe = (zeilen as f32 - 1.0) * 10.0 + if beschr.is_empty() { 0.0 } else { 2.2 } + 9.5;
        if y + hoehe > LIMIT {
            p.neue_seite();
            y = tabellenkopf(&p, 240.1, &BELEG_SPALTEN);
        }
        p.text(&ps.pos_nummer(), 8.0, X_L, y + 0.2, false);
        if !ps.artikel_nr.is_empty() {
            p.text(&ps.artikel_nr, 8.0, 84.0, y + 0.2, false);
        }
        for (i, z) in name.iter().enumerate() {
            p.text(z, 8.0, 132.0, y + i as f32 * 10.0, true);
        }
        let b0 = y + (name.len().max(1) as f32 - 1.0) * 10.0 + 12.2;
        for (i, z) in beschr.iter().enumerate() {
            p.text(z, 8.0, 132.0, b0 + i as f32 * 10.0, false);
        }
        if ps.menge != 0.0 {
            p.mitte(&fmt_menge(ps.menge), 8.0, 342.0, y + 0.2, false);
        }
        p.text(&ps.einheit, 8.0, 372.0, y + 0.2, false);
        if ps.mwst != 0.0 {
            p.rechts(&format!("{}%", num_edit(ps.mwst)), 8.0, 429.0, y + 0.2, false);
        }
        if ps.vk != 0.0 {
            p.rechts(&money(ps.vk), 8.0, 480.0, y + 0.2, false);
        }
        p.rechts(&money(ps.summe), 8.0, 540.0, y + 0.2, false);
        y += hoehe - 9.5 + 21.8;
    }

    // ---- Summenblock (Rechnung: Zahlungsbedingung links, Summen rechts)
    let mut links: Vec<String> = Vec::new();
    if b.typ == "Rechnung" {
        links.extend(c("Zahlungsbedingung").replace("\r\n", "\n").lines().map(|z| z.trim().to_string()).filter(|z| !z.is_empty()));
        let ld = if b.leistungsdatum.is_empty() { &b.datum } else { &b.leistungsdatum };
        let bez = { let n = c("Benennung Leistungsdatum"); if n.trim().is_empty() { "Leistungsdatum ist der".to_string() } else { n } };
        if !ld.is_empty() {
            links.push(format!("{} {}", bez, date_de(ld)));
        }
    } else if b.typ == "Angebot" && !b.gueltig_bis.is_empty() {
        links.push(format!("Dieses Angebot ist gültig bis {}.", date_de(&b.gueltig_bis)));
    }
    let mut rechts: Vec<(String, String, bool)> = Vec::new(); // (Text, Betrag, fett)
    if sm.rabatt != 0.0 {
        rechts.push(("Summe".into(), money(sm.positionen), false));
        let t = if b.rabatt_prozent != 0.0 { format!("Rabatt {}%", num_edit(b.rabatt_prozent)) } else { "Rabatt".to_string() };
        rechts.push((t, money(-sm.rabatt), false));
    }
    rechts.push((benennung_netto(&b.typ), money(sm.netto), false));
    for (satz, betrag) in &sm.mwst {
        rechts.push((format!("{}% USt.", num_edit(*satz)), money(*betrag), false));
    }
    let n_r = rechts.len() + 1;
    let hoehe_r = 24.0 + (n_r as f32 - 1.0) * 12.0 + if b.typ == "Rechnung" && b.bezahlt != 0.0 { 24.0 } else { 0.0 };
    let hoehe_l = if links.is_empty() { 0.0 } else { 6.3 + (links.len() as f32 - 1.0) * 10.0 + 9.3 + 7.0 };
    let hoehe = hoehe_r.max(hoehe_l);

    // QR nur bei offenem Rechnungsbetrag
    let offen = round2(sm.brutto - b.bezahlt);
    let qr_an = b.typ == "Rechnung" && offen > 0.004 && c("RC_QR") != "0" && !c("Firma_IBAN").trim().is_empty();
    let gruss: Vec<String> = {
        let mut g: Vec<String> = b.fusstext.replace("\r\n", "\n").replace('\u{feff}', "").lines().map(|z| z.trim_end().to_string()).collect();
        while g.last().map(|z| z.trim().is_empty()).unwrap_or(false) {
            g.pop();
        }
        // mehr als eine Leerzeile hintereinander auf eine reduzieren
        let mut r: Vec<String> = Vec::new();
        for z in g {
            if z.trim().is_empty() && r.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
                continue;
            }
            r.push(z);
        }
        r
    };
    let benoetigt = hoehe + if qr_an { 11.9 + 68.0 + 8.4 } else { 14.0 } + gruss.len() as f32 * 10.0;
    let mut y0 = y + 0.9;
    if y0 + benoetigt > LIMIT_SUMMEN {
        p.neue_seite();
        y = tabellenkopf(&p, 240.1, &BELEG_SPALTEN);
        y0 = y + 0.9;
    }
    p.linie(59.4, 539.9, y0, 0.5);
    for (i, z) in links.iter().enumerate() {
        p.text(z, 8.0, 61.0, y0 + 6.3 + i as f32 * 10.0, false);
    }
    let mut ry = y0 + 6.8;
    for (k, v, fett) in &rechts {
        p.text(k, 8.0, 373.0, ry, *fett);
        p.rechts(v, 8.0, 539.0, ry, *fett);
        ry += 12.0;
    }
    ry += 1.8;
    let gesamt_label = if b.typ == "Rechnung" { "Gesamtsumme" } else { "Gesamtbetrag" };
    p.text(gesamt_label, 8.0, 373.0, ry, true);
    p.text(&waehrung_txt, 8.0, 474.6, ry, true);
    p.rechts(&money(sm.brutto), 8.0, 539.0, ry, true);
    ry += 13.8;
    if b.typ == "Rechnung" && b.bezahlt != 0.0 {
        p.text("bereits bezahlt", 8.0, 373.0, ry, false);
        p.rechts(&money(-b.bezahlt), 8.0, 539.0, ry, false);
        ry += 12.0;
        p.text("offener Betrag", 8.0, 373.0, ry, true);
        p.rechts(&money(offen), 8.0, 539.0, ry, true);
    }
    let y1 = y0 + hoehe;
    p.linie(59.4, 539.9, y1, 0.5);

    let mut gruss_top = y1 + 14.0;
    if qr_an {
        let bt = y1 + 11.9;
        // Empfaenger wie im AZURO-Original: Einstellung "QR_Name", sonst Inhaber (Mitarbeiter), sonst Firma
        let empfaenger = [c("QR_Name"), c("Inhaber"), c("Firma")].into_iter().find(|n| !n.trim().is_empty()).unwrap_or_default();
        let zweck = format!("RG {} {}", b.nr, b.betreff.split_whitespace().collect::<Vec<_>>().join(" "));
        zahlungs_qr(&p, 372.0, bt, &empfaenger, &c("Firma_IBAN"), &c("Firma_BIC"), offen, zweck.trim());
        gruss_top = bt + 68.0 + 8.4;
    }
    for (i, z) in gruss.iter().enumerate() {
        p.text(z, 8.0, X_L, gruss_top + i as f32 * 10.0, false);
    }

    let file = File::create(ziel).map_err(|e| format!("{}: {}", ziel.display(), e))?;
    p.doc.save(&mut BufWriter::new(file)).map_err(|e| e.to_string())
}

fn benennung_netto(typ: &str) -> String {
    let _ = typ;
    "Nettosumme".to_string()
}

impl Position {
    fn pos_nummer(&self) -> String {
        self.pos.to_string()
    }
}

/// Kasten "Bezahlen Sie bequem mit Ihrem Smartphone" mit EPC-QR-Code (168 x 68 pt, QR 62 x 62 pt rechts)
fn zahlungs_qr(p: &Pdf, x: f32, top: f32, name: &str, iban: &str, bic: &str, betrag: f64, zweck: &str) {
    // Rahmen (leicht abgerundet) in Hellgrau
    let (w, h, r) = (168.0_f32, 68.0_f32, 3.0_f32);
    let rahmen = [(x + r, top), (x + w - r, top), (x + w, top + r), (x + w, top + h - r), (x + w - r, top + h), (x + r, top + h), (x, top + h - r), (x, top + r)];
    p.flaeche(&rahmen, 0.5, 0.5, 0.5);
    let innen = [(x + r + 0.5, top + 0.5), (x + w - r - 0.5, top + 0.5), (x + w - 0.5, top + r + 0.5), (x + w - 0.5, top + h - r - 0.5), (x + w - r - 0.5, top + h - 0.5), (x + r + 0.5, top + h - 0.5), (x + 0.5, top + h - r - 0.5), (x + 0.5, top + r + 0.5)];
    p.flaeche(&innen, 1.0, 1.0, 1.0);
    p.layer.set_fill_color(Color::Rgb(Rgb::new(0.5, 0.5, 0.5, None)));
    for (i, z) in ["Bezahlen Sie bequem", "mit Ihrem Smartphone:", "1. Banking App öffnen.", "2. Bezahlcode einscannen."].iter().enumerate() {
        p.layer.use_text(*z, 7.0, Pdf::mm_x(x + 8.5), Pdf::mm_y(top + 18.3 + i as f32 * 8.0 + 7.0 * ASC), &p.normal);
    }
    if let Some((n, m)) = qr_matrix(&epc_payload(name, iban, bic, betrag, zweck)) {
        let rand = 2.0;
        let modul = 62.0 / (n as f32 + 2.0 * rand);
        let qx = x + 103.0;
        let qy = top + 3.0;
        // Zeilenweise zusammenhaengende dunkle Module zu Rechtecken zusammenfassen
        for zeile in 0..n {
            let mut spalte = 0;
            while spalte < n {
                if m[zeile * n + spalte] {
                    let start = spalte;
                    while spalte < n && m[zeile * n + spalte] {
                        spalte += 1;
                    }
                    p.rechteck(qx + (rand + start as f32) * modul, qy + (rand + zeile as f32) * modul, (spalte - start) as f32 * modul + 0.05, modul + 0.05, (0.0, 0.0, 0.0));
                } else {
                    spalte += 1;
                }
            }
        }
    }
    p.layer.set_fill_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
}

// ------------------------------------------------------------------------------------------------ Mahnung
/// Mahnschreiben fuer einen Kunden: alle offenen Rechnungen, Mahngebuehr, optional Verzugszinsen, Zahlungsfrist.
pub fn mahnung_pdf(db: &Db, g: &crate::mahn::MahnGruppe, stufe: i64, datum_iso: &str, ziel: &Path) -> Result<(), String> {
    let cfgs = db.alle_einstellungen();
    let ck = |k: &str| cfgs.get(k).cloned().unwrap_or_default();
    let zahl = |k: &str, std: f64| {
        let v = parse_num(&ck(k));
        if v > 0.0 { v } else { std }
    };
    let name = |k: &str| {
        let n = ck(k);
        if n.trim().is_empty() { "Mahnung".to_string() } else { n }
    };
    let titel = match stufe {
        1 => name("Benennung erste Mahnung"),
        2 => format!("2. {}", name("Benennung zweite Mahnung")),
        _ => format!("3. {}", name("Benennung dritte Mahnung")),
    };
    let (mut p, cfg) = vorbereiten(db, &titel, &titel)?;
    let c = |k: &str| cfg.get(k).cloned().unwrap_or_default();
    let text = match stufe {
        1 => c("Text 1. Mahnung"),
        2 => c("Text 2. Mahnung"),
        _ => c("Text 3. Mahnung"),
    };
    let frist_tage = if stufe >= 3 { zahl("Mahnziel 3", 8.0) } else { zahl("Mahnziel 2", 8.0) } as i64;
    let gebuehr = match stufe {
        2 => parse_num(&c("Mahngebühr 2")),
        3 => parse_num(&c("Mahngebühr 3")),
        _ => 0.0,
    };
    let zins_satz = match stufe {
        2 => parse_num(&c("Verzugszinsen 2")),
        3 => parse_num(&c("Verzugszinsen 3")),
        _ => 0.0,
    };

    adressblock(&p, &c("Adressfenstertext"), &g.adressfeld);
    let mut infos = vec![("Datum".to_string(), date_de(datum_iso))];
    if !g.kundennr.is_empty() {
        infos.insert(0, ("Kunden Nr.".to_string(), g.kundennr.clone()));
    }
    titelblock(&p, &titel, "", &infos);
    if !g.briefanrede.trim().is_empty() {
        p.text(g.briefanrede.trim(), 8.0, X_L, 325.3, false);
    }
    let mut top = 342.3;
    let mut letzte = 325.3;
    for z in umbrechen(&text, 8.0, X_R - X_L, false) {
        p.text(&z, 8.0, X_L, top, false);
        letzte = top;
        top += 10.0;
    }

    let spalten: [(&str, f32, char); 5] = [("Rechnung Nr.", 60.0, 'l'), ("Datum", 132.0, 'l'), ("Rechnungsbetrag", 372.0, 'r'), ("bereits bezahlt", 456.0, 'r'), ("offen", 540.0, 'r')];
    let mut y = tabellenkopf(&p, letzte + 22.8, &spalten);
    let mut offen_summe = 0.0;
    let mut zinsen = 0.0;
    for post in &g.posten {
        if y + 10.0 > LIMIT {
            p.neue_seite();
            y = tabellenkopf(&p, 240.1, &spalten);
        }
        p.text(&post.beleg.nr.to_string(), 8.0, X_L, y + 0.2, false);
        p.text(&date_de(&post.beleg.datum), 8.0, 132.0, y + 0.2, false);
        p.rechts(&money(post.summen.brutto), 8.0, 372.0, y + 0.2, false);
        p.rechts(&money(post.beleg.bezahlt), 8.0, 456.0, y + 0.2, false);
        p.rechts(&money(post.offen), 8.0, 540.0, y + 0.2, false);
        offen_summe += post.offen;
        if zins_satz > 0.0 && post.tage > 0 {
            zinsen += post.offen * zins_satz / 100.0 * post.tage as f64 / 365.0;
        }
        y += 16.0;
    }
    let zinsen = round2(zinsen);
    let mut zeilen: Vec<(String, String, bool)> = vec![("Offener Betrag".into(), money(offen_summe), false)];
    if gebuehr > 0.0 {
        zeilen.push(("Mahngebühr".into(), money(gebuehr), false));
    }
    if zinsen > 0.0 {
        zeilen.push((format!("Verzugszinsen {}% p. a.", num_edit(zins_satz)), money(zinsen), false));
    }
    let hoehe = 24.0 + (zeilen.len() as f32) * 12.0;
    if y + hoehe + 70.0 > LIMIT_SUMMEN {
        p.neue_seite();
        y = tabellenkopf(&p, 240.1, &spalten);
    }
    let y0 = y + 0.9;
    p.linie(59.4, 539.9, y0, 0.5);
    let mut ry = y0 + 6.8;
    for (k, v, f) in &zeilen {
        p.text(k, 8.0, 373.0, ry, *f);
        p.rechts(v, 8.0, 539.0, ry, *f);
        ry += 12.0;
    }
    ry += 1.8;
    p.text("Zu zahlen", 8.0, 373.0, ry, true);
    p.text("€", 8.0, 474.6, ry, true);
    p.rechts(&money(round2(offen_summe + gebuehr + zinsen)), 8.0, 539.0, ry, true);
    let y1 = y0 + hoehe;
    p.linie(59.4, 539.9, y1, 0.5);
    let frist = date_de(&add_days_iso(datum_iso, frist_tage));
    let mut t = y1 + 14.0;
    for z in umbrechen(&format!("Bitte überweisen Sie den Betrag bis spätestens {} auf unser Konto (siehe Fußzeile).", frist), 8.0, X_R - X_L, true) {
        p.text(&z, 8.0, X_L, t, true);
        t += 10.0;
    }
    t += 10.0;
    for z in umbrechen(&format!("{}\n\n{}", c("Brieffloskel"), c("Firma")), 8.0, X_R - X_L, false) {
        p.text(&z, 8.0, X_L, t, false);
        t += 10.0;
    }
    let file = File::create(ziel).map_err(|e| format!("{}: {}", ziel.display(), e))?;
    p.doc.save(&mut BufWriter::new(file)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn umbruch() {
        let z = umbrechen("Das ist ein langer Text, der umgebrochen werden muss, weil er nicht in eine Zeile passt.", 8.0, 120.0, false);
        assert!(z.len() >= 2);
        assert!(z.iter().all(|l| breite(l, 8.0, false) <= 125.0));
        assert_eq!(umbrechen("a\n\nb", 8.0, 100.0, false), vec!["a", "", "b"]);
    }

    #[test]
    fn schriftbreiten_wie_im_original() {
        // Original (HelveticaNeue 8 pt): "96,00" ist 20,0 pt breit; Helvetica: 4 Ziffern + Komma
        let w = breite("96,00", 8.0, false);
        assert!((w - 20.0).abs() < 1.5, "{}", w);
        assert!((breite("RECHNUNG", 14.0, false) - 79.3).abs() < 6.0);
    }

    #[test]
    fn epc_inhalt() {
        let s = epc_payload("Test Firma", "AT61 1904 3002 3457 3201", "TESTATWW", 590.4, "Rechnung 844");
        let z: Vec<&str> = s.split('\n').collect();
        assert_eq!(z[0], "BCD");
        assert_eq!(z[3], "SCT");
        assert_eq!(z[6], "AT611904300234573201");
        assert_eq!(z[7], "EUR590.40");
        assert_eq!(z[10], "Rechnung 844");
        let (n, m) = qr_matrix(&s).unwrap();
        assert!(n >= 25 && m.len() == n * n && m.iter().any(|x| *x));
    }

    fn demo_beleg(db: &mut Db, anzahl: usize) -> (Beleg, Vec<Position>) {
        for (k, v) in [("Firma", "Test Firma"), ("Firma_IBAN", "AT61 1904 3002 3457 3201"), ("Firma_BIC", "TESTATWW")] {
            db.set(k, v);
        }
        let a = crate::db::Adresse { id: "A".into(), firma: "Kunde äöü ß".into(), ..Default::default() };
        let b = db.neuer_beleg("Rechnung", Some(&a));
        let mut pos = Vec::new();
        for i in 0..anzahl {
            let mut p = Position { artikel: format!("Leistung {} mit €-Zeichen", i), beschreibung: "Beschreibung ".repeat(12), menge: 2.5, einheit: "Std.".into(), vk: 49.9, mwst: 20.0, ..Default::default() };
            p.berechnen();
            pos.push(p);
        }
        db.beleg_speichern(&b, &pos).unwrap();
        (db.beleg(&b.id).unwrap(), pos)
    }

    #[test]
    fn pdf_einseitig_und_mehrseitig() {
        let mut db = Db::memory().unwrap();
        let (b, pos) = demo_beleg(&mut db, 3);
        let sm = db.summen_von(&b, &pos);
        let pfad = std::env::temp_dir().join("rcoffice_test1.pdf");
        beleg_pdf(&db, &b, &pos, &sm, &pfad).unwrap();
        assert!(std::fs::metadata(&pfad).unwrap().len() > 3000);
        let (b2, pos2) = demo_beleg(&mut db, 60);
        let sm2 = db.summen_von(&b2, &pos2);
        let pfad2 = std::env::temp_dir().join("rcoffice_test2.pdf");
        beleg_pdf(&db, &b2, &pos2, &sm2, &pfad2).unwrap();
        assert!(std::fs::metadata(&pfad2).unwrap().len() > std::fs::metadata(&pfad).unwrap().len());
        let _ = std::fs::remove_file(&pfad);
        let _ = std::fs::remove_file(&pfad2);
    }
}
