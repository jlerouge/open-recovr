# Changelog

All notable changes to Open Recovr are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/).

## Unreleased

- Colour themes: a sun/moon button next to the language switches between light and dark, and a palette button opens the other themes (Nord, Catppuccin Mocha and Latte, Tokyo Night, Solarized Dark and Light, Monokai, Gruvbox). The choice is remembered; `--theme <id>` picks one for a run.

## 0.2.2

From testers' feedback:

- The highlighted item of a menu is readable (white on violet).
- Labels can no longer be selected like text on a web page.
- The language globe sits outside the menu.
- The footer shows the version, a link to GitHub and a link to the search log.
- A drive card's tooltip shows its full name.
- Drive cards wrap onto several rows when there are many.
- The footer tip wraps instead of being cut off.
- The expert log can be resized by dragging its top edge (now with a visible grip), and has an "Open the file" link.
- Each search is logged to `journal.txt` next to the settings (`%APPDATA%\Open Recovr` on Windows), with the version, the drive and the search options at the top.

## 0.2.1

- "Open a disk image" is now a card next to the drives instead of a small link under them.

## 0.2.0

- Files with only a number get a name from their content: the date a photo was taken or a video filmed (EXIF, MP4), the title of Word, LibreOffice, EPUB and PDF documents, the artist and title of MP3s. Their date is filled in too.
- Each file found is checked: Intact, Damaged (with the reason) or Empty. Pictures are decoded, archives and Office files unpacked, videos walked box by box.
- Duplicates (same content found several times) are hidden behind a link.
- Disk image files (`.img`, `.dd`, `.raw`…) can be searched: "Open a disk image…" or `--image <file>`. No administrator rights needed.
- `--demo-image <file>` writes a small image for trying these out, and `--scene damaged-N` shows the N-th damaged file in a screenshot.

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
