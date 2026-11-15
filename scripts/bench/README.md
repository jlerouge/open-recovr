# Carving benchmark

This benchmark measures recovery engines (the built-in carver, PhotoRec, or any other tool that writes its files into a folder) on disk images whose content is known. It does not replace `open-recovr --bench`: that one is a small regression test on a synthetic image, and its results flatter our own engine by construction.

## Pieces

| Piece | Role |
|---|---|
| `open-recovr --carve <image> <dir>` | Runs the built-in carver over a whole image and writes out every file it finds. |
| `open-recovr --bench-score <truth.tsv> <dir> [image] [label]` | Compares any recovered folder with the truth file. The engine itself is never looked at. |
| `run.ps1` | Runs each engine, times it, scores it and appends one line per engine to `results.tsv`. |
| `make-vhd.ps1` | Builds a realistic image as administrator: Windows formats it, fills it from a corpus of real files, ages it and deletes files, and records the truth before each deletion. |
| `collect-commons.ps1` | Gathers a corpus of public-domain and CC0 files from Wikimedia Commons, with a manifest of their sources, for images that can be published (see [docs/dataset.md](../../docs/dataset.md)). |
| `make-derived.ps1` | Converts corpus files into the formats Commons lacks: video and sound containers (ffmpeg), image formats (ImageMagick), HEIC (the Windows HEIF encoder), archives (Windows tar and makecab) and Office documents (Word, Excel and PowerPoint through COM). |
| `collect-govdocs.ps1` | Copies the files of extracted govdocs1 threads (real US government documents: Word 97, Excel 97, PowerPoint 97, PostScript...) with a list of their sources. |

## The truth file

A tab-separated file with one line per file:

```
sha256  head  size  state  pieces  extents  name
```

- `head`: the sha256 of the first 4 KiB, or of the whole file if it is shorter.
- `state`: `deleted` or `live`.
- `pieces`: the number of fragments on disk.
- `extents`: `offset+length` byte ranges counted from the start of the image, or `-` when unknown.

## What is counted

Only deleted files count. Each one is scored as follows:

| Status | Meaning |
|---|---|
| exact | Byte for byte. |
| trim | The right bytes followed by extra ones. The file opens like the original. |
| damaged | Starts right and then goes wrong. Typically a fragmented file read straight through. |
| missing | Not recovered. |

- **Recall**: (exact + trim) / deleted files.
- **"of int."**: the same recall, but counted only over the files whose extents still hold the original bytes in the image. No engine can bring back a file that has been overwritten.
- **Junk**: a recovered file that matches no file in the truth, whether live or deleted. Precision is 1 − junk / recovered.
- **Live files**: finding a live file counts neither as a success nor as junk.
- **Copies**: when the same content was on the disk several times (a file copied, then deleted, or written again in a later round), it counts once. It is live if one copy is live; otherwise it is intact if one copy is still whole. Without this, one file recovered could count as several, or as an overwritten copy found again. (This rule was added after the first results on the aged images, where it had pushed the recall of intact files above 100 %.)
- With `SCORE_DETAIL=1`, every deleted file not recovered exactly is listed, and so is every junk file. Each listed file is marked `intact` (still whole on the disk, so recoverable in principle) or `overwritten`, when an image is given.

Each figure is broken down into contiguous and fragmented files, and by extension.

## Data

- Images made by `make-vhd.ps1` from a corpus of real files. Its `fragmented` scenario cuts the free space into small holes first, so that many files end up in pieces.
- NIST CFReDS "File Carving" images (L0 to L5: contiguous, fragmented, out of order, missing pieces, nested, braided) and the DFRWS 2006 challenge. Their layout documents are converted into truth files.

The metrics are fixed before any result is looked at, and they are not changed to suit an engine.
