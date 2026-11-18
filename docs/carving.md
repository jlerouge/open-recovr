# How Open Recovr finds and rebuilds files

This document describes the methods used by Open Recovr's own recovery engine (the "modern" engine) for the full scan: how files are detected in raw sectors, how their length is worked out, and how files cut in pieces by the file system are put back together. It also explains how the engine is measured.

Open Recovr also has a "classic" engine, which calls [PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec) by Christophe Grenier when it is installed. PhotoRec is not included with Open Recovr. The modern engine shares no code with it: it was written from the format specifications (ITU-T T.81 for JPEG, the PNG, GIF, TIFF, ZIP and compound-file specifications, ISO/IEC 14496-12 for MP4, and so on).

The code lives in `src/`:

| File | Role |
|---|---|
| `carve.rs` | Header detection, first pass, length of each format |
| `frag.rs` | Second pass: free sectors, candidate search, the pieces of a file |
| `jpeg.rs`, `png.rs`, `gif.rs`, `bmp.rs`, `tif.rs`, `pcx.rs`, `ole.rs`, `pkzip.rs` | What each format can tell about its own pieces |
| `score.rs` | The scorer used by the benchmarks |
| `engine.rs` (`deep_scan`) | Runs the two passes over a disk and lists the files |

## 1. Overview

The full scan reads the whole disk, sector by sector, without relying on the file system. It works in two passes.

1. **First pass.** Each sector is checked for a known file header. For each header, the engine works out how long the file is by reading its structure, and decides what it found:
   - **Whole**: the file reads straight through to its end.
   - **Broken**: the file starts well but its own structure goes wrong before its end. Such a file was most likely cut in pieces by the file system, and its rest lies elsewhere.
   - **None**: the header was a false match.

   Whole files are kept, and the sectors they cover are claimed.
2. **Second pass.** For each broken file whose format allows it, the engine looks for the next piece among the sectors that no file claimed, accepts a piece only when the format reads on through it without a fault, and goes on until the file ends. The pieces found are claimed in turn, so no other file can take them.

A file whose format offers nothing to check is kept as the first pass read it, and its preview shows whether it opens.

## 2. Detection (first pass)

`carve::detect` looks at the first bytes of a sector and names the kind of file that starts there: JPEG, PNG, GIF, BMP, WebP, PDF, ZIP and the formats built on it, MP4/MOV/M4A/HEIF/CR3, AVI, WAV, MP3, TIFF and the raw formats built on it, OLE compound files, PCX, HTML/XML, Matroska/WebM, MPEG-TS, MPEG-PS, FLAC, Ogg, AAC, ASF, FLV, AIFF, CAF, RAF, CRW, PSD, MIDI, PostScript and DjVu.

Files are written from the start of a cluster, so headers are only looked for on sector boundaries. A magic number alone is never enough. Each detector also checks the fields that follow (a valid version, sizes within range, a known chunk type), so a few random bytes rarely pass.

### Headers inside a file

A header found inside a file that was already found is not ignored. A file written into a gap of a fragmented one also starts on a sector boundary. The header is skipped only when the outer file is known to hold such headers itself: the entries of a ZIP, the frames of a video, the previews of a raw photo, the objects embedded in an Office file (`Kind::holds`).

### Length

`carve::measure` works out each file's length from its structure, never by guessing a size:

- **Chained formats** (PNG, MP4, RIFF, FLV, CAF, Matroska): a chain of chunks or boxes, each one with its size, walked to the end.
- **Formats with an end marker** (JPEG, GIF, PDF, MPEG-PS): read up to that marker. JPEG is decoded as far as needed (see section 4.1), so a 0xFFD9 found inside a thumbnail does not end the main picture. A PDF ends at its last `%%EOF`, not its first: a linearized PDF states its own length in its first object (used when a `%%EOF` really ends there; an incremental update written after it is still followed), and an incremental update is followed past each `%%EOF` as long as an object or a cross-reference table comes next.
- **Formats with a directory** (ZIP, OLE, TIFF, RAF, CRW, ICO, TrueType and OpenType fonts): the directory says where the parts are and how far the file goes.
- **gzip**: the deflate stream carries no length, so it is inflated to its end, and the CRC and size in the trailer must match what came out.
- **Frame streams** (MP3, AAC, MPEG-TS, Ogg, FLAC): consecutive frames as long as they keep the same format. FLAC frames carry no length, so each one ends where its checksum matches.

