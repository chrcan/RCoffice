# RC office

Angebote, Aufträge, Lieferscheine und Rechnungen – geschrieben in Rust.
Läuft unter **Windows** und **macOS** (eine Codebasis, Oberfläche mit egui, Datenbank SQLite).

Programmiert von **RC design visual concepts** · www.rc-design.at

## Was schon drin ist (Version 0.1)

| Bereich | Stand |
|---|---|
| Adressen | Liste, Suche, Anlegen/Ändern/Löschen, Belege je Kunde, Direkt-Sprung zu „Neues Angebot / Neue Rechnung" |
| Artikel | Liste, Suche, Anlegen/Ändern/Löschen, Brutto-Anzeige |
| Angebote, Aufträge, Lieferscheine, Rechnungen | Liste mit Suche und Filter „nur offene", Editor mit Positionen, Rabatt, Summen je Steuersatz, Status |
| Überleitung | Angebot → Auftrag → Lieferschein → Rechnung per Knopfdruck (Positionen werden kopiert, Quelle/Ziel verknüpft) |
| Zahlungen | Teil-/Vollzahlung buchen (Status offen / teilbezahlt / bezahlt), Mahnstufe vermerken |
| PDF | A4 mit Firmenlogo und Briefpapier-Hintergrund, Adressfenster, Positionstabelle, Summen, Zahlungsbedingung, Fußzeile mit Bankdaten, Seitenumbruch |
| Mahnwesen | fällige Mahnungen nach einstellbaren Fristen, Gruppierung je Kunde, Mahn-PDF (1.–3. Mahnung mit Mahngebühr und optional Verzugszinsen), Vermerk bei den Rechnungen |
| E-Mail | Button „E-Mail" im Beleg: erstellt das PDF, öffnet eine neue Nachricht im Standard-Mailprogramm (Empfänger, Betreff und Text aus den Vorlagen) und zeigt das PDF im Ordner an |
| E-Rechnung | Button „E-Rechnung" bei Rechnungen: erzeugt eine **XRechnung** (XML, UBL 2.1 / EN 16931) |
| Start | offene Rechnungen, überfällig, Umsatz Jahr/Vorjahr, offene Angebote/Aufträge |
| Einstellungen | Firmendaten, Texte, Mahnwesen, Nummernkreise, eigenes Logo und Briefpapier (JPG, PNG, BMP, GIF, TIFF, WebP), Vorschau einer Beispielrechnung |
| Datenbank | Import vorhandener Datenbank per Knopfdruck, Sicherungskopie erstellen, Datenordner im Finder/Explorer öffnen |

## Noch nicht enthalten (kommt in späteren Etappen)

* Unterschrift als Bild, direkter SMTP-Versand mit Anhang, ZUGFeRD (PDF mit eingebettetem XML) und ebInterface, OSS-Verfahren
* Die XRechnung wurde nicht mit dem amtlichen KoSIT-Validator geprüft
* Projekte / Projektvorgänge / Zeiterfassung, Korrespondenz (Briefe), Standardtexte-Verwaltung, Dashboard-Diagramme
* Etiketten, Serienbrief, Benutzer-/Rechteverwaltung, PLZ-/BLZ-Verzeichnisse

## Programm starten

**Windows:** Doppelklick auf `RC office.exe`.
**macOS:** `RC office.app` nach `/Applications/` ziehen und per Launchpad oder Spotlight starten.

Die Datenbank liegt unter Windows in `%APPDATA%\RC office\rcoffice.db`, unter macOS in
`~/Library/Application Support/RC office/rcoffice.db`. PDFs landen im Unterordner `PDF`.
Beides lässt sich bequem über **Einstellungen → Daten → „Datenordner im Finder öffnen"** erreichen.

## Datenbank übernehmen

In **Einstellungen → Daten** findest du folgende Funktionen:

* **Datei …** – wählt eine vorhandene `rcoffice.db` aus (z. B. aus einer früheren Installation oder Sicherungskopie).
* **Ordner …** – wählt einen Ordner mit tabellenweisen `.jsonl`-Exporten für den Massen-Import.
* **Importieren** – erkennt automatisch, ob eine Datenbankdatei oder ein Ordner gewählt wurde.
* **Sicherungskopie erstellen …** – speichert die aktuelle Datenbank an einem beliebigen Ort.

**Achtung:** Der Import ersetzt alle vorhandenen Daten in RC office.

### Kommandozeilen-Funktionen

