//! Hilfsfunktionen: Zahlen im deutschen Format, Datumswerte, Pfade.

use chrono::NaiveDate;
use std::path::PathBuf;

/// Liest eine Zahl aus FileMaker-/Eingabetext ("1.234,56", "4%", "12,5", "0,98<GS>0..." ).
pub fn parse_num(s: &str) -> f64 {
    let first = s.split('\u{1d}').next().unwrap_or("");
    let t: String = first.chars().filter(|c| !c.is_whitespace() && *c != '%' && *c != '€').collect();
    if t.is_empty() {
        return 0.0;
    }
    let t = if t.contains(',') { t.replace('.', "").replace(',', ".") } else { t };
    t.parse::<f64>().unwrap_or(0.0)
}

/// 1234.5 -> "1.234,50"
pub fn money(v: f64) -> String {
    let neg = v < 0.0;
    let cents = (v.abs() * 100.0).round() as i64;
    let whole = cents / 100;
    let frac = cents % 100;
    let digits = whole.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    format!("{}{},{:02}", if neg && cents != 0 { "-" } else { "" }, out, frac)
}

/// Zahl fuer Eingabefelder: ohne Tausenderpunkt, Komma als Dezimaltrenner, ohne unnoetige Nullen.
pub fn num_edit(v: f64) -> String {
    if v == 0.0 {
        return String::new();
    }
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}

pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// "dd.mm.yyyy" -> "yyyy-mm-dd" (leer bei ungueltig)
pub fn date_iso(s: &str) -> String {
    let t = s.trim().split(' ').next().unwrap_or("");
    NaiveDate::parse_from_str(t, "%d.%m.%Y").map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()
}

/// "yyyy-mm-dd" -> "dd.mm.yyyy"
pub fn date_de(iso: &str) -> String {
    NaiveDate::parse_from_str(iso, "%Y-%m-%d").map(|d| d.format("%d.%m.%Y").to_string()).unwrap_or_default()
}

/// Eingabe "dd.mm.yyyy" (auch "d.m.yy") -> ISO, sonst leer
pub fn date_input(s: &str) -> String {
    let t = s.trim();
    // zweistellige Jahreszahl ("8.1.07") zuerst als %y lesen, sonst wuerde %Y das Jahr 7 ergeben
    let kurz = t.rsplit('.').next().map(|j| j.len() <= 2).unwrap_or(false);
    let formate: [&str; 3] = if kurz { ["%d.%m.%y", "%d.%m.%Y", "%Y-%m-%d"] } else { ["%d.%m.%Y", "%d.%m.%y", "%Y-%m-%d"] };
    for f in formate {
        if let Ok(d) = NaiveDate::parse_from_str(t, f) {
            return d.format("%Y-%m-%d").to_string();
        }
    }
    String::new()
}

pub fn today_iso() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn add_days_iso(iso: &str, days: i64) -> String {
    NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .map(|d| (d + chrono::Duration::days(days)).format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

pub fn year_of(iso: &str) -> i32 {
    iso.get(0..4).and_then(|y| y.parse().ok()).unwrap_or(0)
}

/// Sucht oberhalb der Programmdatei einen Ordner "daten" (mit rcoffice.db oder portable.txt). So liegen Datenbank,
/// PDFs und eigene Logos im Programmordner und werden mit ihm kopiert (Windows-Programmdatei, macOS-App im Ordner).
fn portabler_datenordner() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut p = exe.parent().map(|x| x.to_path_buf());
    for _ in 0..7 {
        let d = p?;
        let kandidat = d.join("daten");
        if kandidat.join("rcoffice.db").exists() || kandidat.join("portable.txt").exists() {
            return Some(kandidat);
        }
        p = d.parent().map(|x| x.to_path_buf());
    }
    None
}

/// Datenordner: Umgebungsvariable RCOFFICE_DATA, sonst "daten" im Programmordner (portabel), sonst der uebliche Ort
/// (Windows: %APPDATA%\RC office, macOS: ~/Library/Application Support/RC office).
pub fn data_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("RCOFFICE_DATA") {
        return PathBuf::from(p);
    }
    if let Some(d) = portabler_datenordner() {
        return d;
    }
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
    };
    base.unwrap_or_else(|| PathBuf::from(".")).join("RC office")
}

/// Desktop-Ordner des Benutzers, falls auffindbar.
pub fn desktop_dir() -> Option<PathBuf> {
    let home = if cfg!(target_os = "windows") {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    };
    home.map(|h| h.join("Desktop")).filter(|p| p.exists())
}

/// Datenbank-Pfad; ueberschreibbar mit der Umgebungsvariable RCOFFICE_DB.
pub fn db_path() -> PathBuf {
    if let Some(p) = std::env::var_os("RCOFFICE_DB") {
        return PathBuf::from(p);
    }
    data_dir().join("rcoffice.db")
}

/// Entfernt BOM/Steuerzeichen aus importiertem Text.
pub fn clean_text(s: &str) -> String {
    s.chars().filter(|c| *c != '\u{feff}' && *c != '\u{1d}' || *c == '\n').collect::<String>().trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zahlen() {
        assert_eq!(parse_num("1.234,56"), 1234.56);
        assert_eq!(parse_num(",98\u{1d}0\u{1d}0"), 0.98);
        assert_eq!(parse_num("4%"), 4.0);
        assert_eq!(parse_num("-5"), -5.0);
        assert_eq!(money(1234.5), "1.234,50");
        assert_eq!(money(-0.5), "-0,50");
        assert_eq!(money(1000000.0), "1.000.000,00");
        assert_eq!(num_edit(12.5), "12,5");
    }

    #[test]
    fn datum() {
        assert_eq!(date_iso("08.01.2007"), "2007-01-08");
        assert_eq!(date_iso("08.01.2007 12:00"), "2007-01-08");
        assert_eq!(date_de("2007-01-08"), "08.01.2007");
        assert_eq!(date_input("8.1.07"), "2007-01-08");
        assert_eq!(add_days_iso("2007-01-30", 3), "2007-02-02");
    }
}
