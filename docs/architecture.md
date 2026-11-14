# Architecture

Open Recovr is a single Rust binary with an [egui](https://github.com/emilk/egui) interface (through `eframe`). There is no installer and no service: the window does everything, and the slow work runs in background threads.

```
┌──────────────────────── app.rs (the single screen) ────────────────────────┐
│ drives · filters · result list · preview · recover bar · expert log        │
└───────────────┬─────────────────────────────┬──────────────────────────────┘
                │ ScanJob / start_recovery    │ previews
        ┌───────▼────────── engine.rs (background threads) ──────────────────┐
        │ 1. Recycle Bin ──► 2. NTFS file table ──► 3. modern / classic      │
        │    (sys)              (ntfs.rs)        (carve, frag; photorec.rs)  │
        └───────┬────────────────────────────────────────────────────────────┘
                │
        ┌───────▼──────── sys.rs ────────┐
        │ sys/windows.rs │ sys/unix.rs   │  drives, admin rights, bin, raw reads,
        └────────────────┴───────────────┘  thumbnails, sound, tray icon
```

## Modules

| File | Role |
|---|---|
| `src/main.rs` | Reads the command-line options and opens the window. |
| `src/app.rs` | The whole interface: drive cards, type chips, result table, preview panel, recover bar, dialogs, expert log. Also holds the `--screenshot` and `--scene` modes. |
| `src/engine.rs` | Background work. `ScanJob` runs a search and fills a shared `ScanShared` (found files, phase, progress, ETA, log). `start_recovery` copies ticked files out. Also reads a found file back, whatever its origin. |
| `src/ntfs.rs` | The quick scan: parses the NTFS Master File Table straight from the raw volume. Deleted records keep their name, parent folder and data runs. |
| `src/carve.rs`, `src/frag.rs` and the format modules (`jpeg.rs`, `png.rs`, `gif.rs`, `bmp.rs`, `tif.rs`, `pcx.rs`, `ole.rs`, `pkzip.rs`) | The modern engine: finds files by their headers in raw sectors, works out where each one ends, and puts fragmented files back together. See [carving.md](carving.md). |
| `src/photorec.rs` | The classic engine: drives PhotoRec as a separate process (see below). |
| `src/addon.rs` | Fetches PhotoRec on demand on Windows (the optional add-on). |
| `src/score.rs` | `--carve` and `--bench-score`, the benchmark tools (see [benchmarks.md](benchmarks.md)). |
| `src/preview.rs` | What the preview panel shows: decoded pictures, the first lines of text and Office files, or a system thumbnail for videos and PDFs. |
| `src/sys.rs` + `src/sys/` | Everything that talks to the operating system: listing drives and their kind, admin rights and elevation, the Recycle Bin/Trash, free-space maps, raw reads, thumbnails, sound, the tray icon. `windows.rs` uses `windows-sys`; `unix.rs` covers Linux and macOS with `libc`. |
| `src/i18n.rs` + `lang/` | Translations (see below). |
| `src/art.rs`, `src/icons.rs`, `assets/` | The drive pictures (SVG rendered with `resvg` at build time) and the line icons, drawn with shapes so they never depend on a font. |
| `src/bench.rs` | `--bench`: builds a disk image with known files, deletes them, and measures how many each search method brings back byte for byte. |
| `build.rs` | Renders the icons and embeds the Windows icon and version info. |

## How a search runs

`engine::ScanJob::start` spawns a thread that goes through up to three stages, stopping early if the user cancels:

1. **Recycle Bin.** It always runs, with no rights needed. It reads `$Recycle.Bin` on Windows, and the freedesktop Trash or `.Trash`/`.Trashes` on Linux and macOS. Results have *Excellent* chances.
2. **NTFS file table** (deep mode, NTFS only). `ntfs::scan` opens the raw volume and walks every MFT record that is marked deleted. It rebuilds the full path from the parent references and reads the data runs. Small files are stored inside the record itself (resident data). A file whose clusters have since been reused, or look blank, gets *Low* chances. System noise (`\Windows\`, `\AppData\Local\`…) is filtered out.
3. **Whole drive** (deep mode, when the user asked for the full scan or the drive isn't NTFS). By default, `engine::deep_scan` runs the modern engine over the drive's free space. When the user chose the classic engine in Settings and PhotoRec was found, `photorec::scan` runs PhotoRec instead.

Each found file is a `Found` with an `Origin`, which tells how to read it back:

- `Recycled`: a path in the bin;
- `Runs`: NTFS data runs on the raw volume;
- `Bytes`: resident data;
- `Raw`: an offset and length found by carving;
- `Staged`: a file PhotoRec saved.

The interface reads `ScanShared` every frame, so results appear as they arrive.

**Nothing is written to the searched drive.** Raw volumes are opened read-only. PhotoRec's output folder (the *stage*) is always placed on another drive, and so is the recovery folder; the app warns before it saves anywhere else.

## Running PhotoRec (classic engine)

PhotoRec is a text-mode program, and it is optional. Open Recovr runs it hidden, with a command line built from the user's choices:

- the device;
- free space only on NTFS, FAT and ext, all space elsewhere;
- the wanted file families.

It parses PhotoRec's progress from the screen output (sector reached, files found, elapsed time) to fill the progress bar and the time estimate. It then picks up the files PhotoRec writes into the stage folder as they appear. Unreadable sectors are skipped by PhotoRec, so a damaged medium doesn't freeze the search.

The PhotoRec binary is found in this order:

- **Windows:** the add-on folder `%LOCALAPPDATA%Open Recovr	estdisk-7.2`, filled by Settings → *Download PhotoRec* (`addon.rs` downloads the official archive from cgsecurity.org and checks each kept file against a known SHA-256); then `PATH`; then `vendor/testdisk-7.2/` near the program, filled by `scripts/fetch-photorec.ps1` in a dev checkout. PhotoRec is not included in the releases.
- **Linux and macOS:** `photorec` in `PATH`, `/usr/sbin`, `/usr/local/bin` or `/opt/homebrew/bin`.

PhotoRec runs as a separate, unmodified program that the user installs on their own, so Open Recovr's own code keeps the MIT licence while PhotoRec keeps the GPL.

`--photorec-test <image> <stage>` runs this pipeline on a disk image, for debugging.

## Administrator rights

The Recycle Bin needs no rights. Reading a raw volume does. When the user switches to the deep search, the app relaunches itself elevated:

- **Windows:** the UAC prompt, through `ShellExecute` with `runas`;
- **macOS:** `osascript … with administrator privileges`;
- **Linux:** `pkexec`.

The new instance keeps the selected drive and language.

## Translations

Interface texts are written in French in the code, inside the `t!("…")` macro, which also does `format!`-style filling. At start, `i18n` loads the file `lang/xx.txt` for the chosen language. It is embedded in the binary. Each French line is followed by `= translation`. A missing translation falls back to French, and the test `every_language_has_every_text` fails if any `t!` text is missing from a language file.

## Screenshots and tests

- `--screenshot out.png` renders the window with egui's own screenshot support and quits after a fixed number of frames.
- `--scene` puts the app in a given state first (`home`, `scanning`, `selected`, `recovered`), so `scripts/screenshots.sh` can regenerate every picture in `docs/screenshots/` with no mouse driving.
- Unit tests cover the NTFS parser (on a synthetic volume), the carver's length rules and the rebuilding of fragmented files, drive classification, translations and icon rendering. `cargo test`. `--bench` is a small end-to-end regression check; engines are compared with the tools in `scripts/bench` (see [benchmarks.md](benchmarks.md)).