Each format also has a maximum size (`Kind::max_size`), so one bad length cannot swallow the disk.

The same reading gives the file's real extension: a ZIP may be a .docx, .xlsx, .odt, .epub or .jar; an MP4 may be a .mov, .m4a, .heic or .cr3; a TIFF may be a .dng, .nef or .arw.

## 3. Rebuilding fragmented files (second pass)

### Where the cut is

The format says where the first piece stops making sense: the JPEG Huffman decoder hits an invalid code, a PNG chunk fails its CRC, a ZIP entry no longer inflates to its CRC, the rows of a bitmap stop looking like the rows above them. The cut lies at one of the few sector boundaries just before that point, and these are tried from the nearest one down.

### Where the next piece may be

Candidates are the sectors that no file found so far claims. They are tried nearest to the cut first, on both sides of it: up to 64 MiB ahead and 32 MiB behind (`frag::AHEAD`, `frag::BEHIND`). File systems usually put the rest of a file in the next free space, a little further on, so most pieces are found within the first few tries.

The candidate list is fed from the map of claimed ranges as the search moves on, in the direction it is going, so the search stays cheap on large disks.

A file can have up to 8 pieces (`frag::MAX_PIECES`).

### When a piece is accepted

A piece is accepted only when the format reads on through it for a while. Each format below gives its own test, and each test is strict enough that random data, text or a piece of another file of the same kind fails it.

When several candidates pass, the best one wins: the one that fits most smoothly, or the one that leads to a complete file.

### Where the last piece is

Some formats put their directory at the end of the file: ZIP (the central directory and its end record), OLE (the FAT), many TIFF writers (the IFD). This directory is searched for on the disk. Where it is found tells where the last piece lies, so the engine only has to find the cut between the first piece and the last one, not the whole path.

## 4. What each format can tell

### 4.1 JPEG (baseline)

Written from ITU-T T.81, sections F.1.2 and F.2.2.

The coded data is decoded as far as needed to tell where it stops making sense. No picture is made: there is no full inverse DCT. Each of the following is checked:

- each Huffman code;
- each coefficient position and range;
- each restart marker, in sequence.

Random bytes, text, or the coded data of another picture fail these checks within a few hundred bytes.

At the fault, the decoder starts again from a state it saved just before the cut (one is kept every sector or so), on each candidate sector. Several pictures coded the same way could all decode, so the luma blocks are used to tell them apart:

- For the top and bottom rows of each block only, the pixels are computed from the dequantized coefficients (a partial inverse DCT).
- The row of blocks after the join must carry on the row above it. A small gap means a good join; a large gap rules the candidate out.

A candidate must also decode at least 8 KiB past the join before it is believed (`jpeg::TRIAL`).

Progressive JPEGs are measured but not rebuilt.

### 4.2 PNG

A PNG is a chain of chunks, each one closed by a CRC-32 of its type and data. Writers split the image data into chunks of a few kilobytes, so a cut always falls inside a chunk, which then fails its CRC. The bytes before that chunk are known good, and the cut lies at one of the few sector boundaries inside it.

A candidate is accepted only when two things hold:

- the chunk's CRC comes out right through it;
- the next chunk reads as a chunk.

A wrong piece passes that test with odds of about one in four billion.

### 4.3 GIF

A GIF stores its picture as one LZW stream, cut into sub-blocks of up to 255 bytes, with no checksum.

The stream has rules that a stretch of another file seldom keeps for long:

- every code names an entry the dictionary already holds;
- no colour lies past the palette;
- the stream does not end before the last pixel.

