# Benchmarks: Open Recovr's modern engine and PhotoRec

These figures compare Open Recovr's own recovery engine (the "modern" engine, see [carving.md](carving.md)) with [PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec) 7.2 by Christophe Grenier, which Open Recovr's "classic" engine calls. PhotoRec has been the reference open-source carver for twenty years, and it laid the groundwork that tools like this one build on.

Both engines are scored by the same tool, which only looks at the files each engine wrote out (see [carving.md, section 6](carving.md#6-how-the-engine-is-measured) for the metrics). PhotoRec is run directly, with its default file types, over the whole image.

To reproduce, from the `app` folder:

```powershell
.\scripts\bench\run.ps1 -Image <image> -Truth <truth.tsv> -Out <folder> -Engines ours,photorec
```

## Public data sets

Measured on 2026-11-14 with the development version of Open Recovr after 0.3.1 and PhotoRec 7.2, on Windows 11.

### NIST CFReDS "File Carving", graphics images

Each image holds 6 or 7 pictures (BMP, GIF, JPEG, PCX, PNG and TIFF).

| Image | What it tests | Deleted | Intact | Modern: exact + trim | PhotoRec: exact + trim | PhotoRec: damaged |
|---|---|---:|---:|---:|---:|---:|
| L0 | contiguous files | 6 | 6 | **6** | **6** (2 trimmed) | 0 |
| L1 | fragmented, pieces in order | 6 | 6 | **6** | **6** (2 trimmed) | 0 |
| L2 | fragmented, pieces out of order | 6 | 6 | **6** | 0 | 2 |
| L3 | missing pieces | 6 | 0 | 0 | 0 | 1 |
| L4 | nested files | 7 | 7 | **7** | 5 (1 trimmed) | 1 |
| L5 | braided files | 6 | 6 | **6** | 0 | 2 |
| **Total** | | 37 | 31 | **31 / 31** | 17 / 31 | |

In L3 no file can be rebuilt whole, since pieces are missing on purpose. Both engines return one damaged file there, and PhotoRec also returns one junk file. L5 holds one picture twice, counted once.

### DFRWS 2006 carving challenge

32 deleted files (Word, Excel, HTML, JPEG, text and ZIP), 17 of them fragmented.

| | Modern | PhotoRec 7.2 |
|---|---:|---:|
| Exact | **24** | 16 |
| Recall, contiguous files | 12 / 15 | 12 / 15 |
| Recall, fragmented files | **12 / 17** | 4 / 17 |
| Damaged | 4 | 3 |
| Junk (files that match nothing) | **1** | 6 |
| Precision | **96.6 %** | 76.0 % |
| Time | 1.9 s | 5.1 s |

Neither engine recovers the 3 text files. Both get 2 of the 6 HTML files. These formats have no structure to check, so their pieces cannot be told apart.

### What these figures do and do not show

- The data sets are small (69 distinct deleted files in all).
- They were used while the modern engine was being developed, so they are not independent of it. They show that the methods work on the cases they were designed for, not how the engine behaves on any disk.
- PhotoRec covers far more file types (several hundred) than the modern engine (about forty). These tests only include types that both handle.

## Aged disk images

To check the engine on fragmentation it was not tuned on, disk images are built by Windows itself (`scripts/bench/make-vhd.ps1`). A virtual disk is formatted, filled from a corpus of real files, then aged over 12 rounds of deletions and rewrites. The truth, including where each file lay, is recorded before each deletion. Neither the corpus nor the images are published, since they contain copyrighted files. Only the scripts and the figures are.

Measured on 2026-11-14, same versions as above. The corpus holds 517 files: PNG, OGG, WAV, MP3, BMP, JPEG, fonts, WebP, icons, ZIP, PDF, gzip, MP4, Office documents, AVI, GIF and TIFF, between a few KB and 35 MB.

| Disk | File system | Scenario | Used for tuning? |
|---|---|---|---|
| ntfs-deleted | NTFS | files deleted and rewritten | **yes** |
| ntfs-quickformat | NTFS | the same, then a quick format | no |
| exfat-camera | exFAT, 32 KB clusters | a camera card: photos taken, some deleted, more taken | no |
| fat32-deleted | FAT32, 4 KB clusters | files deleted and rewritten | no |

The first disk was used to find and fix weaknesses of the modern engine. The three others were built with other seeds and only looked at once the fixes were in, so they show how the engine does on disks it was not tuned on. One more fix was made after that (a ZIP end record taken from another archive, see below). Both the figures before and after it are given.

"Intact" counts the deleted files whose bytes are all still on the disk. No engine can do better than that. Times are the best of two runs with the image already in the disk cache, alternating which engine runs first. The very first run of each image reads it from the disk and is much slower, whichever engine it is.

### ntfs-deleted (used for tuning)

159 deleted files, 87 of them intact, 1 of these fragmented.

| | Exact | Damaged | Junk | Time |
|---|---:|---:|---:|---:|
| Modern, before tuning | 77 | | 22 | |
| Modern, after tuning | **85** | 1 | **10** | 12.6 s |
| PhotoRec | **85** | 1 | 163 | 8.1 s |

### ntfs-quickformat (not used for tuning)

498 deleted files, 441 of them intact, 4 of these fragmented.

| | Exact | Trimmable | Damaged | Junk | Time |
|---|---:|---:|---:|---:|---:|
| Modern, first run | **431** | 0 | 3 | 7 (37.5 MB) | 12.8 s |
| Modern, after the ZIP fix | **431** | 0 | 2 | **6** (3.7 MB) | |
| PhotoRec | 408 | 10 | 10 | 31 | 11.1 s |

Each engine rebuilds 1 of the 4 fragmented files.

### exfat-camera (not used for tuning)

The camera scenario takes most photos again from the same files, so most deleted contents still have a live copy and are not counted. 21 deleted contents have none. 4 of them were overwritten, which leaves 17 that can be recovered. No file is fragmented.

| | Exact | Damaged | Junk | Time |
|---|---:|---:|---:|---:|
| Modern | 16 | 1 | **0** | **4.9 s** |
| PhotoRec | 16 | 1 | 12 | 12.1 s |

The damaged file is the same AVI for both. Its header gives a size shorter than the file (see the corpus notes below). PhotoRec's junk is 11 EXIF thumbnails it carves out of the photos as separate pictures, and one 11 MB text file.

### fat32-deleted (not used for tuning)

80 deleted files. 64 are whole and in one piece on the disk. The 16 others are overwritten or fragmented: FAT32 does not say where a file lay, so the two cannot be told apart.

| | Exact | Trimmable | Damaged | Junk | Time |
|---|---:|---:|---:|---:|---:|
| Modern | **62** | 0 | 2 | **42** | 20.4 s |
| PhotoRec | 57 | 3 | 3 | 61 | **13.1 s** |

The modern engine misses a TIFF and a file named `.zip` that is not a ZIP. 41 of its 42 junk files are pieces of MP3 files: the middle or end of real corpus files whose start was overwritten or that are fragmented. MP3 is a stream of frames, so such a piece looks valid. PhotoRec's junk is 35 text files, 15 JPEG thumbnails, 7 MP3 and 4 OGG pieces.

### What was fixed, and what was not

On the tuning disk, the modern engine first got 77 files and 22 junk files. The fixes made after looking at it are general ones, not tied to these files:

- (Found on ntfs-quickformat, after its first run.) A ZIP whose central directory is lost is no longer glued to the end record of the next archive on the disk. Such a file was tens of MB of mixed data. The end record is now taken only if the directory before it lists the file's first entry, under its name.
- Icons, TrueType/OpenType fonts and gzip files are now recognised, each measured from its own structure: the directory of images or tables, or the deflate stream and its trailer.
- A PDF runs to its last `%%EOF` (linearized files, incremental updates) instead of stopping at the first.

One miss was left as it is on purpose. A TIFF on the FAT32 disk ends one byte after the size its structure gives, because its writer padded it to an even length. Rounding every TIFF up to an even size would bring this one file back and cut one byte off every TIFF that has an odd length. The corpus holds a single TIFF, which is too little to tell which case is more common.

### Corpus notes

The corpus is a copy of real files, taken as they are. Some do not match their extension, which affects both engines alike:

- 6 of the 12 `.zip` files are not ZIP archives. The scorer compares contents, not names, so they count as what they really are.
- One `.jpg` is a 35 MB GIF and one `.gif` is cut short.
- One `.avi` has bytes after the end its header gives. Both engines stop at that end, so both get it as damaged.
- There is no plain text in the corpus. PhotoRec carves text files and the modern engine does not, so this costs PhotoRec nothing in recall but explains part of its junk.

### What these figures show

- On the three disks it was not tuned on, the modern engine recovers as many or more files than PhotoRec (431 against 418, 16 against 16, 62 against 60, counting trimmable files) and returns far fewer junk files.
- It is not always faster. PhotoRec is faster on both NTFS disks and on FAT32. The modern engine is faster on exFAT.
- Windows leaves little fragmentation on these disks (5 fragmented intact files on the two NTFS disks, none on exFAT). They test the engine on ordinary disks more than on fragmentation. The public data sets above test fragmentation.
- MP4 and MP3 files in pieces are not put back together by either engine.
