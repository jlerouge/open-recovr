<div align="center">

<img src="assets/icon.svg" alt="" width="96" height="96">

# Open Recovr

**Gelöschte Dateien zurückholen, ganz einfach.**

Ein einziger Bildschirm, kein Assistent: Laufwerk wählen, suchen, ankreuzen, was zurück soll.<br>
Frei und kostenlos, angetrieben von [PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec).

[English](README.md) · [Français](README.fr.md) · [Español](README.es.md) · **Deutsch** · [Italiano](README.it.md) · [Português (Brasil)](README.pt.md) · [Nederlands](README.nl.md) · [Polski](README.pl.md) · [Українська](README.uk.md)

[![Tests](https://img.shields.io/github/actions/workflow/status/jlerouge/open-recovr/ci.yml?branch=main&label=tests&logo=github)](https://github.com/jlerouge/open-recovr/actions/workflows/ci.yml)
[![Version](https://img.shields.io/github/v/release/jlerouge/open-recovr?include_prereleases&sort=semver&label=version)](https://github.com/jlerouge/open-recovr/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/jlerouge/open-recovr/total)](https://github.com/jlerouge/open-recovr/releases)
[![Lizenz: MIT](https://img.shields.io/badge/lizenz-MIT-blue.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange?logo=rust)](https://www.rust-lang.org)
[![Systeme](https://img.shields.io/badge/system-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)](#unterstützte-systeme)
[![Stand With Ukraine](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/badges/StandWithUkraine.svg)](https://stand-with-ukraine.pp.ua)

<img src="docs/screenshots/de-4-results.png" alt="Open Recovr zeigt auf Laufwerk C: gefundene gelöschte Dateien mit der Vorschau eines Fotos" width="820">

</div>

## Warum Open Recovr?

Den Urlaubsordner oder eine Abschlussarbeit zu verlieren, ist schon stressig genug. Wiederherstellungsprogramme sind oft teuer oder voller Fachjargon. Kostenlose Werkzeuge wie PhotoRec sind hervorragend, laufen aber in einem Textterminal.

Open Recovr setzt eine freundliche Oberfläche mit nur einem Bildschirm auf PhotoRec, eine bewährte freie Engine. Dazu kommt eine schnelle Analyse, die Dateien **mit ihrem ursprünglichen Namen und Ordner** zurückbringt.

## So funktioniert es

| 1. Schnell oder gründlich | 2. Laufwerk wählen | 3. Suchen |
|:---:|:---:|:---:|
| [![](docs/screenshots/de-1-start.png)](docs/screenshots/de-1-start.png) | [![](docs/screenshots/de-2-home.png)](docs/screenshots/de-2-home.png) | [![](docs/screenshots/de-3-scanning.png)](docs/screenshots/de-3-scanning.png) |
| Ein paar Sekunden im Papierkorb, oder das ganze Laufwerk. | Festplatte, SSD, Speicherkarte, USB-Stick, CD… | Dateien erscheinen, sobald sie gefunden werden, mit Restzeit. |

| 4. Durchsehen und Vorschau | 5. Ankreuzen | 6. Wiederherstellen |
|:---:|:---:|:---:|
| [![](docs/screenshots/de-4-results.png)](docs/screenshots/de-4-results.png) | [![](docs/screenshots/de-5-selected.png)](docs/screenshots/de-5-selected.png) | [![](docs/screenshots/de-6-recovered.png)](docs/screenshots/de-6-recovered.png) |
| Filter nach Typ, Format, Größe oder Name. | Jede Datei zeigt ihre Chancen auf Wiederherstellung. | Kopie auf ein Laufwerk Ihrer Wahl. |

| Einstellungen | Dunkles Design | Expertenmodus |
|:---:|:---:|:---:|
| [![](docs/screenshots/de-8-settings.png)](docs/screenshots/de-8-settings.png) | [![](docs/screenshots/de-9-dark.png)](docs/screenshots/de-9-dark.png) | [![](docs/screenshots/de-7-expert.png)](docs/screenshots/de-7-expert.png) |
| Sprache, Farbdesign, Updates, Bericht. | Hell, dunkel und 8 weitere Designs. | Das Suchprotokoll, für Neugierige. |

Die vollständige Anleitung finden Sie im **[Benutzerhandbuch](docs/user-guide.de.md)**.

## Wohin verschwinden gelöschte Dateien?

Eine Datei zu löschen, vernichtet sie nicht. Das System vergisst nur, wo sie liegt, und markiert ihren Platz als frei. Der Inhalt bleibt auf dem Laufwerk, bis etwas Neues darübergeschrieben wird. Jede Suche reicht eine Stufe tiefer:

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/illustrations/deleted-file-de-dark.svg">
  <img src="docs/illustrations/deleted-file-de.svg" alt="Was mit einer gelöschten Datei passiert: 1. Im Papierkorb, gefunden von der Schnellsuche. 2. Papierkorb geleert, der Name steht noch im Inhaltsverzeichnis des Laufwerks, gefunden von der Tiefensuche. 3. Verzeichnis neu belegt, die Teile liegen noch auf dem Laufwerk, gefunden von der Tiefensuche dank PhotoRec. 4. Von einer neuen Datei überschrieben, zu spät." width="100%">
</picture>

In Schritt 3 sind nur noch die Bytes übrig. **File Carving** durchsucht das ganze Laufwerk nach den Signaturen, die Anfang und Ende jedes Dateityps markieren, und setzt die Dateien daraus wieder zusammen. Das ist der schwierige Teil, und ihn erledigt **[PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec)**. Der ganze Verdienst gebührt Christophe Grenier, der es seit über zwanzig Jahren schreibt und pflegt. Open Recovr erfindet es nicht neu: Es fügt die Oberfläche hinzu, die Schritte 1 und 2 sowie die Prüfungen, Namen und Vorschauen für jede gefundene Datei.

Je früher Sie suchen, desto mehr ist zu finden. Auf einer SSD wird freigegebener Platz oft kurz danach im Hintergrund gelöscht (TRIM): Die Schritte 1 und 2 helfen bei frisch gelöschten Dateien noch, die vollständige Analyse findet dort aber womöglich wenig.

> [!WARNING]
> **Speichern Sie wiederhergestellte Dateien nie auf dem Laufwerk, von dem sie stammen.** Sie könnten Dateien überschreiben, die noch auf ihre Wiederherstellung warten. Ein USB-Stick oder eine andere Festplatte ist ideal. Open Recovr warnt Sie, wenn Sie es versuchen.

> [!CAUTION]
> **Ein defektes Laufwerk (Klicken, sehr langsam, Lesefehler) kann mit jedem Lesen schlechter werden.** Benutzen Sie es nicht weiter und kopieren Sie es zuerst vollständig auf eine gesunde Festplatte, mit einem Werkzeug wie [ddrescue](https://www.gnu.org/software/ddrescue/). Öffnen Sie diese Kopie dann mit *Abbild öffnen…* und durchsuchen Sie sie so oft Sie wollen, ohne das Original anzurühren.

## Funktionen

- 🗑️ **Schnellsuche** im Papierkorb, in Sekunden und ohne besondere Rechte.
- 🔎 **Schnelle Analyse (NTFS).** Liest die Dateitabelle des Laufwerks (MFT), in der eine gelöschte Datei ihren Namen behält. Die Dateien kommen **mit Namen und Ordner** zurück, in wenigen Minuten, auch nach dem Leeren des Papierkorbs.
- 🧲 **Vollständige Analyse.** PhotoRec liest das ganze Laufwerk und erkennt **Hunderte von Dateitypen**, auch nach einer Formatierung oder auf FAT/exFAT-Karten. Ohne PhotoRec übernimmt eine eingebaute Engine die gängigen Formate.
- 💿 **Beschädigte Datenträger.** Unlesbare Bereiche (zerkratzte CD/DVD, abgenutzte Karten) werden übersprungen, statt die Suche zu blockieren.
- 🖼️ **Vorschau vor der Wiederherstellung.** Fotos, Videos, Töne, PDFs, Texte und Word-Dokumente.
- 🏷️ **Namen und Daten, die etwas sagen.** Eine Datei, die nur eine Nummer hatte, bekommt einen Namen aus ihrem Inhalt: `photo_2024-07-14_18-22-05.jpg` aus dem EXIF-Datum, den Titel eines Word- oder PDF-Dokuments, Künstler und Titel einer MP3.
- ✅ **Vor der Wiederherstellung geprüft.** Jede gefundene Datei wird vollständig gelesen: **Intakt**, **Beschädigt** (mit Grund: abgeschnitten, mit einer anderen Datei vermischt, Index fehlt…) oder **Leer**.
- 👯 **Duplikate ausgeblendet.** Dieselbe Datei, mehrfach gefunden, wird nur einmal angezeigt.
- 📀 **Datenträgerabbilder.** Öffnet eine `.img`-/`.dd`-Kopie eines defekten Laufwerks (z. B. mit ddrescue erstellt), damit das Original unberührt bleibt. Ohne Administratorrechte.
- 🗂️ **Sortieren und filtern** nach Typ (Fotos, Videos, Musik, Dokumente, Archive), Format, Größe und Name.
- 💾 **Alle Laufwerke.** Festplatten, SATA- und NVMe-SSDs, SD-, microSD- und CompactFlash-Karten, USB-Sticks, externe Festplatten und CD/DVD.
- 🕒 **Im Hintergrund.** Das Fenster kann sich während einer langen Suche neben der Uhr verstecken.
- 🛠️ **Expertenmodus.** Das vollständige Suchprotokoll und die Ausgabe von PhotoRec.
- 🎨 **Farbdesigns.** Hell oder dunkel mit einem Klick, dazu Nord, Catppuccin, Tokyo Night, Solarized, Monokai und Gruvbox.
- 🔄 **Sichere Updates.** Ist eine neue Version verfügbar, lädt ein Klick sie herunter, prüft ihre Signatur und installiert sie.
- 🩺 **Hilfebericht.** *Bericht erstellen…* speichert eine Zip-Datei (System, Laufwerke, Protokoll der letzten Suche) für die Person, die Ihnen hilft, mit unkenntlich gemachten persönlichen Namen. Open Recovr verschickt sie nie selbst.
- 🌍 **9 Sprachen:** Deutsch, Englisch, Französisch, Spanisch, Italienisch, brasilianisches Portugiesisch, Niederländisch, Polnisch und Ukrainisch.
- 🔒 **Nur lesend.** Auf das durchsuchte Laufwerk wird nie etwas geschrieben. Kein Konto, keine Telemetrie, kein Internet nötig: Die einzige Verbindung ist die Suche nach Updates (nur die Nummer der neuesten Version, bei GitHub abgefragt), abschaltbar in den Einstellungen.

Ein per USB angeschlossenes Telefon kann nicht durchsucht werden: Es zeigt nur die Dateien, die es noch hat, und sein Speicher ist verschlüsselt. Open Recovr erklärt, wo Sie stattdessen suchen können: im Papierkorb der Galerie- oder Fotos-App, in Google Fotos, iCloud oder auf der microSD-Karte.

## Herunterladen

Laden Sie die neueste Version unter **[Releases](https://github.com/jlerouge/open-recovr/releases/latest)** herunter:

| System | Datei | So starten Sie es |
|---|---|---|
| **Windows 10 / 11** (64 Bit) | `open-recovr-…-windows-x64-setup.exe` oder `…-windows-x64.zip` | Installer starten (ohne Administratorrechte) oder entpacken und `open-recovr.exe` öffnen. PhotoRec ist enthalten. Beide aktualisieren sich mit einem Klick. |
| Linux (64 Bit) | `open-recovr-…-linux-x64.tar.gz` | Entpacken und `./open-recovr` starten. Für die vollständige Analyse: `sudo apt install testdisk`. |
| macOS (Apple Silicon / Intel) | `open-recovr-…-macos-arm64.tar.gz` / `…-macos-x64.tar.gz` | Entpacken, dann Rechtsklick → *Öffnen* (die App ist nicht signiert). Danach `brew install testdisk`. |

Das Laufwerk selbst zu lesen (schnelle und vollständige Analyse) erfordert Administratorrechte. Open Recovr fragt nur danach, wenn Sie zur Tiefensuche wechseln; der Papierkorb wird auch ohne sie durchsucht.

## Unterstützte Systeme

| | Windows | Linux | macOS |
|---|:---:|:---:|:---:|
| In der CI gebaut | ✅ | ✅ | ✅ |
| Auf echter Hardware getestet | ✅ | ⚠️ noch nicht | ⚠️ noch nicht |
| Papierkorb | ✅ | ✅ freedesktop-Papierkorb | ✅ `.Trash` |
| NTFS-Dateitabelle (schnelle Analyse) | ✅ | ✅ NTFS-Volumes | ✅ NTFS-Volumes |
| Vollständige Analyse (PhotoRec) | ✅ enthalten | ✅ `apt install testdisk` | ✅ `brew install testdisk` |
| Video-/PDF-Vorschaubilder, Ton | ✅ | ❌ | ❌ |
| Symbol neben der Uhr | ✅ | ❌ | ❌ |

**Windows ist die wichtigste und getestete Plattform.** Die Versionen für Linux und macOS lassen sich bauen, wurden aber noch auf keiner echten Hardware getestet. Rückmeldungen und Korrekturen sind sehr willkommen, siehe [CONTRIBUTING.md](CONTRIBUTING.md).

## Selbst bauen

Sie benötigen [Rust](https://rustup.rs) 1.95 oder neuer.

```sh
git clone https://github.com/jlerouge/open-recovr
cd open-recovr
cargo build --release
```

Unter Windows laden Sie PhotoRec einmalig mit `powershell -ExecutionPolicy Bypass -File scripts\fetch-photorec.ps1` herunter. Die Kommandozeilenoptionen sind im [englischen README](README.md#build-from-source) beschrieben.

## Lizenz und Danksagung

Open Recovr ist freie Software unter der [MIT-Lizenz](LICENSE).

Die vollständige Analyse übernimmt **[PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec)** © Christophe Grenier, lizenziert unter der GNU GPL v2 oder später. Open Recovr startet es als **separates, unverändertes Programm**. Die Windows-Version enthält PhotoRec 7.2 samt Lizenz (`vendor/testdisk-7.2/COPYING.txt`). Der Quellcode von PhotoRec ist auf [cgsecurity.org](https://www.cgsecurity.org/wiki/TestDisk_Download) erhältlich.

Open Recovr ist ein unabhängiges Projekt und steht in keiner Verbindung zu kommerzieller Wiederherstellungssoftware.

## Sternverlauf

<a href="https://star-history.com/#jlerouge/open-recovr&Date">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=jlerouge/open-recovr&type=Date&theme=dark">
    <img alt="Diagramm der GitHub-Sterne" src="https://api.star-history.com/svg?repos=jlerouge/open-recovr&type=Date">
  </picture>
</a>
