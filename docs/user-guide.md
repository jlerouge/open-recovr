# User guide

**English** · [Français](user-guide.fr.md) · [Español](user-guide.es.md) · [Deutsch](user-guide.de.md) · [Italiano](user-guide.it.md) · [Português (Brasil)](user-guide.pt.md) · [Nederlands](user-guide.nl.md) · [Polski](user-guide.pl.md) · [Українська](user-guide.uk.md)

This guide walks through a recovery from start to finish. The app speaks English, French, Spanish, German, Italian, Brazilian Portuguese, Dutch, Polish and Ukrainian: use the language button at the top right.

- [Before you start](#before-you-start)
- [1. Quick or deep search](#1-quick-or-deep-search)
- [2. Pick the drive and what you are looking for](#2-pick-the-drive-and-what-you-are-looking-for)
- [3. The search](#3-the-search)
- [4. Browse the results](#4-browse-the-results)
- [5. Tick what you want back](#5-tick-what-you-want-back)
- [6. Recover](#6-recover)
- [Disk images](#disk-images)
- [Settings, themes and updates](#settings-themes-and-updates)
- [Expert mode](#expert-mode)
- [Asking someone for help](#asking-someone-for-help)
- [FAQ](#faq)

## Before you start

**Stop using the drive as much as you can.** A deleted file stays on the drive until something new is written over it. Every download, install or save on that drive lowers your chances.

- If the files were on your system drive (usually `C:`), don't install anything on it. Run Open Recovr from a USB stick if you can.
- If they were on a memory card or a USB stick, take it out and don't take new photos with it.
- **Have another drive ready** to receive the recovered files: a USB stick, an external disk, or a second internal disk.

## 1. Quick or deep search

![The "How far to search?" dialog](screenshots/en-1-start.png)

At start, Open Recovr asks how far to search:

- **Quick search** looks only in the Recycle Bin. It takes a few seconds and needs no special rights. If you emptied the bin, it won't find anything.
- **Deep search** reads the drive itself. It needs administrator rights, so Windows asks you to allow the app (on Linux and macOS, you are asked for your password).

You can switch later with the **Switch to deep search** button at the top. Tick *Don't offer this at start again* if you always want the same choice.

## 2. Pick the drive and what you are looking for

![The home screen](screenshots/en-2-home.png)

**Where were your files?** Click the drive, card or stick where the files were. Each one shows its type (hard disk, SATA or NVMe SSD, SD card, USB stick, CD…), its free space and its name.

**I'm looking for:** narrows the search to *Photos*, *Videos*, *Music*, *Documents* or *Archives*. Fewer types make the search faster and the list shorter. **Refine** goes further: specific formats (only JPG, only DOCX…) and a minimum size.

In deep-search mode, NTFS drives (most Windows disks) offer two kinds of analysis:

| Analysis | What it reads | Time | Names kept? |
|---|---|---|---|
| **Quick** | The drive's file table (MFT) | A few minutes | ✅ name and folder |
| **Full** | The file table, then the whole drive with PhotoRec | From minutes to over an hour | Partly (see below) |

Start with **Quick**. If your file isn't there, click **Not found? Run the full scan**.

On other file systems (FAT32 and exFAT memory cards, ext4, APFS…), the deep search goes straight to the full analysis.

## 3. The search

![A deep search in progress](screenshots/en-3-scanning.png)

Click **Search for lost files**. Files appear in the list **as soon as they are found**: you don't have to wait for the end to start browsing.

The right-hand panel shows the current phase, the progress, an estimate of the time left and how many files were found so far.

- **Stop the search** keeps what was already found.
- **Continue in the background** hides the window next to the clock (Windows). Click the icon to bring it back. A notification tells you when the search is over.

Unreadable areas, such as a scratched CD or a worn card, are skipped. The search carries on instead of freezing.

## 4. Browse the results

![The results, with a preview](screenshots/en-4-results.png)

- The **tabs** at the top (*All*, *Photos*, *Documents*…) show how many files of each kind were found.
- **Format** and **Size** filter further. The search box filters by name.
- Click a column title to **sort** by type, name, date or size.
- Click a file to **preview** it on the right, with its details. Photos, videos, sounds, PDFs, text and Word documents can be previewed. **Open** opens it in its usual program, from a temporary copy.

Each file found is read from end to end, and the **Condition** column gives the result:

| Condition | Meaning |
|---|---|
| **Intact** | The file was checked from start to end and is whole. It will come back as it was. |
| **Damaged** | Part of it is missing or wrong. The details say why: cut short, mixed with another file, missing index… It may still partly open. |
| **Empty** | The space it used holds only zeros. There is nothing left to recover. |

The same file is often found several times (in the bin and on the drive, for example). The copies are hidden behind a *duplicates hidden · Show* link.

Files found by PhotoRec have no name of their own. Open Recovr gives them one from their contents when it can: `photo_2024-07-14_18-22-05.jpg` from the date a photo was taken, the title of a Word or PDF document, the artist and song of an MP3.

## 5. Tick what you want back

![Photos ticked for recovery](screenshots/en-5-selected.png)

Tick the files one by one, or use the checkbox in the column header to tick everything in the current view. **Tick all** and **Untick all** at the bottom work on the whole list. The bottom bar shows how many files are ticked and their total size.

## 6. Recover

![Recovery finished](screenshots/en-6-recovered.png)

Click **Recover the selection** and choose a folder **on another drive**. If you choose the drive being searched, Open Recovr warns you that you may overwrite other recoverable files.

The files are copied into a new folder named *Recovered files* followed by the date, with one subfolder per kind (Photos, Documents…). A folder deleted to the Recycle Bin comes back whole. When it is done, **Open the folder** shows them.

Nothing is ever written to the drive being searched. The files are only read from it.

## Disk images

A failing drive (clicking, very slow, full of read errors) can give up for good while it is being searched. It is safer to copy it once, sector by sector, with a tool such as [ddrescue](https://www.gnu.org/software/ddrescue/), and then search the copy.

The **Disk image** card next to the drives opens such a copy (`.img`, `.dd`, `.raw`…). The search then works as on a drive, without administrator rights, and the original is left alone.

## Settings, themes and updates

![The settings page](screenshots/en-8-settings.png)

The menu at the top right (☰) holds everything that isn't needed to get files back:

- **Settings**: the language, the colour theme, expert mode, whether to offer the deep search at start, updates, and information about the app.
- **Colour theme**: light, dark, Nord, Catppuccin, Tokyo Night, Solarized, Monokai or Gruvbox. The sun/moon button next to the language switches between light and dark in one click.
- **Check for updates**, **Prepare a report…** and the project's page on GitHub.

When a new version is out, the bottom bar shows **→ v…: update**. One click downloads it, checks that it is signed with the project's key, installs it and restarts Open Recovr. If a search or a recovery is running, the update waits for it to end.

## Expert mode

![Expert mode with the search log](screenshots/en-7-expert.png)

**Expert mode**, in the menu at the top right, shows the search log under the list: the drive and file system, each phase, and PhotoRec's own lines. **Copy all** copies the log, which is useful for a bug report. Drag the top edge of the log to make it taller.

Each search is also written to `journal.txt` next to the settings (`%APPDATA%\Open Recovr` on Windows). The **Log** link at the bottom opens it.

## Asking someone for help

In the menu at the top right (☰), **Prepare a report…** saves a zip you can send to someone who helps you, by e-mail or any other way. Open Recovr only writes the file; it never sends it. Inside are plain text files: what the report is, the system and drives (model, size, file system), the settings and the log of the last search. Before anything is written, your account name, the computer's name, your home folder and e-mail addresses are masked, and so are the document titles PhotoRec puts in the names of the files it finds. Open the zip and read it before passing it on.

## FAQ

**My file isn't in the list. What now?**
Make sure the right drive is selected and the type filter isn't hiding it (*All*). Then try the deep search, and on NTFS the full analysis. If the drive has been heavily used since the deletion, the file may have been overwritten. No software can bring it back then.

**The recovered files have names like `f1234567.jpg`.**
Those files were found by PhotoRec, which recognises a file by its content, not by the file table, so their original name is unknown. When the file itself says nothing about its date or title, the number stays. Sort by date or size, and use the preview to find yours.

**A file is marked Damaged. Is it worth recovering?**
Often, yes. A damaged photo may show only its top half, a damaged video may play until the damaged part. Recover it and try it: it costs nothing, and the drive is never changed.

**Can it recover files from my phone?**
Not over USB. A phone only shares the files it still has, and its memory is encrypted. Look in the bin of the Gallery or Photos app (deleted photos are usually kept for 30 days), in Google Photos or iCloud, or put the phone's microSD card in a card reader and search that.

**Does it work on an SSD?**
Partly. Most SSDs erase deleted blocks in the background (TRIM), so files deleted a while ago are often gone for good. The Recycle Bin and the file table still help for recent deletions. Act quickly.

**Can I recover files from a formatted drive?**
Often, yes, with the full analysis. A quick format leaves the contents in place. A full format (or "secure erase") overwrites them.

**Why does the deep search need administrator rights?**
It reads the drive sector by sector, below the file system, and the operating system reserves that for administrators. Open Recovr only reads; it never writes to that drive.

**Is it safe? Does it send my data anywhere?**
Open Recovr works offline. It has no account and no telemetry. Its only connection is the update check: at startup it asks GitHub for the number of the latest release, and sends nothing else. Untick "Check for updates at startup" in Settings to turn it off; the menu can still check on request. An update is installed only if it is signed with the project's key, which never goes to GitHub: a fake version put there by someone else is refused. The code is open: see for yourself.

**Linux / macOS: the deep search says PhotoRec isn't available.**
Install it: `sudo apt install testdisk` (Debian, Ubuntu), `sudo dnf install testdisk` (Fedora) or `brew install testdisk` (macOS). Open Recovr then finds `photorec` on its own. Without it, a simpler built-in engine searches for the most common formats.