```
rcoffice --info                                       # zeigt Datenbank- und PDF-Pfad
rcoffice --pdf Rechnung 821 /pfad/rechnung.pdf        # einzelnes PDF erzeugen
rcoffice --import <Ordner> [--db <Datei.db>]          # Massenimport aus .jsonl-Ordner
rcoffice --xrechnung 821 /pfad/x.xml                  # E-Rechnung (XML) erzeugen
rcoffice --mahnung "Kunde" 1 /pfad/mahnung.pdf        # Mahnung als Vorschau (bucht nichts)
```

## Logo und Briefpapier

In **Einstellungen → Briefpapier und Logo** lassen sich eigene Bilder hochladen
(JPG, JPEG, PNG, BMP, GIF, TIFF, WebP – werden intern zu JPG konvertiert, Transparenz wird auf Weiß gelegt).
Darunter wird eine kleine A4-Vorschau einer Beispielrechnung angezeigt, damit der visuelle Eindruck
sofort kontrollierbar ist. Über die zugehörigen Checkboxen lässt sich der Druck von Logo, Briefpapier
und Zahlungs-QR-Code auf PDFs ein- und ausschalten.

Im Repository liegen unter `app/assets/` nur neutrale **Platzhalter** für `briefpapier.jpg` (1653 × 2339, A4 bei ~200 dpi)
und `logo.jpg` (530 × 204). Beide Dateien werden per `include_bytes!` in das Binary eingebettet und sind damit
die Standardmotive, wenn in den Einstellungen nichts anderes hinterlegt ist. Für den produktiven Einsatz entweder die
Dateien direkt durch eigene ersetzen (gleiche Maße empfohlen) oder zur Laufzeit über die Einstellungen eigene
Bilder hochladen – letzteres landet in der Datenbank und lässt die mitgelieferten Platzhalter unberührt.

## Selbst bauen

Voraussetzung: [Rust](https://rustup.rs) (stable) und ein C-Compiler (für SQLite).

```
cd app
cargo build --release
cargo test
```

### macOS

Eine macOS-Version kann nur auf einem Mac (oder per GitHub Actions) gebaut werden:

* Auf dem Mac: `./build-mac.sh` im Projektordner – erzeugt `RC office.app` als Universal Binary (Apple Silicon + Intel).
* Ohne Mac: Projekt auf GitHub hochladen – `.github/workflows/build.yml` baut automatisch
  eine Windows-Version und eine macOS-Version (Universal Binary für Apple Silicon **und** Intel,
  gebaut auf einem Apple-Silicon-Runner). Download unter **Actions → gewünschter Lauf → Artefakte**.

#### Artefakte aus GitHub Actions entpacken

GitHub verpackt jedes Artefakt beim Download zusätzlich in ein ZIP. Die macOS-App liegt darin als
`tar.gz`, damit das Ausführungsbit erhalten bleibt (ohne das würde `RC office.app` nicht starten).

* **Windows** (`rcoffice-windows.zip`): entpacken → `rcoffice.exe` ist direkt startbar.
* **macOS** (`rcoffice-macos-universal.zip`): entpacken → darin liegt `RC-office-macos-universal.tar.gz`.
  Dieses Archiv noch einmal entpacken, z. B. per Doppelklick im Finder oder im Terminal:
  ```
  tar -xzf RC-office-macos-universal.tar.gz
  ```
  Danach `RC office.app` nach `/Applications/` ziehen. Beim ersten Start: **Rechtsklick → Öffnen**
  (die App ist nur ad-hoc signiert, nicht von Apple notarisiert).

## Aufbau des Quellcodes (`app/src`)

| Datei | Inhalt |
|---|---|
| `main.rs` | Start, Kommandozeilen-Funktionen |
| `app.rs` | Fenster, Navigation, Start, Adressen, Artikel, Einstellungen |
| `belege.rs` | Belegliste und Belegeditor |
| `db.rs` | SQLite-Schema, Datentypen, alle Abfragen, Überleitung, Zahlungen |
| `calc.rs` | Belegsummen (Netto, Rabatt, MwSt je Satz, Brutto) |
| `pdf.rs` | PDF-Ausgabe |
| `mahn.rs` | Mahnwesen: Fristen, Mahnliste, Oberfläche |
| `erechnung.rs` | XRechnung-XML |
| `versand.rs` | E-Mail-Vorbereitung |
| `import.rs` | Import aus `.jsonl`-Export-Dateien |
| `util.rs` | Zahlen-/Datumsformate, Pfade |
