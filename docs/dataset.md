# A public carving data set (work in progress)

The aged disk images in [benchmarks.md](benchmarks.md) cannot be shared, because their corpus holds copyrighted files. The public data sets that can be shared (NIST CFReDS, DFRWS 2006) are small, old and were used to tune the engine. This page describes a data set meant to fill that gap: disk images that anyone may download, with the full truth, on which any carver can be measured.

## Goals

- **Shareable without conditions.** Every file on the images is in the public domain or under CC0, so the images can be published under CC0 too.
- **Old and new formats.** MIDI, MPEG-1, DjVu and GIF next to WebP, FLAC, Opus, WebM, SVG and today's PDF.
- **Realistic damage.** File systems made by Windows itself, aged by rounds of writes and deletions, then deleted, quick-formatted or refilled. Also, disks where the free space was cut into small holes before the files were written, so that many files are fragmented. Windows alone leaves little fragmentation (see [benchmarks.md](benchmarks.md)).
- **Full truth.** For each file: its hashes, its size, whether it is deleted or live, and where its pieces lie when the file system says so.
- **Not tuned on.** The images used to measure are built with seeds chosen once, before any engine is run on them. The scripts can build new images with other seeds for anyone who suspects over-fitting.

## The corpus

`scripts/bench/collect-commons.ps1` gathers files from [Wikimedia Commons](https://commons.wikimedia.org/). A file is taken only when both its structured data and its description page say CC0 or public domain, with no further restriction (trademarks, personality rights). Each file is checked against the SHA-1 given by Commons and listed in `manifest.tsv` with its title, author, licence and address, so the corpus can be fetched again file by file.

| Kind | Formats |
|---|---|
| Pictures | JPEG, PNG, GIF, TIFF, WebP, SVG, DjVu, XCF |
| Documents | PDF |
| Sound | FLAC, WAV, MP3, MIDI, Ogg (Vorbis, Opus) |
| Video | WebM, MPEG-1/2 |
| Other | STL (3D models) |

Commons holds only open formats. Two more sources add the containers and the formats of particular programs that disks are full of.

**Converted from the corpus** by `scripts/bench/make-derived.ps1`. The results are as free as their sources.

| Written by | Formats |
|---|---|
| ffmpeg (video, cut to 20 s) | MP4 (H.264, H.265), MKV (VP9, H.264), AVI (MPEG-4, MJPEG), MOV (H.264, ProRes), 3GP, MPEG-TS, FLV, WMV, MXF, DV |
| ffmpeg (sound) | M4A (AAC), AIFF, AC-3, WMA |
| ImageMagick | AVIF, JPEG XL, PSD, BMP, ICO, TGA, PCX, JPEG 2000, DDS, SGI, PPM, PICT, EPS |
| Windows HEIF encoder | HEIC |
| Windows tar (libarchive) and makecab, a few corpus files each | ZIP, 7z, TAR (plain, gzip, bzip2, xz), ISO 9660, cpio, CAB |
| Microsoft Word, reports of made-up words with a table and corpus pictures | DOCX, DOC, RTF, ODT |
| Microsoft Excel, tables of random figures with a chart | XLSX, XLS, ODS |
| Microsoft PowerPoint, slides of corpus pictures | PPTX, PPT, ODP |

The Office files and the HEIC files are written by the programs that write them on real disks, so their inner layout is the real one. The files made by ffmpeg and ImageMagick are valid, but a phone, a camera or Photoshop lays out its files in its own way: these are a weaker stand-in, and `derived.tsv` names the tool behind each file so results can be split.

**govdocs1** ([Digital Corpora](https://digitalcorpora.org/corpora/file-corpora/files/)), collected by `scripts/bench/collect-govdocs.ps1`. These are about a million real files gathered from web servers of the US government: Word 97, Excel 97 and PowerPoint 97 documents, PDF, PostScript, RTF, dBase, as their authors saved them. Works of the US federal government are not protected by copyright in the US, and Digital Corpora distributes the corpus for free reuse. A few files may come from contractors or be quoted from elsewhere: this should be stated with the data set, and a file removed if someone claims it. Files a carver cannot find by a signature (plain text, CSV, logs) are left out.

`scripts/bench/make-credits.ps1` writes the credits of a set of images: the author, licence and Commons page of every work on them, including the sources of each converted file, archive and document, and the govdocs1 files with the citation Digital Corpora asks for. None of these files needs credit, but they are credited all the same, and a file with no known source stops the script.

Still missing: a real Photoshop PSD, a HEIC from a phone, raw camera files (CR2, NEF, ARW, DNG) and RAR archives. Free sources for these are still to be found.

## The images

Built by `scripts/bench/make-vhd.ps1` (see [scripts/bench/README.md](../scripts/bench/README.md)):

| Image | File system | Scenario |
|---|---|---|
| ntfs-deleted | NTFS | aged, files deleted |
| ntfs-fragmented | NTFS | free space cut into holes first, then aged, files deleted |
| ntfs-quickformat | NTFS | aged, then quick-formatted |
| exfat-camera | exFAT | a camera card, photos deleted and taken again |
| fat32-fragmented | FAT32 | like ntfs-fragmented |

In the `fragmented` scenario, filler files of random bytes take up to 90 % of the volume, then half of them are deleted at random. The files written afterwards are split across the holes. The filler is not part of the truth: a carver that returns it gets junk.

## Scoring

The scorer and the metrics are those of [carving.md, section 6](carving.md#6-how-the-engine-is-measured) and [scripts/bench/README.md](../scripts/bench/README.md). They were fixed before the data set was built.

## First results

These are Open Recovr 0.4.0 and PhotoRec 7.2 on the five images. Each image is 2 GB and was built by `make-vhd.ps1` with seeds 11 to 15, in the order of the table above. Both engines ran once, on the same PC, with the images on a SATA SSD. The engine was not changed after these runs. The images were built a second time before publication, once the Office documents on them were found to carry the name of the account that saved them (`make-derived.ps1` now removes it); these are the results on the published images.

| Image | Deleted | Still intact | Recovered whole (Open Recovr / PhotoRec) | Recall of intact | Junk files | Seconds |
|---|---|---|---|---|---|---|
| ntfs-deleted | 771 | 270 | 219 / 236 | 81.1 % / 87.4 % | 52 / 326 | 73 / 55 |
| ntfs-fragmented | 376 | 117 | 67 / 70 | 57.3 % / 59.8 % | 499 / 1,609 | 81 / 29 |
| ntfs-quickformat | 1,252 | 760 | 587 / 615 | 77.2 % / 80.9 % | 21 / 332 | 125 / 83 |
| exfat-camera | 12 | 12 | 11 / 10 | 91.7 % / 83.3 % | 14 / 292 | 121 / 232 |
| fat32-fragmented | 415 | 415 | 84 / 90 | 20.2 % / 21.7 % | 99 / 232 | 137 / 21 |

"Recovered whole" counts the exact files plus the trimmable ones, which are whole but followed by extra bytes. What these runs show:

- **Recall** is close. PhotoRec is ahead by 1.5 to 6 points on four images, almost entirely through trimmable files: it stops a file late where Open Recovr drops the file or cuts it exactly. Open Recovr is ahead by one file on exfat-camera. Counting exact files only, Open Recovr is ahead on three images and behind by two or three files on ntfs-deleted and ntfs-fragmented.
- **Precision** is where the engines differ most. On every image PhotoRec returns 2.3 to 21 times as many junk files: 1,609 against 499 on ntfs-fragmented, 292 against 14 on exfat-camera.
- **Fragmented files** are still lost by both. On ntfs-fragmented, 36 deleted files were split and still intact, and neither engine gives one back; over the three NTFS images, Open Recovr recovers 2 of these 98 files and PhotoRec 3.
- **Speed** goes to PhotoRec on four images, over six times faster on FAT32. Open Recovr is faster on exfat-camera only.
- **Few deleted files survive** the aging: rounds of writes overwrite most of them, which is what happens on a disk in use. exfat-camera keeps only 12, too few to tell the engines apart.
- **FAT32 and exFAT images** have no extents in their truth. A file there counts as fragmented only when the scorer knows it, so their "still intact" column holds every deleted file and their recall is underestimated.

## Still open

- Where to publish: compressed with zstd, each image fits under the 2 GB limit of a GitHub release asset; Zenodo would add a DOI.
- The truth of FAT32 and exFAT images has no extents, since Windows does not report them on these file systems. They could be read from the FAT before each deletion.
- A short write-up of the method and the results, if the data set proves useful.
- Larger images, or more of them per scenario: on a 2 GB disk few deleted files survive the aging, so one image says little about a single format.
