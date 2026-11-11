// Auf Windows kein Konsolenfenster beim Start der App (CLI-Optionen funktionieren im Terminal weiterhin).
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod app;
mod belege;
mod calc;
mod db;
mod erechnung;
mod import;
mod mahn;
mod pdf;
mod util;
mod versand;

use std::path::PathBuf;

fn cli_import(args: &[String]) -> i32 {
    // rcoffice --import <Ordner mit .jsonl> [--db <Datei>]
    let dir = match args.get(2) {
        Some(d) => PathBuf::from(d),
        None => {
            eprintln!("Aufruf: rcoffice --import <Ordner mit den exportierten .jsonl-Dateien> [--db <Datenbankdatei>]");
            return 2;
        }
    };
    let db_pfad = match args.iter().position(|a| a == "--db").and_then(|i| args.get(i + 1)) {
        Some(p) => PathBuf::from(p),
        None => util::db_path(),
    };
    println!("Datenbank: {}", db_pfad.display());
    let mut db = match db::Db::open(&db_pfad) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Datenbank konnte nicht geöffnet werden: {}", e);
            return 1;
        }
    };
    match import::importieren(&mut db, &dir) {
        Ok(b) => {
            println!("Import fertig: {}", b.text());
            0
        }
        Err(e) => {
            eprintln!("Import fehlgeschlagen: {}", e);
            1
        }
    }
}

fn cli_pdf(args: &[String]) -> i32 {
    // rcoffice --pdf <Typ> <Nr> <Zieldatei.pdf>
    if args.len() < 5 {
        eprintln!("Aufruf: rcoffice --pdf <Angebot|Auftrag|Lieferschein|Rechnung> <Nummer> <Zieldatei.pdf>");
        return 2;
    }
    let db = match db::Db::open(&util::db_path()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Datenbank: {}", e);
            return 1;
        }
    };
    let nr: i64 = args[3].parse().unwrap_or(0);
    let Some(b) = db.beleg_nach_nr(&args[2], nr) else {
        eprintln!("{} Nr. {} nicht gefunden.", args[2], nr);
        return 1;
    };
    let pos = db.positionen(&b.id);
    let sm = db.summen_von(&b, &pos);
    match pdf::beleg_pdf(&db, &b, &pos, &sm, std::path::Path::new(&args[4])) {
        Ok(_) => {
            println!("PDF erstellt: {}", args[4]);
            0
        }
        Err(e) => {
            eprintln!("PDF-Fehler: {}", e);
            1
        }
    }
}

fn cli_mahnung(args: &[String]) -> i32 {
    // rcoffice --mahnung <Teil des Kundennamens> <Stufe 1-3> <Zieldatei.pdf>   (nur Vorschau, bucht nichts)
    if args.len() < 5 {
        eprintln!("Aufruf: rcoffice --mahnung <Kundenname> <1|2|3> <Zieldatei.pdf>");
        return 2;
    }
    let db = match db::Db::open(&util::db_path()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Datenbank: {}", e);
            return 1;
        }
    };
    let such = args[2].to_lowercase();
    let Some(g) = db.mahnliste(0).into_iter().find(|g| g.kunde.to_lowercase().contains(&such)) else {
        eprintln!("Kein Kunde mit offenen Rechnungen zu '{}' gefunden.", args[2]);
        return 1;
    };
    let stufe: i64 = args[3].parse().unwrap_or(1).clamp(1, 3);
    match pdf::mahnung_pdf(&db, &g, stufe, &util::today_iso(), std::path::Path::new(&args[4])) {
        Ok(_) => {
            println!("Mahnung (Stufe {}) für {} erstellt: {}", stufe, g.kunde, args[4]);
            0
        }
        Err(e) => {
            eprintln!("PDF-Fehler: {}", e);
            1
        }
    }
}

fn cli_xrechnung(args: &[String]) -> i32 {
    // rcoffice --xrechnung <Rechnungsnummer> <Zieldatei.xml>
    if args.len() < 4 {
        eprintln!("Aufruf: rcoffice --xrechnung <Rechnungsnummer> <Zieldatei.xml>");
        return 2;
    }
    let db = match db::Db::open(&util::db_path()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Datenbank: {}", e);
            return 1;
        }
    };
    let Some(b) = db.beleg_nach_nr("Rechnung", args[2].parse().unwrap_or(0)) else {
        eprintln!("Rechnung {} nicht gefunden.", args[2]);
        return 1;
    };
    let pos = db.positionen(&b.id);
    match erechnung::xrechnung_xml(&db, &b, &pos) {
        Ok(x) => match std::fs::write(&args[3], x) {
            Ok(_) => {
                println!("XRechnung erstellt: {}", args[3]);
                0
            }
            Err(e) => {
                eprintln!("Schreiben fehlgeschlagen: {}", e);
                1
            }
        },
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

fn cli_adressen_nachtragen(args: &[String]) -> i32 {
    // rcoffice --adressen-nachtragen <Ordner mit .jsonl>   (stellt fehlende Adressen aus dem AZURO-Export wieder her)
    let Some(dir) = args.get(2) else {
        eprintln!("Aufruf: rcoffice --adressen-nachtragen <Ordner mit den exportierten .jsonl-Dateien>");
        return 2;
    };
    let mut db = match db::Db::open(&util::db_path()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Datenbank: {}", e);
            return 1;
        }
    };
    match import::adressen_nachtragen(&mut db, std::path::Path::new(dir)) {
        Ok((neu, erg)) => {
            println!("Neu angelegt ({}):", neu.len());
            for n in &neu {
                println!("  {}", n);
            }
            println!("Ergänzt ({}):", erg.len());
            for n in &erg {
                println!("  {}", n);
            }
            0
        }
        Err(e) => {
            eprintln!("Fehler: {}", e);
            1
        }
    }
}

fn cli_info() -> i32 {
    println!("Programm:       {}", std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default());
    println!("Datenordner:    {}", util::data_dir().display());
    println!("Datenbank:      {}", util::db_path().display());
    println!("PDF-Ordner:     {}", util::data_dir().join("PDF").display());
    0
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|a| a == "--info").unwrap_or(false) {
        std::process::exit(cli_info());
    }
    if args.get(1).map(|a| a == "--import").unwrap_or(false) {
        std::process::exit(cli_import(&args));
    }
    if args.get(1).map(|a| a == "--pdf").unwrap_or(false) {
        std::process::exit(cli_pdf(&args));
    }
    if args.get(1).map(|a| a == "--adressen-nachtragen").unwrap_or(false) {
        std::process::exit(cli_adressen_nachtragen(&args));
    }
    if args.get(1).map(|a| a == "--xrechnung").unwrap_or(false) {
        std::process::exit(cli_xrechnung(&args));
    }
    if args.get(1).map(|a| a == "--mahnung").unwrap_or(false) {
        std::process::exit(cli_mahnung(&args));
    }
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1360.0, 860.0]).with_min_inner_size([1000.0, 640.0]).with_maximized(true).with_title("RC office"),
        ..Default::default()
    };
    eframe::run_native("RC office", opts, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