Past a cut, the codes still decode to strings of the picture's own pixels (the dictionary is the same), but in the wrong places: the rows stop carrying on from the rows above.

The next piece is the free sector that, decoded from the decoder's state at the cut, carries the rows on as smoothly as the picture did, and keeps doing so for a while. An interlaced picture is decoded in its own order, and each row is compared with the row decoded just before it.

### 4.4 BMP

An uncompressed bitmap has no checksum and no markers: its pixels are all the structure it has. In a photograph, each byte is close to the byte one row above it. Where a piece ends, this closeness breaks at once, at the start of a sector.

The next piece is the free sector whose bytes, put after the cut, carry on the rows above as smoothly as the picture did before, and keep reading as a picture for a while after.

### 4.5 TIFF

Uncompressed strips are rows of pixels, so their pieces are found as for a bitmap.

Many writers put the directory (the IFD) and the strip tables after the pixels, at an offset the file header gives. When the file is cut, that offset points into some other file. The IFD is then searched for on its own: a free sector near the start of the file that holds, at the right place in the sector, a directory whose strips fit the file. Where it lies says where the last piece is, and the cut before it is tried there first.

### 4.6 PCX

The scan lines are run-length encoded, with nothing else to check them by. Two kinds of evidence are combined:

- the rows of a photograph change little from one to the next;
- the encoding has rules of its own: no run is empty, and equal pixels side by side are written as a run, not one by one.

Both break at once where a piece ends. The next piece is chosen as for a GIF: decoded from the state at the cut, it must carry the rows on smoothly.

### 4.7 OLE compound files (Word, Excel and PowerPoint 97–2003)

A compound file is a small file system of 512-byte sectors: a FAT chains the sectors into streams, and a directory names the streams. It has no checksum, so the file is checked through what its parts say about each other:

- every FAT sector holds valid entries, and the FAT marks its own sectors as such;
- every chain stays inside the file and ends;
- the directory reads as one;
- an Excel workbook is a chain of records whose lengths lead from one record to the next;
- in a Word document, the paragraph tables say where each paragraph ends, and a paragraph mark must stand there in the text.

Writers put the FAT near the end of the file, so in its last piece. The FAT sector that carries the marks of the other FAT sectors is searched for on the disk, and it places that last piece. The cut between the first piece and the last is then searched from the top down. Each check that fails in the first piece pulls the bound down to where it failed.

### 4.8 ZIP and the formats built on it (docx, xlsx, odt, epub, jar…)

Each entry carries the CRC-32 of its content. Inflating an entry tells whether its bytes are all there, and where the first wrong byte is, give or take a few hundred bytes.

The end-of-central-directory record says where the central directory starts in the file. Found on the disk, it places the last piece exactly. What is left is the cut between the first piece and the last, searched from the top down: each failure pulls the bound down to where the data first went wrong.

### 4.9 PDF

A PDF ends with `startxref`, the place of its cross-reference table, and `%%EOF`. The table gives the offset of every object in the file. A whole PDF is checked against it: each object listed must start where the table says (a sample of at most 4096 are read). A PDF whose objects are not where its table says is broken, even if its end was found.

To rebuild one, the `startxref … %%EOF` lines near the first piece are searched for on the disk. Each gives a table, which places the last piece: the table's own offset in the file and its place on the disk must agree, sector for sector. A linearized file's last `startxref` points to the small table near its start, whose `/Prev` leads to the main one. Every object the tables list is then put in a piece: under the shift of the piece before it, under the last piece's shift, or, when neither fits, under a new shift found by searching for `n g obj` on the disk. The cuts between pieces are searched from the top down. A piece is accepted when its objects read on to `endobj`, a stream is as long as its `/Length` says, and a compressed stream inflates with a good checksum.

PDFs whose table is itself a compressed stream (about 3% of them) are measured but not rebuilt.

### 4.10 MP3

