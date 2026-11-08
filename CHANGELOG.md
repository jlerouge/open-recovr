# Changelog

All notable changes to Open Recovr are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/).

## 0.1.0 — first public release

- One screen: drive choice, file types, results, preview and recovery.
- Recycle Bin search, in seconds, with no administrator rights.
- Quick analysis of the NTFS file table: files come back with their name and folder.
- Full analysis with PhotoRec 7.2 (bundled in the Windows build), with the time left shown. A built-in engine takes over for common formats when PhotoRec is missing.
- Unreadable areas of a drive (scratched CD/DVD, worn card) are skipped instead of stopping the search.
- Previews of photos, videos, sounds, PDFs, text and Word documents, next to the file's details.
- Sorting and filters by type, format, size and name.
- One picture per kind of drive: hard disk, SATA SSD, NVMe SSD, CD/DVD, SD, microSD and CompactFlash cards, USB stick, external disk, phone.
- Background search with an icon next to the clock (Windows).
- Expert mode: the search log and PhotoRec's output.
- Interface in English, French, Spanish and German.
- Windows (tested), Linux and macOS (built, not yet tested on real hardware).
- `--screenshot` and `--scene` options to regenerate the documentation screenshots.
- MIT licence for Open Recovr's own code. PhotoRec keeps its GPL and is fetched by `scripts/fetch-photorec.ps1` instead of being stored in Git.
