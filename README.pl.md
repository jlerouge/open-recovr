<div align="center">

<img src="assets/icon.svg" alt="" width="96" height="96">

# Open Recovr

**Odzyskaj usunięte pliki, po prostu.**

Jeden ekran, bez kreatora: wybierz dysk, szukaj i zaznacz, co chcesz odzyskać.<br>
Wolne i darmowe oprogramowanie, napędzane przez [PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec).

[English](README.md) · [Français](README.fr.md) · [Español](README.es.md) · [Deutsch](README.de.md) · [Italiano](README.it.md) · [Português (Brasil)](README.pt.md) · [Nederlands](README.nl.md) · **Polski** · [Українська](README.uk.md)

[![Tests](https://img.shields.io/github/actions/workflow/status/jlerouge/open-recovr/ci.yml?branch=main&label=testy&logo=github)](https://github.com/jlerouge/open-recovr/actions/workflows/ci.yml)
[![Wersja](https://img.shields.io/github/v/release/jlerouge/open-recovr?include_prereleases&sort=semver&label=wersja)](https://github.com/jlerouge/open-recovr/releases/latest)
[![Pobrania](https://img.shields.io/github/downloads/jlerouge/open-recovr/total?label=pobrania)](https://github.com/jlerouge/open-recovr/releases)
[![Licencja MIT](https://img.shields.io/badge/licencja-MIT-blue.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange?logo=rust)](https://www.rust-lang.org)
[![Systemy](https://img.shields.io/badge/system-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)](#obsługiwane-systemy)
[![Stand With Ukraine](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/badges/StandWithUkraine.svg)](https://stand-with-ukraine.pp.ua)

<img src="docs/screenshots/en-4-results.png" alt="Open Recovr pokazuje usunięte pliki znalezione na dysku C:, z podglądem zdjęcia" width="820">

</div>

> [!NOTE]
> Ta strona jest tłumaczeniem maszynowym, a zrzuty ekranu są po angielsku. Poprawki od rodzimego użytkownika języka są mile widziane: zobacz [CONTRIBUTING.md](CONTRIBUTING.md).

## Dlaczego Open Recovr?

Utrata folderu ze zdjęciami z wakacji albo pracy dyplomowej to wystarczający stres. Programy do odzyskiwania są często drogie albo pełne żargonu. Darmowe, takie jak PhotoRec, są świetne, ale działają w terminalu tekstowym.

Open Recovr nakłada przyjazny interfejs z jednym ekranem na PhotoRec, sprawdzony wolny silnik. Dodaje też szybką analizę, która przywraca pliki **z ich oryginalną nazwą i folderem**.

## Jak to działa

| 1. Szybko czy głęboko | 2. Wybierz dysk | 3. Szukaj |
|:---:|:---:|:---:|
| [![](docs/screenshots/en-1-start.png)](docs/screenshots/en-1-start.png) | [![](docs/screenshots/en-2-home.png)](docs/screenshots/en-2-home.png) | [![](docs/screenshots/en-3-scanning.png)](docs/screenshots/en-3-scanning.png) |
| Kilka sekund w Koszu albo cały dysk. | Dysk twardy, SSD, karta pamięci, pendrive, CD… | Pliki pojawiają się na bieżąco, z pozostałym czasem. |

| 4. Przeglądaj i podglądaj | 5. Zaznacz | 6. Odzyskaj |
|:---:|:---:|:---:|
| [![](docs/screenshots/en-4-results.png)](docs/screenshots/en-4-results.png) | [![](docs/screenshots/en-5-selected.png)](docs/screenshots/en-5-selected.png) | [![](docs/screenshots/en-6-recovered.png)](docs/screenshots/en-6-recovered.png) |
| Filtry według typu, formatu, rozmiaru lub nazwy. | Każdy plik pokazuje, czy jest nienaruszony. | Kopiowane na wybrany przez Ciebie dysk. |

| Ustawienia | Ciemny motyw | Tryb eksperta |
|:---:|:---:|:---:|
| [![](docs/screenshots/en-8-settings.png)](docs/screenshots/en-8-settings.png) | [![](docs/screenshots/en-9-dark.png)](docs/screenshots/en-9-dark.png) | [![](docs/screenshots/en-7-expert.png)](docs/screenshots/en-7-expert.png) |
| Język, motyw kolorów, aktualizacje, raport. | Jasny, ciemny i 8 innych motywów. | Dziennik wyszukiwania, dla ciekawskich. |

Pełny opis krok po kroku jest w **[instrukcji obsługi](docs/user-guide.pl.md)**.

> [!WARNING]
> **Nigdy nie zapisuj odzyskanych plików na dysku, z którego pochodzą.** Możesz nadpisać pliki, które jeszcze czekają na odzyskanie. Pendrive albo inny dysk będzie idealny. Open Recovr ostrzeże Cię, jeśli spróbujesz.

## Funkcje

- 🗑️ **Natychmiastowe wyszukiwanie** w Koszu, w kilka sekund i bez specjalnych uprawnień.
- 🔎 **Szybka analiza (NTFS).** Czyta tablicę plików dysku (MFT), w której usunięty plik zachowuje nazwę. Pliki wracają **z nazwą i folderem**, w kilka minut, nawet po opróżnieniu Kosza.
- 🧲 **Pełna analiza.** PhotoRec czyta cały dysk i rozpoznaje **setki typów plików**, nawet po formatowaniu i na kartach FAT/exFAT. Bez PhotoRec najpopularniejszymi formatami zajmuje się wbudowany silnik.
- 💿 **Uszkodzone nośniki.** Nieczytelne obszary (porysowane CD/DVD, zużyte karty) są pomijane, zamiast blokować wyszukiwanie.
- 🖼️ **Podgląd przed odzyskaniem.** Zdjęcia, filmy, dźwięki, PDF-y, teksty i dokumenty Word.
- 🏷️ **Sensowne nazwy i daty.** Plik, który miał tylko numer, dostaje nazwę z zawartości: `photo_2024-07-14_18-22-05.jpg` z daty EXIF, tytuł dokumentu Word lub PDF, wykonawca i tytuł utworu MP3.
- ✅ **Sprawdzone przed odzyskaniem.** Każdy znaleziony plik jest czytany od początku do końca: **Nienaruszony**, **Uszkodzony** (z przyczyną: ucięty, pomieszany z innym plikiem, brak indeksu…) lub **Pusty**.
- 👯 **Ukryte duplikaty.** Ten sam plik znaleziony kilka razy jest pokazywany raz.
- 📀 **Obrazy dysków.** Otwiera kopię `.img` / `.dd` psującego się dysku (zrobioną na przykład za pomocą ddrescue), aby nie dotykać oryginału. Bez uprawnień administratora.
- 🗂️ **Sortowanie i filtry** według typu (zdjęcia, filmy, muzyka, dokumenty, archiwa), formatu, rozmiaru i nazwy.
- 💾 **Każdy rodzaj dysku.** Dyski twarde, SSD SATA i NVMe, karty SD, microSD i CompactFlash, pendrive'y, dyski zewnętrzne, CD/DVD.
- 🕒 **W tle.** Podczas długiego wyszukiwania okno może schować się obok zegara.
- 🛠️ **Tryb eksperta.** Pełny dziennik wyszukiwania i wyjście PhotoRec.
- 🎨 **Motywy kolorów.** Jasny lub ciemny jednym kliknięciem, a także Nord, Catppuccin, Tokyo Night, Solarized, Monokai i Gruvbox.
- 🔄 **Bezpieczne aktualizacje.** Gdy wyjdzie nowa wersja, jedno kliknięcie ją pobiera, sprawdza podpis i instaluje.
- 🩺 **Raport pomocy.** *Przygotuj raport…* zapisuje plik zip (system, dyski, dziennik ostatniego wyszukiwania) do wysłania osobie, która Ci pomaga, z zamaskowanymi danymi osobistymi. Open Recovr nigdy nie wysyła go sam.
- 🌍 **9 języków:** polski, angielski, francuski, hiszpański, niemiecki, włoski, portugalski (Brazylia), niderlandzki i ukraiński.
- 🔒 **Tylko odczyt.** Na przeszukiwanym dysku nigdy nic nie jest zapisywane. Bez konta, bez telemetrii, bez potrzeby internetu: jedyne połączenie to sprawdzanie aktualizacji (numer najnowszej wersji, pobierany z GitHuba), które można wyłączyć w Ustawieniach.

Telefonu podłączonego przez USB nie można przeszukać: pokazuje tylko pliki, które jeszcze ma, a jego pamięć jest zaszyfrowana. Open Recovr wyjaśnia, gdzie szukać zamiast tego: w koszu aplikacji Galeria lub Zdjęcia, w Zdjęciach Google, w iCloud albo na karcie microSD.

## Pobieranie

Pobierz najnowszą wersję z **[Releases](https://github.com/jlerouge/open-recovr/releases/latest)**:

| System | Plik | Jak uruchomić |
|---|---|---|
| **Windows 10 / 11** (64-bit) | `open-recovr-…-windows-x64-setup.exe` lub `…-windows-x64.zip` | Uruchom instalator (bez uprawnień administratora) albo rozpakuj i otwórz `open-recovr.exe`. PhotoRec jest w zestawie. Obie wersje aktualizują się jednym kliknięciem. |
| Linux (64-bit) | `open-recovr-…-linux-x64.tar.gz` | Rozpakuj i uruchom `./open-recovr`. Do pełnej analizy: `sudo apt install testdisk`. |
| macOS (Apple Silicon / Intel) | `open-recovr-…-macos-arm64.tar.gz` / `…-macos-x64.tar.gz` | Rozpakuj, potem prawy przycisk → *Otwórz* (aplikacja nie jest podpisana). Następnie `brew install testdisk`. |

Odczyt samego dysku (szybka i pełna analiza) wymaga uprawnień administratora. Open Recovr prosi o nie dopiero, gdy przechodzisz do głębokiego wyszukiwania; Kosz jest przeszukiwany bez nich.

## Obsługiwane systemy

| | Windows | Linux | macOS |
|---|:---:|:---:|:---:|
| Kompilowany w CI | ✅ | ✅ | ✅ |
| Testowany na prawdziwym komputerze | ✅ | ⚠️ jeszcze nie | ⚠️ jeszcze nie |
| Kosz | ✅ | ✅ kosz freedesktop | ✅ `.Trash` |
| Tablica plików NTFS (szybka analiza) | ✅ | ✅ woluminy NTFS | ✅ woluminy NTFS |
| Pełna analiza (PhotoRec) | ✅ w zestawie | ✅ `apt install testdisk` | ✅ `brew install testdisk` |
| Miniatury wideo / PDF, dźwięk | ✅ | ❌ | ❌ |
| Ikona obok zegara | ✅ | ❌ | ❌ |

**Windows to główna, przetestowana platforma.** Wersje dla Linuksa i macOS się kompilują, ale nikt jeszcze nie przetestował ich na prawdziwym komputerze. Zgłoszenia i poprawki są mile widziane: zobacz [CONTRIBUTING.md](CONTRIBUTING.md).

## Kompilacja

Potrzebujesz [Rust](https://rustup.rs) 1.95 lub nowszego.

```sh
git clone https://github.com/jlerouge/open-recovr
cd open-recovr
cargo build --release
```

W Windows pobierz PhotoRec raz poleceniem `powershell -ExecutionPolicy Bypass -File scripts\fetch-photorec.ps1`. Opcje wiersza poleceń są opisane w [angielskim README](README.md#build-from-source).

## Licencja i podziękowania

Open Recovr jest wolnym oprogramowaniem na [licencji MIT](LICENSE).

Pełną analizę wykonuje **[PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec)** © Christophe Grenier, na licencji GNU GPL v2 lub nowszej. Open Recovr uruchamia go jako **osobny, niezmieniony program**. Wersja dla Windows zawiera PhotoRec 7.2 z jego licencją (`vendor/testdisk-7.2/COPYING.txt`). Kod źródłowy PhotoRec jest dostępny na [cgsecurity.org](https://www.cgsecurity.org/wiki/TestDisk_Download).

Open Recovr to niezależny projekt, niezwiązany z żadnym komercyjnym oprogramowaniem do odzyskiwania danych.