Each MPEG audio frame starts with a header that gives its length. After the header comes the side information: `main_data_begin`, how many bytes of this frame's audio lie back in the frames before it (the bit reservoir), and the length in bits of each granule's data. From these, each frame tells how many bytes of the reservoir it leaves to the next one, and the next frame's `main_data_begin` must say the same. Most encoders keep this chain exact, frame after frame. A frame of another song lands on a matching value only about once in 400 times, and only the frames at a join can show a wrong join: the other song's own frames agree with each other.

A Xing or Info frame at the start of a song gives the length of its audio. A song whose frames stop before that length is broken. Without that frame, a song that has an ID3 tag and whose frames stop on something that is neither silence nor a tag is broken too. After the frames may come an APEv2 tag and the 128-byte ID3v1 one.

To rebuild a song, the free sectors whose first bytes start a run of frames are noted during the scan. Each cut on a sector bound in the last frames read is tried against them. The frame across the cut must lead to a header of the same format at the right place in the candidate sector, and the reservoir chain must hold over the frames on both sides of the join and on for 8 frames more, or to the end of the song. The song is followed this way through up to 32 pieces, to the length its Info frame gives, or to a tag or to silence.

### 4.11 Other formats

MP4, MOV, Matroska, AVI, WAV, FLAC, Ogg, MIDI, PostScript, DjVu and the others are found and measured by the first pass. A fragmented file of these kinds is not rebuilt yet. It is listed as the first pass read it, and its preview and integrity check show whether it plays or opens.

## 5. After recovery

`inspect.rs` looks inside each file that was found:

- it reads the date, camera or title written in it, and uses them for a better name than a bare offset;
- it checks whether the file is whole, by decoding the image or by checking the archive's CRCs;
- it computes a fingerprint of the content, to group copies of the same file.

## 6. How the engine is measured

### The scorer

`open-recovr --bench-score <truth.tsv> <dir> [image]` compares any folder of recovered files with a truth file that lists the files really on the disk. It never looks at how the folder was made, so PhotoRec, the modern engine or any other tool can be scored the same way. `scripts/bench/run.ps1` runs each engine, times it, scores it and appends the results to a table.

Only deleted files count. Each one is scored as follows:

| Status | Meaning |
|---|---|
| exact | Byte for byte. |
| trim | The right bytes followed by extra ones. The file opens like the original. |
| damaged | Starts right and then goes wrong. Typically a fragmented file read straight through. |
| missing | Not recovered. |

- **Recall** is (exact + trim) / deleted files.
- **Recall of intact files** counts only the files whose bytes are still all on the disk. No engine can bring back a file whose data was overwritten.
- **Junk** is a recovered file that matches no file on the disk. Precision is 1 − junk / recovered.

The metrics were fixed before any result was looked at, and they are not changed to suit an engine. There was one correction since: copies of the same content are counted once, as one recovered file could otherwise count twice. It applies to both engines alike. Details are in [scripts/bench/README.md](../scripts/bench/README.md).

`open-recovr --bench` is a separate, small regression test on a synthetic image. It is built by us for our own engine, so its results flatter it, and it is not used for any comparison.

### Data sets

- **NIST CFReDS "File Carving" images (graphics)**, L0 to L5: contiguous files, fragmented files, files out of order, missing pieces, nested files and braided files. The layout documents are converted into truth files by `cfreds-truth.sh`.
- **The DFRWS 2006 carving challenge**, converted by `dfrws2006-truth.sh`.
- **Aged disk images** built by `make-vhd.ps1`. Windows formats a virtual disk (NTFS, exFAT or FAT32), fills it from a corpus of real files, then deletes and rewrites files over several rounds. File system fragmentation thus comes from Windows itself, not from a generator written by us. The truth, including each file's extents, is recorded before each deletion.

The corpus used for the aged images comes from files on a Windows installation, many of them copyrighted, so these images are not distributed. Only the scripts and the figures are published.

### A caution about these figures

The public data sets are small: 37 graphics files for CFReDS and 32 files for DFRWS 2006. They were also used while the engine was being developed, so the results on them are not independent of it. The aged disk images were made to check that the engine also behaves on fragmentation it was not tuned on. See [benchmarks.md](benchmarks.md) for the figures.
