//! Finds files by their signature in raw disk data and works out where each one ends.

use std::io::{Read, Seek, SeekFrom};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Jpeg,
    Png,
    Gif,
    Bmp,
    Webp,
    Pdf,
    Zip,
    Mp4,
    Mov,
    M4a,
    Avi,
    Wav,
    Mp3,
    Tiff,
    Ole,
    Pcx,
    Markup,
    Mkv,
    Ts,
    Mpeg,
    Flac,
    Ogg,
    Aac,
    Asf,
    Flv,
    Aiff,
    Caf,
    Raf,
    Crw,
    Psd,
    Ico,
    Font,
    Gzip,
}

impl Kind {
    pub fn max_size(self) -> u64 {
        const MB: u64 = 1 << 20;
        match self {
            Kind::Jpeg | Kind::Png | Kind::Webp => 60 * MB,
            Kind::Gif => 30 * MB,
            Kind::Bmp => 200 * MB,
            Kind::Pdf => 300 * MB,
            Kind::Zip => 1024 * MB,
            Kind::M4a => 80 * MB,
            Kind::Mp3 | Kind::Aac => 200 * MB,
            Kind::Mkv | Kind::Ts | Kind::Mpeg | Kind::Asf | Kind::Flv => 8192 * MB,
            Kind::Flac | Kind::Aiff | Kind::Caf | Kind::Psd => 2048 * MB,
            Kind::Ogg => 1024 * MB,
            Kind::Raf | Kind::Crw => 200 * MB,
            Kind::Mp4 | Kind::Mov | Kind::Avi => 8192 * MB,
            Kind::Wav => 2048 * MB,
            Kind::Tiff => 400 * MB,
            Kind::Ole => 512 * MB,
            Kind::Pcx => 100 * MB,
            Kind::Markup => 64 * MB,
            Kind::Ico => 16 * MB,
            Kind::Font => 64 * MB,
            Kind::Gzip => 2048 * MB,
        }
    }

    /// Whether a header of kind `inner`, found on a sector boundary inside a file of this kind,
    /// is most likely a part of it rather than a file of its own: the entries of a zip, the
    /// frames of a video, the previews of a raw photo, the objects embedded in an Office file.
    /// Anything else found there is a file that was written into a gap of this one.
    pub fn holds(self, inner: Kind) -> bool {
        use Kind::*;
        match self {
            Zip => inner == Zip,
            // videos: their frames, cover art, sound tracks stored as bare MP3 or AAC frames
            Mp4 | Mov | M4a | Avi | Mkv | Asf | Flv => matches!(inner, Mp4 | Mov | M4a | Avi | Mkv | Jpeg | Png | Mp3 | Aac),
            // a DVD's pack headers come every 2048 bytes, a transport stream's packets every 188
            Ts | Mpeg => matches!(inner, Ts | Mpeg | Mp3 | Aac | Jpeg),
            Wav | Aiff | Caf => matches!(inner, Mp3 | Aac),
            Mp3 | Aac => matches!(inner, Mp3 | Aac | Jpeg | Png),
            Flac => matches!(inner, Flac | Jpeg | Png),
            // cover art in an Ogg is written in base64, never as a bare JPEG or PNG
            Ogg => inner == Ogg,
            Tiff | Raf | Crw | Psd => matches!(inner, Tiff | Jpeg),
            Ole => inner == Ole,
            _ => false,
        }
    }
}

/// The files already found that still cover the position being scanned. A header met inside one
/// of them is looked at all the same (a file written into a gap of a fragmented one starts on a
/// sector boundary like any other) unless the outer file is known to hold such headers itself.
#[derive(Default)]
pub struct Open(Vec<(Kind, u64)>);

impl Open {
    pub fn allows(&mut self, at: u64, kind: Kind) -> bool {
        self.0.retain(|&(_, end)| end > at);
        !self.0.iter().any(|&(outer, _)| outer.holds(kind))
    }

    pub fn push(&mut self, kind: Kind, end: u64) {
        self.0.push((kind, end));
    }
}

/// Looks at the first bytes of a sector and says which kind of file starts there.
pub fn detect(b: &[u8]) -> Option<Kind> {
    if b.len() < 16 {
        return None;
    }
    if b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF && matches!(b[3], 0xE0..=0xEF | 0xDB | 0xFE | 0xC4) {
        return Some(Kind::Jpeg);
    }
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(Kind::Png);
    }
    if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        return Some(Kind::Gif);
    }
    if b.starts_with(b"BM") && b[6..10] == [0, 0, 0, 0] {
        let dib = u32::from_le_bytes(b[14..18].try_into().unwrap());
        if matches!(dib, 12 | 40 | 52 | 56 | 108 | 124) {
            return Some(Kind::Bmp);
        }
    }
    if b.starts_with(b"%PDF-1.") || b.starts_with(b"%PDF-2.") {
        return Some(Kind::Pdf);
    }
    if b.starts_with(b"PK\x03\x04") {
        return Some(Kind::Zip);
    }
    if &b[4..8] == b"ftyp" {
        let size = u32::from_be_bytes(b[0..4].try_into().unwrap());
        if (16..=512).contains(&size) {
            return Some(match &b[8..12] {
                b"qt  " => Kind::Mov,
                b"M4A " | b"M4B " => Kind::M4a,
                _ => Kind::Mp4,
            });
        }
    }
    if b.starts_with(b"RIFF") {
        match &b[8..12] {
            b"WAVE" => return Some(Kind::Wav),
            b"AVI " => return Some(Kind::Avi),
            b"WEBP" => return Some(Kind::Webp),
            _ => {}
        }
    }
    if b.starts_with(b"ID3") && b[3] <= 4 && b[6..10].iter().all(|x| x & 0x80 == 0) {
        return Some(Kind::Mp3);
    }
    // TIFF, and the raw photos that only change its magic number: Olympus ORF, Panasonic RW2
    let le_tiff = b.starts_with(b"II*\0") || b.starts_with(b"IIRO") || b.starts_with(b"IIRS") || b.starts_with(b"IIU\0");
    if (le_tiff && le32(&b[4..]) >= 8) || ((b.starts_with(b"MM\0*") || b.starts_with(b"MMOR")) && be32(&b[4..]) >= 8) {
        return Some(Kind::Tiff);
    }
    if b.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Some(Kind::Mkv);
    }
    if b.len() >= 400
        && ((b[0] == 0x47 && b[188] == 0x47 && b[376] == 0x47 && b[1] & 0x80 == 0)
            || (b[4] == 0x47 && b[196] == 0x47 && b[388] == 0x47 && b[5] & 0x80 == 0))
    {
        return Some(Kind::Ts);
    }
    if b.starts_with(&[0, 0, 1, 0xBA]) && (b[4] & 0xC4 == 0x44 || b[4] & 0xF1 == 0x21) {
        return Some(Kind::Mpeg);
    }
    if b.starts_with(b"fLaC") && b[4] & 0x7F == 0 && b[5..8] == [0, 0, 34] {
        return Some(Kind::Flac);
    }
    if b.starts_with(b"OggS\0") && b[5] & 0x02 != 0 && b[5] < 8 {
        return Some(Kind::Ogg);
    }
    if b.starts_with(&ASF_HEADER) {
        return Some(Kind::Asf);
    }
    if b.starts_with(b"FLV\x01") && b[4] & 0xFA == 0 && be32(&b[5..]) == 9 {
        return Some(Kind::Flv);
    }
    if b.starts_with(b"FORM") && (&b[8..12] == b"AIFF" || &b[8..12] == b"AIFC") {
        return Some(Kind::Aiff);
    }
    if b.starts_with(b"caff\0\x01\0\0desc") {
        return Some(Kind::Caf);
    }
    if b.starts_with(b"FUJIFILMCCD-RAW ") {
        return Some(Kind::Raf);
    }
    if b.starts_with(b"II\x1a\0\0\0HEAPCCDR") {
        return Some(Kind::Crw);
    }
    if b.starts_with(b"8BPS") && matches!(b[4..6], [0, 1] | [0, 2]) && b[6..12] == [0; 6] {
        return Some(Kind::Psd);
    }
    // icons and cursors: the first entry of the directory, which starts right after it
    if b[..4] == [0, 0, 1, 0] || b[..4] == [0, 0, 2, 0] {
        let n = le16(&b[4..]);
        if (1..=64).contains(&n) && b[9] == 0 && le32(&b[18..]) == 6 + 16 * n && le32(&b[14..]) >= 40 {
            return Some(Kind::Ico);
        }
    }
    // fonts: the table count and the search fields derived from it
    if b.starts_with(&[0, 1, 0, 0]) || b.starts_with(b"OTTO") || b.starts_with(b"true") {
        let n = u16::from_be_bytes([b[4], b[5]]) as u32;
        let p = if n == 0 { 0 } else { 1u32 << (31 - n.leading_zeros()) };
        let field = |i: usize| u16::from_be_bytes([b[i], b[i + 1]]) as u32;
        if (4..=64).contains(&n) && field(6) == 16 * p && field(8) == p.trailing_zeros() && field(10) == 16 * n - 16 * p {
            return Some(Kind::Font);
        }
    }
    if b.starts_with(&[0x1F, 0x8B, 8]) && b[3] & 0xE0 == 0 && matches!(b[8], 0 | 2 | 4) && (b[9] <= 13 || b[9] == 255) {
        return Some(Kind::Gzip);
    }
    if b.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return Some(Kind::Ole);
    }
    if b.len() >= 128
        && b[0] == 0x0A
        && matches!(b[1], 0 | 2 | 3 | 4 | 5)
        && b[2] == 1
        && matches!(b[3], 1 | 2 | 4 | 8)
        && b[64] == 0
        && (1..=4).contains(&b[65])
        && le16(&b[8..]) >= le16(&b[4..])
        && le16(&b[10..]) >= le16(&b[6..])
    {
        return Some(Kind::Pcx);
    }
    // HTML and XML: text that starts with a declaration, after an optional BOM and blank lines
    let t = b.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(b);
    let t = &t[t.iter().take(64).take_while(|x| x.is_ascii_whitespace()).count()..];
    if t.len() >= 14 {
        let l = t[..14].to_ascii_lowercase();
        if l.starts_with(b"<?xml ") || l.starts_with(b"<!doctype html") || (l.starts_with(b"<html") && b" >\r\n\t".contains(&l[5])) {
            return Some(Kind::Markup);
        }
    }
    // MP3 without a tag, AAC in ADTS frames: just frames, one after the other. The frame after the
    // first must be valid too when it starts within the sector.
    // (`keep`: the bits of the third byte that stay the same from frame to frame)
    for (kind, frame, keep) in [(Kind::Mp3, mp3_frame as fn(&[u8]) -> Option<u64>, 0x0C), (Kind::Aac, adts_frame, 0xFC)] {
        if let Some(len) = frame(b) {
            let next = len as usize;
            let same = |n: &[u8]| frame(n).is_some() && n[1] == b[1] && n[2] & keep == b[2] & keep;
            if next + 8 > b.len() || same(&b[next..]) {
                return Some(kind);
            }
        }
    }
    None
}

const ASF_HEADER: [u8; 16] = [0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C];

/// Any source of bytes that can be read at a position: a volume, an image file...
pub trait Source {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> usize;
}

/// A raw volume: reads must start and end on whole sectors (512 bytes on disks and cards,
/// 2048 on CDs and DVDs), so they are aligned on 4096, which suits both.
pub struct Volume<R: Read + Seek> {
    pub inner: R,
    pub size: u64,
}

impl<R: Read + Seek> Source for Volume<R> {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> usize {
        if offset >= self.size {
            return 0;
        }
        let start = offset & !4095;
        let end = ((offset + buf.len() as u64 + 4095) & !4095).min((self.size + 511) & !511);
        let mut tmp = vec![0u8; (end - start) as usize];
        if self.inner.seek(SeekFrom::Start(start)).is_err() {
            return 0;
        }
        let mut got = 0;
        while got < tmp.len() {
            match self.inner.read(&mut tmp[got..]) {
                Ok(0) | Err(_) => break,
                Ok(n) => got += n,
            }
        }
        let skip = (offset - start) as usize;
        if got <= skip {
            return 0;
        }
        let n = (got - skip).min(buf.len());
        buf[..n].copy_from_slice(&tmp[skip..skip + n]);
        n
    }
}

const BLOCK: usize = 1 << 20;

/// Reads a file that starts at `base`, one block at a time.
struct Cursor<'a> {
    src: &'a mut dyn Source,
    base: u64,
    cache_at: u64,
    cache: Vec<u8>,
    /// Whether a whole block was read already: the first read is short, as most headers that look
    /// right at first are not files at all.
    warm: bool,
}

impl<'a> Cursor<'a> {
    fn new(src: &'a mut dyn Source, base: u64) -> Self {
        Self { src, base, cache_at: 0, cache: Vec::new(), warm: false }
    }

    /// `n` bytes at `pos` (relative to the file start), or None past the end of the disk.
    fn get(&mut self, pos: u64, n: usize) -> Option<&[u8]> {
        let have = pos >= self.cache_at && pos + n as u64 <= self.cache_at + self.cache.len() as u64;
        if !have {
            let want = if self.warm { BLOCK } else { BLOCK / 16 }.max(n);
            self.warm = true;
            let mut buf = vec![0u8; want];
            let got = self.src.read_at(self.base + pos, &mut buf);
            buf.truncate(got);
            self.cache_at = pos;
            self.cache = buf;
            if self.cache.len() < n {
                return None;
            }
        }
        let s = (pos - self.cache_at) as usize;
        Some(&self.cache[s..s + n])
    }

    /// What is left of the current block from `pos`, at least one byte.
    fn chunk(&mut self, pos: u64) -> Option<&[u8]> {
        if !(pos >= self.cache_at && pos < self.cache_at + self.cache.len() as u64) {
            self.get(pos, 1)?;
        }
        let s = (pos - self.cache_at) as usize;
        Some(&self.cache[s..])
    }

    /// Position just after the first `pat` found from `pos`, if within `max`.
    fn find(&mut self, mut pos: u64, pat: &[u8], max: u64) -> Option<u64> {
        // searched in the cached block itself, as `get_window` does but without copying it
        let finder = memchr::memmem::Finder::new(pat);
        while pos < max {
            let enough = pos >= self.cache_at && pos + (pat.len() + 64) as u64 <= self.cache_at + self.cache.len() as u64;
            if !enough {
                // near the end of the cached block: reload from `pos` so the pattern can't be split
                self.cache.clear();
            }
            let c = self.chunk(pos)?;
            if c.len() < pat.len() {
                return None;
            }
            if let Some(i) = finder.find(c) {
                return Some(pos + i as u64 + pat.len() as u64);
            }
            pos += (c.len() - pat.len() + 1) as u64;
        }
        None
    }

    fn get_window(&mut self, pos: u64, overlap: usize) -> Option<Vec<u8>> {
        let c = self.chunk(pos)?.to_vec();
        if c.len() >= overlap + 64 {
            return Some(c);
        }
        // Near the end of the cached block: reload from `pos` so the pattern can't be split.
        self.cache.clear();
        let c = self.chunk(pos)?.to_vec();
        if c.len() < overlap { None } else { Some(c) }
    }
}

fn be32(b: &[u8]) -> u64 {
    u32::from_be_bytes(b[..4].try_into().unwrap()) as u64
}
fn le32(b: &[u8]) -> u64 {
    u32::from_le_bytes(b[..4].try_into().unwrap()) as u64
}
fn le16(b: &[u8]) -> u64 {
    u16::from_le_bytes(b[..2].try_into().unwrap()) as u64
}

/// What the first pass makes of a header.
pub enum Measure {
    /// a file that reads straight through: its length and extension
    /// (sure when its content was checked to the end, not just its declared size trusted:
    /// only then may no other file have a piece inside it)
    Whole(u64, &'static str, bool),
    /// a file whose content goes wrong before its end: most likely cut in pieces, for the
    /// second pass to put back together
    Broken,
    None,
}

/// Like `measure`, but tells a file cut in pieces from one that is not there at all, for the
/// formats whose content can be checked closely enough (baseline JPEG, PNG, ZIP).
pub fn assess(src: &mut dyn Source, offset: u64, kind: Kind) -> Measure {
    if kind == Kind::Zip {
        match crate::pkzip::measure(src, offset) {
            crate::pkzip::Walk::Done(len) => {
                let ext = measure(src, offset, kind).map_or("zip", |m| m.1);
                return Measure::Whole(len, ext, true);
            }
            crate::pkzip::Walk::Broken(..) => return Measure::Broken,
            crate::pkzip::Walk::Unsupported => {}
        }
    }
    if kind == Kind::Ole {
        match crate::ole::measure(src, offset) {
            crate::ole::Check::Good(len, ext) => return Measure::Whole(len, ext, true),
            crate::ole::Check::Bad(_) => return Measure::Broken,
            crate::ole::Check::Unsupported => {}
        }
    }
    if kind == Kind::Png {
        match crate::png::measure(src, offset) {
            Some(crate::png::Walk::Done(len)) => return Measure::Whole(len, "png", true),
            Some(crate::png::Walk::Broken(_)) => return Measure::Broken,
            None => return Measure::None,
        }
    }
    if kind == Kind::Jpeg {
        match crate::jpeg::measure(src, offset, kind.max_size()) {
            crate::jpeg::Outcome::Done(len) => return Measure::Whole(len, "jpg", true),
            crate::jpeg::Outcome::Broken(_) => return Measure::Broken,
            crate::jpeg::Outcome::Unsupported => {}
        }
    }
    if kind == Kind::Gif {
        match crate::gif::measure(src, offset) {
            crate::gif::Check::Good(len) => return Measure::Whole(len, "gif", true),
            crate::gif::Check::Bad => return Measure::Broken,
            crate::gif::Check::Unsupported => {}
        }
    }
    if kind == Kind::Tiff {
        match crate::tif::measure(src, offset) {
            crate::tif::Check::Good(len) => return Measure::Whole(len, "tif", true),
            crate::tif::Check::Bad => return Measure::Broken,
            crate::tif::Check::Unsupported => {}
        }
    }
    if kind == Kind::Pcx {
        match crate::pcx::measure(src, offset) {
            crate::pcx::Check::Good(len) => return Measure::Whole(len, "pcx", true),
            crate::pcx::Check::Bad => return Measure::Broken,
            crate::pcx::Check::Unsupported => {}
        }
    }
    if kind == Kind::Bmp {
        match crate::bmp::measure(src, offset) {
            crate::bmp::Check::Good(len) => return Measure::Whole(len, "bmp", true),
            crate::bmp::Check::Bad => return Measure::Broken,
            crate::bmp::Check::Unsupported => {}
        }
    }
    match measure(src, offset, kind) {
        Some((len, ext)) => Measure::Whole(len, ext, false),
        None => Measure::None,
    }
}

/// The file found at `offset`: its length and its extension, worked out from its content for the
/// formats that cover several kinds of files (zip, Office, MP4, TIFF...).
pub fn measure(src: &mut dyn Source, offset: u64, kind: Kind) -> Option<(u64, &'static str)> {
    let mut c = Cursor::new(src, offset);
    let max = kind.max_size();
    let ext = match kind {
        Kind::Jpeg => "jpg",
        Kind::Png => "png",
        Kind::Gif => "gif",
        Kind::Bmp => "bmp",
        Kind::Webp => "webp",
        Kind::Pdf => "pdf",
        Kind::Avi => "avi",
        Kind::Wav => "wav",
        Kind::Mp3 => "mp3",
        Kind::Pcx => "pcx",
        Kind::Mpeg => "mpg",
        Kind::Flac => "flac",
        Kind::Aac => "aac",
        Kind::Flv => "flv",
        Kind::Aiff => "aif",
        Kind::Caf => "caf",
        Kind::Raf => "raf",
        Kind::Crw => "crw",
        Kind::Psd => "psd",
        Kind::Ico => "ico",
        Kind::Gzip => "gz",
        // worked out from the file itself
        Kind::Zip | Kind::Tiff | Kind::Ole | Kind::Markup | Kind::Font => "",
        Kind::Mp4 | Kind::Mov | Kind::M4a | Kind::Mkv | Kind::Ts | Kind::Ogg | Kind::Asf => "",
    };
    let len = match kind {
        Kind::Jpeg => jpeg(&mut c, max),
        Kind::Png => png(&mut c, max),
        Kind::Gif => gif(&mut c, max),
        Kind::Bmp => {
            let h = c.get(0, 16)?;
            let size = le32(&h[2..]);
            let data = le32(&h[10..]);
            (size > 64 && data < size && size <= max).then_some(size)
        }
        Kind::Pdf => pdf(&mut c, max),
        Kind::Avi | Kind::Wav | Kind::Webp => {
            let h = c.get(0, 12)?;
            let size = le32(&h[4..]) + 8;
            (size > 64 && size <= max).then_some(size + (size & 1))
        }
        Kind::Aiff => {
            let size = be32(&c.get(0, 12)?[4..]) + 8;
            (size > 64 && size <= max).then_some(size + (size & 1))
        }
        Kind::Mp3 => mp3(&mut c, max),
        Kind::Pcx => pcx(&mut c, max),
        Kind::Mpeg => mpeg_ps(&mut c, max),
        Kind::Flac => flac(&mut c, max),
        Kind::Aac => aac(&mut c, max),
        Kind::Flv => flv(&mut c, max),
        Kind::Caf => caf(&mut c, max),
        Kind::Raf => raf(&mut c, max),
        Kind::Crw => crw(&mut c, max),
        Kind::Psd => psd(&mut c, max),
        Kind::Ico => ico(&mut c, max),
        Kind::Font => {
            let otf = c.get(0, 4)? == b"OTTO";
            return font(&mut c, max).filter(|&len| len >= 256).map(|len| (len, if otf { "otf" } else { "ttf" }));
        }
        Kind::Gzip => gzip(&mut c, max),
        Kind::Zip => return zip(&mut c, max),
        Kind::Tiff => return tiff(&mut c, max),
        Kind::Ole => return ole(&mut c, max),
        Kind::Markup => return markup(&mut c, max).filter(|&(len, _)| len >= 64),
        Kind::Mp4 | Kind::Mov | Kind::M4a => return mp4(&mut c, max).filter(|&(len, _)| len >= 256),
        Kind::Mkv => return mkv(&mut c, max).filter(|&(len, _)| len >= 256),
        Kind::Ts => return ts(&mut c, max),
        Kind::Ogg => return ogg(&mut c, max).filter(|&(len, _)| len >= 256),
        Kind::Asf => return asf(&mut c, max).filter(|&(len, _)| len >= 256),
    }?;
    (len >= 256).then_some((len, ext))
}

fn jpeg(c: &mut Cursor, max: u64) -> Option<u64> {
    let mut pos = 2u64;
    let mut seen_sos = false;
    loop {
        if pos > max {
            return None;
        }
        let h = c.get(pos, 4)?;
        if h[0] != 0xFF {
            return None;
        }
        let m = h[1];
        match m {
            0xFF => pos += 1,
            0xD8 | 0x01 | 0xD0..=0xD7 => pos += 2,
            0xD9 => return seen_sos.then_some(pos + 2),
            _ => {
                let len = u16::from_be_bytes([h[2], h[3]]) as u64;
                if len < 2 {
                    return None;
                }
                pos += 2 + len;
                if m == 0xDA {
                    seen_sos = true;
                    // Entropy-coded data: runs until the next real marker.
                    'scan: loop {
                        if pos > max {
                            return None;
                        }
                        let w = c.get_window(pos, 2)?;
                        for i in 0..w.len() - 1 {
                            if w[i] == 0xFF {
                                let n = w[i + 1];
                                if n != 0x00 && n != 0xFF && !(0xD0..=0xD7).contains(&n) {
                                    pos += i as u64;
                                    break 'scan;
                                }
                            }
                        }
                        pos += (w.len() - 1) as u64;
                    }
                }
            }
        }
    }
}

/// A PDF ends at its last `%%EOF`, which is not always the first one: a linearized file has one
/// after its first page, and each incremental update appends a body, a table and another `%%EOF`.
/// A linearized file states its own length (`/L`), used when a `%%EOF` really ends there.
/// Otherwise the file goes on past a `%%EOF` as long as an object or a table follows it.
fn pdf(c: &mut Cursor, max: u64) -> Option<u64> {
    let eol = |c: &mut Cursor, at: u64| {
        let t = c.get(at, 2).map(|t| t.to_vec()).unwrap_or_default();
        t.iter().take_while(|&&x| x == b'\r' || x == b'\n').count() as u64
    };
    let head = match c.get(0, 1024) {
        Some(h) => h.to_vec(),
        None => c.get(0, 64)?.to_vec(),
    };
    if let Some(l) = linearized_len(&head)
        && l > 64
        && l <= max
        && let Some(t) = c.get(l - 16, 16)
    {
        let t = t.to_vec();
        let body = t.iter().rposition(|&x| x != b'\r' && x != b'\n' && x != b' ');
        if body.is_some_and(|i| i >= 4 && &t[i - 4..=i] == b"%%EOF") {
            return Some(l);
        }
    }
    let mut end = match pdf_eof(c, 8, max)? {
        Ok(e) => e + eol(c, e),
        // cut in pieces: what comes before the next file is all that is surely its own
        Err(next) => return Some(next),
    };
    loop {
        let Some(next) = c.get(end, 32).map(|t| t.to_vec()) else { break };
        let s = next.iter().position(|x| !x.is_ascii_whitespace()).unwrap_or(next.len());
        if !pdf_goes_on(&next[s..]) {
            break;
        }
        match pdf_eof(c, end + s as u64, max) {
            Some(Ok(e)) => end = e + eol(c, e),
            // it goes on but is cut in pieces (a linearized PDF past its first page, say): all
            // up to the next file is its own
            Some(Err(next)) => return Some(next),
            None => break,
        }
    }
    Some(end)
}

/// The position after the next `%%EOF` from `from`, or, as an error, where another PDF starts on
/// a sector boundary before it. A PDF is not stored that way inside another one: the file was
/// cut in pieces, and reading on to a later `%%EOF` would take in other files whole, again and
/// again for each piece of a PDF found before them.
fn pdf_eof(c: &mut Cursor, from: u64, max: u64) -> Option<Result<u64, u64>> {
    const SEG: u64 = 4 << 20;
    let mut pos = from;
    while pos < max {
        let seg = (pos + SEG).min(max);
        let eof = c.find(pos, b"%%EOF", seg);
        let upto = eof.unwrap_or(seg).min(max);
        let mut s = pos.max(1).next_multiple_of(512);
        while s + 5 <= upto {
            if c.get(s, 5)? == b"%PDF-" {
                return Some(Err(s));
            }
            s += 512;
        }
        if let Some(e) = eof {
            return Some(Ok(e));
        }
        if seg >= max {
            break;
        }
        // a few bytes again, for a `%%EOF` across the two
        pos = seg - 4;
    }
    None
}

/// The `/L` length of a linearization dictionary at the start of a PDF.
fn linearized_len(head: &[u8]) -> Option<u64> {
    let i = head.windows(11).position(|w| w == b"/Linearized")?;
    let rest = &head[i..];
    // `/L` and not `/Linearized`
    let k = (0..rest.len().saturating_sub(2)).find(|&k| &rest[k..k + 2] == b"/L" && !rest[k + 2].is_ascii_alphabetic())?;
    let digits: Vec<u8> =
        rest[k + 2..].iter().skip_while(|x| x.is_ascii_whitespace()).take_while(|x| x.is_ascii_digit()).copied().collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

/// Whether bytes after a `%%EOF` carry on the same PDF: an object ("12 0 obj") or a table.
fn pdf_goes_on(b: &[u8]) -> bool {
    if b.starts_with(b"xref") {
        return true;
    }
    let num = |b: &[u8]| b.iter().take_while(|x| x.is_ascii_digit()).count();
    let a = num(b);
    if a == 0 || b.get(a) != Some(&b' ') {
        return false;
    }
    let g = num(&b[a + 1..]);
    g > 0 && b[a + 1 + g..].starts_with(b" obj")
}

fn png(c: &mut Cursor, max: u64) -> Option<u64> {
    let mut pos = 8u64;
    loop {
        let h = c.get(pos, 8)?;
        let len = be32(h);
        let typ = [h[4], h[5], h[6], h[7]];
        if !typ.iter().all(|b| b.is_ascii_alphabetic()) || len > max {
            return None;
        }
        pos += 12 + len;
        if &typ == b"IEND" {
            return Some(pos);
        }
        if pos > max {
            return None;
        }
    }
}

fn gif(c: &mut Cursor, max: u64) -> Option<u64> {
    let h = c.get(0, 13)?;
    let flags = h[10];
    let mut pos = 13u64;
    if flags & 0x80 != 0 {
        pos += 3 * (1 << ((flags & 7) + 1));
    }
    fn sub_blocks(c: &mut Cursor, mut pos: u64, max: u64) -> Option<u64> {
        loop {
            let n = c.get(pos, 1)?[0] as u64;
            pos += 1 + n;
            if n == 0 {
                return Some(pos);
            }
            if pos > max {
                return None;
            }
        }
    }
    loop {
        if pos > max {
            return None;
        }
        match c.get(pos, 1)?[0] {
            0x3B => return Some(pos + 1),
            0x21 => pos = sub_blocks(c, pos + 2, max)?,
            0x2C => {
                let d = c.get(pos, 10)?;
                let f = d[9];
                pos += 10;
                if f & 0x80 != 0 {
                    pos += 3 * (1 << ((f & 7) + 1));
                }
                pos = sub_blocks(c, pos + 1, max)?;
            }
            _ => return None,
        }
    }
}

fn zip(c: &mut Cursor, max: u64) -> Option<(u64, &'static str)> {
    let h = c.get(0, 30)?;
    let name_len = le16(&h[26..]) as usize;
    let extra_len = le16(&h[28..]);
    let first = c.get(30, name_len.min(64))?.to_vec();
    let mime = c.get(30 + name_len as u64 + extra_len, 48).map(|m| m.to_vec()).unwrap_or_default();
    let end_sig = c.find(4, b"PK\x05\x06", max)? - 4;
    let e = c.get(end_sig, 22)?.to_vec();
    let cd_size = le32(&e[12..]);
    let cd_start = le32(&e[16..]);
    // The central directory ends where the end record starts (or, in a ZIP64 file, where its
    // ZIP64 records start). When the end record is further away than it says, either the file is
    // in pieces, or this end record belongs to another archive (this one's own end is gone, and
    // reading on would glue unrelated data to it). It is this file's end only if the directory
    // just before it lists this file's first entry, by its name, at offset 0.
    let zip64 =
        cd_start == 0xFFFF_FFFF || cd_size == 0xFFFF_FFFF || (end_sig >= 20 && c.get(end_sig - 20, 4).is_some_and(|b| b == b"PK\x06\x07"));
    if !zip64 && cd_start + cd_size != end_sig {
        let ours = cd_size <= 8 << 20
            && cd_size <= end_sig
            && c.get(end_sig - cd_size, cd_size as usize).is_some_and(|cd| {
                cd.windows(4).enumerate().any(|(i, w)| {
                    w == b"PK\x01\x02"
                        && cd.len() >= i + 46 + first.len()
                        && le32(&cd[i + 42..]) == 0
                        && le16(&cd[i + 28..]) as usize == name_len
                        && cd[i + 46..i + 46 + first.len()] == first[..]
                })
            });
        if !ours {
            return None;
        }
    }
    let len = end_sig + 22 + le16(&e[20..]);
    let mut ext = "zip";
    if first.starts_with(b"mimetype") {
        if mime.windows(4).any(|w| w == b"text") {
            ext = "odt";
        } else if mime.windows(11).any(|w| w == b"spreadsheet") {
            ext = "ods";
        } else if mime.windows(12).any(|w| w == b"presentation") {
            ext = "odp";
        }
    } else if cd_start < end_sig
        && cd_size <= 8 << 20
        && let Some(cd) = c.get(cd_start, cd_size as usize)
    {
        let has = |p: &[u8]| cd.windows(p.len()).any(|w| w == p);
        if has(b"word/document") {
            ext = "docx";
        } else if has(b"xl/workbook") {
            ext = "xlsx";
        } else if has(b"ppt/presentation") {
            ext = "pptx";
        }
    }
    Some((len, ext))
}

/// The ISO base media files (ISO/IEC 14496-12): MP4, QuickTime, M4A, 3GP, HEIF photos (HEIC and
/// AVIF), Canon CR3. A chain of boxes, each one with its size; the brands of the first one, `ftyp`,
/// say which kind of file it is.
fn mp4(c: &mut Cursor, max: u64) -> Option<(u64, &'static str)> {
    const KNOWN: [&[u8; 4]; 12] =
        [b"ftyp", b"moov", b"mdat", b"free", b"skip", b"wide", b"uuid", b"meta", b"pnot", b"udta", b"moof", b"mfra"];
    let n = (be32(c.get(0, 16)?) as usize).clamp(16, 512);
    let ftyp = c.get(0, n)?.to_vec();
    let major: &[u8] = &ftyp[8..12];
    let compatible: Vec<&[u8]> = ftyp[16..].as_chunks::<4>().0.iter().map(|b| &b[..]).collect();
    let has = |b: &[u8]| major == b || compatible.contains(&b);
    let ext = match major {
        b"qt  " => "mov",
        b"M4A " | b"M4B " | b"M4P " => "m4a",
        b"M4V " | b"M4VH" | b"M4VP" => "m4v",
        b"crx " => "cr3",
        b"avif" | b"avis" => "avif",
        b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevx" => "heic",
        b"mif1" | b"msf1" if has(b"avif") => "avif",
        b"mif1" | b"msf1" => "heic",
        m if m.starts_with(b"3g2") => "3g2",
        m if m.starts_with(b"3gp") || m.starts_with(b"3ge") || m.starts_with(b"3gg") => "3gp",
        _ => "mp4",
    };
    // a HEIF photo has no movie box: its items are described in `meta`
    let photo = ext == "heic" || ext == "avif";
    let mut pos = 0u64;
    let (mut moov, mut mdat, mut meta) = (false, false, false);
    loop {
        let Some(h) = c.get(pos, 16) else { break };
        let mut size = be32(h);
        let typ: [u8; 4] = h[4..8].try_into().unwrap();
        if size == 1 {
            size = u64::from_be_bytes(h[8..16].try_into().unwrap());
        }
        if !KNOWN.contains(&&typ) || size < 8 {
            break;
        }
        moov |= &typ == b"moov";
        mdat |= &typ == b"mdat";
        meta |= &typ == b"meta";
        pos += size;
        if pos > max {
            return None;
        }
    }
    (moov && mdat || photo && meta).then_some((pos, ext))
}

/// The length of the MPEG audio layer III frame whose header starts `f`, if it is one.
fn mp3_frame(f: &[u8]) -> Option<u64> {
    const BR1: [u64; 15] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
    const BR2: [u64; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
    if f.len() < 4 || f[0] != 0xFF || f[1] & 0xE0 != 0xE0 {
        return None;
    }
    let version = (f[1] >> 3) & 3; // 3 = MPEG1, 2 = MPEG2, 0 = MPEG2.5
    let layer = (f[1] >> 1) & 3; // 1 = layer III
    let bri = (f[2] >> 4) as usize;
    let sri = ((f[2] >> 2) & 3) as usize;
    if version == 1 || layer != 1 || bri == 0 || bri == 15 || sri == 3 || f[3] & 3 == 2 {
        return None;
    }
    let pad = ((f[2] >> 1) & 1) as u64;
    let base = [44100u64, 48000, 32000][sri];
    let (sr, br, k) = match version {
        3 => (base, BR1[bri], 144),
        2 => (base / 2, BR2[bri], 72),
        _ => (base / 4, BR2[bri], 72),
    };
    Some(k * br * 1000 / sr + pad)
}

/// The length of the AAC frame in ADTS form (ISO/IEC 13818-7) whose header starts `h`, if it is one.
fn adts_frame(h: &[u8]) -> Option<u64> {
    if h.len() < 7 || h[0] != 0xFF || h[1] & 0xF6 != 0xF0 || (h[2] >> 2) & 15 > 12 {
        return None;
    }
    let len = ((h[3] & 3) as u64) << 11 | (h[4] as u64) << 3 | (h[5] >> 5) as u64;
    (len >= 7).then_some(len)
}

/// Frames one after the other, from `pos`, as long as they keep the same format; returns where
/// they end and how many there are.
fn frames(c: &mut Cursor, mut pos: u64, max: u64, frame: fn(&[u8]) -> Option<u64>, keep: u8) -> Option<(u64, u32)> {
    let mut n = 0;
    let mut first: Option<[u8; 2]> = None;
    loop {
        if pos > max {
            return None;
        }
        let Some(f) = c.get(pos, 8) else { break };
        let Some(len) = frame(f) else { break };
        let id = [f[1], f[2] & keep];
        if *first.get_or_insert(id) != id {
            break;
        }
        pos += len;
        n += 1;
    }
    Some((pos, n))
}

fn mp3(c: &mut Cursor, max: u64) -> Option<u64> {
    let h = c.get(0, 10)?;
    let (start, least) = if h.starts_with(b"ID3") {
        let tag = h[6..10].iter().fold(0u64, |a, &b| (a << 7) | b as u64);
        (10 + tag + if h[5] & 0x10 != 0 { 10 } else { 0 }, 10)
    } else {
        // without a tag, only the frames say it is a song
        (0, 32)
    };
    let (mut pos, n) = frames(c, start, max, mp3_frame, 0x0C)?;
    if c.get(pos, 3).is_some_and(|t| t == b"TAG") {
        pos += 128;
    }
    (n >= least).then_some(pos)
}

fn aac(c: &mut Cursor, max: u64) -> Option<u64> {
    let (pos, n) = frames(c, 0, max, adts_frame, 0xFC)?;
    (n >= 32).then_some(pos)
}

/// An EBML variable-length integer at `pos`: its value (with its length marker for an element ID,
/// without it for a size, all ones meaning "unknown") and its length.
fn vint(c: &mut Cursor, pos: u64, id: bool) -> Option<(u64, u64)> {
    let b = c.get(pos, 8)?;
    let len = b[0].leading_zeros() as usize + 1;
    if len > 8 || (id && len > 4) {
        return None;
    }
    let first = if id { b[0] as u64 } else { (b[0] as u64) & (0xFF >> len) };
    let v = b[1..len].iter().fold(first, |a, &x| (a << 8) | x as u64);
    Some((v, len as u64))
}

/// An EBML element at `pos`: its ID, its size (None when unknown) and the length of its header.
fn element(c: &mut Cursor, pos: u64) -> Option<(u64, Option<u64>, u64)> {
    let (id, a) = vint(c, pos, true)?;
    let (size, b) = vint(c, pos + a, false)?;
    let unknown = size == (1u64 << (7 * b)) - 1;
    Some((id, (!unknown).then_some(size), a + b))
}

/// Matroska and WebM: an EBML header that names the kind of document, then one Segment element
/// that holds everything. Recorders that write as they go (a browser, a screen capture) leave
/// its size unknown: the file then ends after the last element that can belong to a segment.
fn mkv(c: &mut Cursor, max: u64) -> Option<(u64, &'static str)> {
    // the elements a segment holds, and those a cluster holds
    const SEGMENT: [u64; 10] = [0x114D9B74, 0x1549A966, 0x1654AE6B, 0x1F43B675, 0x1C53BB6B, 0x1941A469, 0x1043A770, 0x1254C367, 0xEC, 0xBF];
    const CLUSTER: [u64; 8] = [0xE7, 0xA3, 0xA0, 0xA7, 0xAB, 0x5854, 0xEC, 0xBF];
    fn walk(c: &mut Cursor, mut pos: u64, max: u64, ids: &[u64]) -> u64 {
        while pos <= max {
            let Some((id, size, head)) = element(c, pos) else { break };
            if !ids.contains(&id) {
                break;
            }
            pos = match size {
                Some(n) => pos + head + n,
                None if id == 0x1F43B675 => walk(c, pos + head, max, &CLUSTER),
                None => break,
            };
        }
        pos
    }
    let (id, size, head) = element(c, 0)?;
    let size = size.filter(|&s| id == 0x1A45DFA3 && s <= 1024)?;
    let ebml = c.get(head, size as usize)?;
    let ext = if ebml.windows(7).any(|w| w == b"\x42\x82\x84webm") {
        "webm"
    } else if ebml.windows(11).any(|w| w == b"\x42\x82\x88matroska") {
        "mkv"
    } else {
        return None;
    };
    let seg = head + size;
    let (id, size, head) = element(c, seg)?;
    if id != 0x18538067 {
        return None;
    }
    let end = match size {
        Some(n) => seg + head + n,
        None => walk(c, seg + head, max, &SEGMENT),
    };
    (end <= max).then_some((end, ext))
}

/// MPEG transport streams: packets of 188 bytes that start with 0x47 (192 bytes, with a time
/// stamp first, on AVCHD camcorders and Blu-ray discs).
fn ts(c: &mut Cursor, max: u64) -> Option<(u64, &'static str)> {
    let (step, at, ext) = if c.get(0, 1)?[0] == 0x47 { (188, 0, "m2t") } else { (192, 4, "mts") };
    let mut pos = 0u64;
    let mut n = 0;
    while c.get(pos + at, 2).is_some_and(|p| p[0] == 0x47 && p[1] & 0x80 == 0) {
        pos += step;
        n += 1;
        if pos > max {
            return None;
        }
    }
    (n >= 16).then_some((pos, ext))
}

/// MPEG program streams (MPEG-1 and MPEG-2 video files, DVD VOB files): pack headers and the
/// packets between them, up to the end code.
fn mpeg_ps(c: &mut Cursor, max: u64) -> Option<u64> {
    let mut pos = 0u64;
    let mut packs = 0;
    while pos <= max {
        let Some(h) = c.get(pos, 6).map(|h| [h[0], h[1], h[2], h[3], h[4], h[5]]) else { break };
        if h[..3] != [0, 0, 1] {
            break;
        }
        match h[3] {
            0xBA if h[4] & 0xC0 == 0x40 => match c.get(pos, 14) {
                Some(p) => pos += 14 + (p[13] & 7) as u64,
                None => break,
            },
            0xBA if h[4] & 0xF0 == 0x20 => pos += 12,
            0xBA => break,
            0xB9 => return (packs >= 4).then_some(pos + 4),
            0xBB.. => pos += 6 + ((h[4] as u64) << 8 | h[5] as u64),
            _ => break,
        }
        packs += (h[3] == 0xBA) as u32;
    }
    (pos <= max && packs >= 4).then_some(pos)
}

const fn crc16_table() -> [u16; 256] {
    let mut t = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = (i as u16) << 8;
        let mut k = 0;
        while k < 8 {
            c = if c & 0x8000 != 0 { (c << 1) ^ 0x8005 } else { c << 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}
const CRC16: [u16; 256] = crc16_table();

fn crc8(data: &[u8]) -> u8 {
    let mut c = 0u8;
    for &b in data {
        c ^= b;
        for _ in 0..8 {
            c = if c & 0x80 != 0 { (c << 1) ^ 0x07 } else { c << 1 };
        }
    }
    c
}

/// The number of samples in the FLAC frame whose header starts `h`, if it is one (its checksum
/// must match).
fn flac_frame(h: &[u8]) -> Option<u64> {
    if h.len() < 16 || h[0] != 0xFF || h[1] & 0xFE != 0xF8 {
        return None;
    }
    let (bs, sr, ch, depth) = (h[2] >> 4, h[2] & 15, h[3] >> 4, (h[3] >> 1) & 7);
    if bs == 0 || sr == 15 || ch > 10 || depth == 3 || depth == 7 || h[3] & 1 != 0 {
        return None;
    }
    // the frame or sample number, coded like UTF-8
    let n = match h[4].leading_ones() as usize {
        0 => 1,
        k @ 2..=7 => k,
        _ => return None,
    };
    if h[5..4 + n].iter().any(|&x| x & 0xC0 != 0x80) {
        return None;
    }
    let mut p = 4 + n;
    let samples = match bs {
        1 => 192,
        2..=5 => 576u64 << (bs - 2),
        6 => {
            p += 1;
            h[p - 1] as u64 + 1
        }
        7 => {
            p += 2;
            ((h[p - 2] as u64) << 8 | h[p - 1] as u64) + 1
        }
        _ => 256u64 << (bs - 8),
    };
    p += match sr {
        12 => 1,
        13 | 14 => 2,
        _ => 0,
    };
    (crc8(&h[..p]) == h[p]).then_some(samples)
}

/// FLAC: metadata blocks, then frames. Frames carry no length: each one ends with a checksum of
/// the whole frame, so its end is where that checksum matches, just before the next frame (or,
/// for the last one, once all the samples announced in STREAMINFO are there).
fn flac(c: &mut Cursor, max: u64) -> Option<u64> {
    let mut pos = 4u64;
    let (mut total, mut biggest) = (0u64, 0u64);
    loop {
        let h = c.get(pos, 4)?;
        let (last, typ, len) = (h[0] & 0x80 != 0, h[0] & 0x7F, be32(h) & 0xFF_FFFF);
        if typ == 127 {
            return None;
        }
        if typ == 0 {
            let s = c.get(pos + 4, 18)?;
            biggest = (s[7] as u64) << 16 | (s[8] as u64) << 8 | s[9] as u64;
            total = ((s[13] & 15) as u64) << 32 | be32(&s[14..]);
        }
        pos += 4 + len;
        if pos > max {
            return None;
        }
        if last {
            break;
        }
    }
    let cap = if biggest > 0 { biggest + 2 } else { 1 << 20 };
    let (mut done, mut n) = (0u64, 0u32);
    loop {
        let Some(samples) = c.get(pos, 16).and_then(flac_frame) else { break };
        // the places where the checksum of everything since the frame start comes back to zero
        let mut ends = Vec::new();
        let (mut crc, mut at) = (0u16, pos);
        while at - pos < cap {
            let Some(s) = c.chunk(at) else { break };
            let s = s[..s.len().min((cap - (at - pos)) as usize)].to_vec();
            for (i, &b) in s.iter().enumerate() {
                crc = (crc << 8) ^ CRC16[((crc >> 8) as u8 ^ b) as usize];
                let end = at + i as u64 + 1;
                if crc == 0 && end - pos >= 16 {
                    ends.push(end);
                }
            }
            at += s.len() as u64;
        }
        let last = total > 0 && done + samples >= total;
        let next = if last { None } else { ends.iter().copied().find(|&e| c.get(e, 16).and_then(flac_frame).is_some()) };
        let end = match next {
            Some(e) => e,
            None if last || total == 0 => match ends.first() {
                Some(&e) => e,
                None => break,
            },
            None => break,
        };
        done += samples;
        n += 1;
        pos = end;
        if pos > max {
            return None;
        }
        if next.is_none() {
            break;
        }
    }
    (n > 0).then_some(pos)
}

/// Ogg (Vorbis, Opus, Theora...): pages that start with "OggS". Each logical stream opens with a
/// page flagged "beginning of stream" and closes with one flagged "end of stream".
fn ogg(c: &mut Cursor, max: u64) -> Option<(u64, &'static str)> {
    let mut pos = 0u64;
    let mut open: Vec<u64> = Vec::new();
    let (mut pages, mut ext) = (0u32, "ogg");
    while pos <= max {
        let Some(h) = c.get(pos, 27) else { break };
        if &h[..5] != b"OggS\0" {
            break;
        }
        let (flags, serial, segments) = (h[5], le32(&h[14..]), h[26] as u64);
        let Some(table) = c.get(pos + 27, segments as usize) else { break };
        let body: u64 = table.iter().map(|&x| x as u64).sum();
        if flags & 2 != 0 {
            if pages > 0 && open.is_empty() {
                break; // the next file
            }
            open.push(serial);
            if pages == 0 {
                let p = c.get(pos + 27 + segments, 8).map(|p| p.to_vec()).unwrap_or_default();
                ext = if p.starts_with(b"OpusHead") {
                    "opus"
                } else if p.starts_with(b"\x80theora") {
                    "ogv"
                } else {
                    "ogg"
                };
            }
        } else if !open.contains(&serial) {
            break;
        }
        pos += 27 + segments + body;
        pages += 1;
        if flags & 4 != 0 {
            open.retain(|&s| s != serial);
            if open.is_empty() {
                break;
            }
        }
    }
    (pos <= max && pages >= 2).then_some((pos, ext))
}

fn le64(b: &[u8]) -> u64 {
    u64::from_le_bytes(b[..8].try_into().unwrap())
}

/// Windows Media (ASF: wmv, wma). A header object, then a data object and indexes; the header's
/// file properties give the size of the whole file, except while it is being recorded.
fn asf(c: &mut Cursor, max: u64) -> Option<(u64, &'static str)> {
    const DATA: [u8; 16] = [0x36, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C];
    const FILE_PROPERTIES: [u8; 16] = [0xA1, 0xDC, 0xAB, 0x8C, 0x47, 0xA9, 0xCF, 0x11, 0x8E, 0xE4, 0x00, 0xC0, 0x0C, 0x20, 0x53, 0x65];
    const VIDEO: [u8; 16] = [0xC0, 0xEF, 0x19, 0xBC, 0x4D, 0x5B, 0xCF, 0x11, 0xA8, 0xFD, 0x00, 0x80, 0x5F, 0x5C, 0x44, 0x2B];
    const AUDIO: [u8; 16] = [0x40, 0x9E, 0x69, 0xF8, 0x4D, 0x5B, 0xCF, 0x11, 0xA8, 0xFD, 0x00, 0x80, 0x5F, 0x5C, 0x44, 0x2B];
    const INDEXES: [[u8; 16]; 2] = [
        [0x90, 0x08, 0x00, 0x33, 0xB1, 0xE5, 0xCF, 0x11, 0x89, 0xF4, 0x00, 0xA0, 0xC9, 0x03, 0x49, 0xCB],
        [0xD3, 0x29, 0xE2, 0xD6, 0xDA, 0x35, 0xD1, 0x11, 0x90, 0x34, 0x00, 0xA0, 0xC9, 0x03, 0x49, 0xBE],
    ];
    let size = le64(&c.get(0, 30)?[16..]);
    if !(30..=1 << 20).contains(&size) {
        return None;
    }
    let head = c.get(0, size as usize)?.to_vec();
    let at = |g: &[u8; 16]| head.windows(16).position(|w| w == g);
    let ext = if at(&VIDEO).is_some() {
        "wmv"
    } else if at(&AUDIO).is_some() {
        "wma"
    } else {
        "asf"
    };
    if let Some(i) = at(&FILE_PROPERTIES).filter(|&i| i + 92 <= head.len()) {
        let (len, broadcast) = (le64(&head[i + 40..]), le32(&head[i + 88..]) & 1 != 0);
        if !broadcast && len > size && len <= max {
            return Some((len, ext));
        }
    }
    let mut pos = size;
    while let Some(o) = c.get(pos, 24) {
        let len = le64(&o[16..]);
        if !(o[..16] == DATA || INDEXES.iter().any(|g| o[..16] == *g)) || len < 24 {
            break;
        }
        pos += len;
        if pos > max {
            return None;
        }
    }
    (pos > size).then_some((pos, ext))
}

/// Flash video: a header, then tags, each one followed by its own size.
fn flv(c: &mut Cursor, max: u64) -> Option<u64> {
    let mut pos = 13u64;
    let mut tags = 0;
    while let Some(t) = c.get(pos, 11) {
        let len = (t[1] as u64) << 16 | (t[2] as u64) << 8 | t[3] as u64;
        if !matches!(t[0] & 0x1F, 8 | 9 | 18) || t[8..11] != [0, 0, 0] {
            break;
        }
        if !c.get(pos + 11 + len, 4).is_some_and(|p| be32(p) == 11 + len) {
            break;
        }
        pos += 15 + len;
        tags += 1;
        if pos > max {
            return None;
        }
    }
    (tags >= 2).then_some(pos)
}

/// Apple Core Audio files: chunks with a 64-bit size, the audio data among them.
fn caf(c: &mut Cursor, max: u64) -> Option<u64> {
    let mut pos = 8u64;
    let mut data = false;
    while let Some(h) = c.get(pos, 12) {
        if !h[..4].iter().all(|x| x.is_ascii_alphanumeric() || *x == b' ') {
            break;
        }
        let size = i64::from_be_bytes(h[4..12].try_into().unwrap());
        if size < 0 {
            // the audio data of a recording that was never closed: its end is unknown
            return None;
        }
        data |= &h[..4] == b"data";
        pos += 12 + size as u64;
        if pos > max {
            return None;
        }
    }
    data.then_some(pos)
}

/// Fujifilm RAF: a header that gives the place and length of the JPEG preview, of the sensor
/// header and of the sensor data.
fn raf(c: &mut Cursor, max: u64) -> Option<u64> {
    let h = c.get(0, 0x6C)?.to_vec();
    let mut end = 0x6C;
    for i in [0x54, 0x5C, 0x64] {
        let (at, len) = (be32(&h[i..]), be32(&h[i + 4..]));
        if len == 0 {
            continue;
        }
        if at < 0x6C || at + len > max {
            return None;
        }
        end = end.max(at + len);
    }
    (end > 0x6C).then_some(end)
}

/// Canon CRW (CIFF): a header, then a heap whose table of contents is at its very end, its offset
/// in the last four bytes. The scan looks for such a table, counted from the start of the heap;
/// the table of the whole heap points at sub-heaps that are built the same way (the tables of the
/// sub-heaps count from the start of each one, so they don't match on the way).
fn crw(c: &mut Cursor, max: u64) -> Option<u64> {
    /// Whether `n` entries of 10 bytes, after their count, can end at `q` when the table is at `v`.
    fn shape(v: u64, q: u64) -> Option<usize> {
        let n = q.checked_sub(v + 2)?;
        (n > 0 && n <= 10 * 256 && n % 10 == 0).then_some((n / 10) as usize)
    }
    /// The entries of the table of the heap that starts at `start` and ends at `start + len`.
    fn table(c: &mut Cursor, start: u64, len: u64) -> Option<(u64, Vec<u8>)> {
        let q = len.checked_sub(4)?;
        let v = le32(c.get(start + q, 4)?);
        let n = shape(v, q)?;
        let t = c.get(start + v, 2 + 10 * n)?.to_vec();
        (le16(&t) as usize == n).then_some((v, t))
    }
    let heap = le32(&c.get(0, 6)?[2..]);
    let mut len = 16u64;
    while heap + len <= max {
        if let Some((v, t)) = table(c, heap, len) {
            let mut fits = true;
            let mut subs = Vec::new();
            for e in t[2..].chunks(10) {
                let (typ, size, at) = (le16(e), le32(&e[2..]), le32(&e[6..]));
                if typ & 0xC000 != 0 {
                    continue; // its value is in the entry itself
                }
                fits &= at + size <= v;
                if matches!(typ & 0x3800, 0x2800 | 0x3000) {
                    subs.push((at, size));
                }
            }
            if fits && !subs.is_empty() && subs.iter().all(|&(at, size)| table(c, heap + at, size).is_some()) {
                return Some(heap + len);
            }
        }
        len += 1;
    }
    None
}

/// Photoshop (PSD, and PSB for big documents): a header, three sections with their lengths, then
/// the merged image, raw or compressed line by line with the length of each line listed first.
/// An icon (or a cursor): a directory of images, each with its offset and size. The file ends
/// with the last image. Each image must start with a PNG signature or a bitmap header.
fn ico(c: &mut Cursor, max: u64) -> Option<u64> {
    let n = le16(c.get(4, 2)?);
    let dir = c.get(6, 16 * n as usize)?.to_vec();
    let first = 6 + 16 * n;
    let mut end = 0;
    let mut at = Vec::new();
    for e in dir.chunks(16) {
        let (size, off) = (le32(&e[8..]), le32(&e[12..]));
        if e[3] != 0 || size < 40 || off < first || off + size > max {
            return None;
        }
        at.push(off);
        end = end.max(off + size);
    }
    for off in at {
        let h = c.get(off, 8)?;
        if !(h.starts_with(b"\x89PNG\r\n\x1a\n") || le32(h) == 40) {
            return None;
        }
    }
    Some(end)
}

/// A TrueType or OpenType font: a directory of tables, each with its offset and length,
/// padded to four bytes. The file ends with the last table.
fn font(c: &mut Cursor, max: u64) -> Option<u64> {
    let n = c.get(4, 2).map(|b| u16::from_be_bytes([b[0], b[1]]) as u64)?;
    let dir = c.get(12, 16 * n as usize)?.to_vec();
    let first = 12 + 16 * n;
    let mut end = 0;
    let mut prev = [0u8; 4];
    for e in dir.chunks(16) {
        let tag: [u8; 4] = e[..4].try_into().unwrap();
        let (off, len) = (be32(&e[8..]), be32(&e[12..]));
        // tags are printable and sorted
        if !tag.iter().all(|&x| (0x20..0x7F).contains(&x)) || tag <= prev || off < first || off + len > max {
            return None;
        }
        prev = tag;
        end = end.max(off + len);
    }
    Some(end.div_ceil(4) * 4)
}

/// A gzip file: a header, a deflate stream, then the CRC and length of what it inflates to.
/// Inflating it is the only way to find its end, and the trailer checks the result.
fn gzip(c: &mut Cursor, max: u64) -> Option<u64> {
    use miniz_oxide::inflate::stream::{InflateState, inflate};
    use miniz_oxide::{DataFormat, MZError, MZFlush, MZStatus};
    let h = c.get(0, 10)?.to_vec();
    let flags = h[3];
    let mut pos = 10u64;
    if flags & 4 != 0 {
        pos += 2 + le16(c.get(pos, 2)?);
    }
    for bit in [8u8, 16] {
        if flags & bit != 0 {
            pos = c.find(pos, b"\0", pos + 4096)?;
        }
    }
    if flags & 2 != 0 {
        pos += 2;
    }
    let mut st = InflateState::new_boxed(DataFormat::Raw);
    let mut out = vec![0u8; 64 << 10];
    let mut crc = crc32fast::Hasher::new();
    let mut size = 0u64;
    loop {
        if pos >= max {
            return None;
        }
        let inp = c.chunk(pos)?.to_vec();
        let mut inp = &inp[..];
        loop {
            let r = inflate(&mut st, inp, &mut out, MZFlush::None);
            crc.update(&out[..r.bytes_written]);
            size += r.bytes_written as u64;
            inp = &inp[r.bytes_consumed..];
            pos += r.bytes_consumed as u64;
            match r.status {
                Ok(MZStatus::StreamEnd) => {
                    let t = c.get(pos, 8)?;
                    let ok = le32(t) as u32 == crc.finalize() && le32(&t[4..]) == size & 0xFFFF_FFFF;
                    return ok.then_some(pos + 8);
                }
                Ok(_) | Err(MZError::Buf) => {}
                Err(_) => return None,
            }
            if (inp.is_empty() && r.bytes_written < out.len()) || (r.bytes_consumed == 0 && r.bytes_written == 0) {
                break;
            }
        }
    }
}

fn psd(c: &mut Cursor, max: u64) -> Option<u64> {
    let h = c.get(0, 26)?.to_vec();
    let big = h[5] == 2;
    let be16 = |b: &[u8]| (b[0] as u64) << 8 | b[1] as u64;
    let (channels, height, width, depth) = (be16(&h[12..]), be32(&h[14..]), be32(&h[18..]), be16(&h[22..]));
    if !(1..=56).contains(&channels) || height == 0 || width == 0 || !matches!(depth, 1 | 8 | 16 | 32) {
        return None;
    }
    let mut pos = 26u64;
    for section in 0..3 {
        let len = if section == 2 && big { u64::from_be_bytes(c.get(pos, 8)?.try_into().unwrap()) + 8 } else { be32(c.get(pos, 4)?) + 4 };
        pos += len;
        if pos > max {
            return None;
        }
    }
    let lines = channels * height;
    let compression = be16(c.get(pos, 2)?);
    pos += 2;
    match compression {
        0 => pos += lines * (width * depth).div_ceil(8),
        1 => {
            let w = if big { 4 } else { 2 };
            if lines * w > 64 << 20 {
                return None;
            }
            let counts = c.get(pos, (lines * w) as usize)?;
            let sum: u64 = counts.chunks(w as usize).map(|v| if big { be32(v) } else { be16(v) }).sum();
            pos += lines * w + sum;
        }
        _ => return None,
    }
    (pos <= max).then_some(pos)
}

fn rd16(le: bool, b: &[u8]) -> u64 {
    let v = [b[0], b[1]];
    (if le { u16::from_le_bytes(v) } else { u16::from_be_bytes(v) }) as u64
}
fn rd32(le: bool, b: &[u8]) -> u64 {
    let v = b[..4].try_into().unwrap();
    (if le { u32::from_le_bytes(v) } else { u32::from_be_bytes(v) }) as u64
}

/// TIFF (TIFF 6.0, with the EXIF and DNG additions), and the raw photos built on it. The file is a
/// header and image file directories (IFDs) that point at everything else: it ends at the far end
/// of whatever they point at.
fn tiff(c: &mut Cursor, max: u64) -> Option<(u64, &'static str)> {
    let h = c.get(0, 16)?.to_vec();
    let le = h[0] == b'I';
    let mut end = 8u64;
    let mut queue = vec![rd32(le, &h[4..])];
    let mut seen: Vec<u64> = Vec::new();
    let (mut dng, mut sub, mut make) = (false, false, None);
    let mut has_data = false;
    while let Some(ifd) = queue.pop() {
        if ifd < 8 || ifd >= max || seen.contains(&ifd) || seen.len() >= 64 {
            continue;
        }
        let first = seen.is_empty();
        seen.push(ifd);
        let n = match c.get(ifd, 2) {
            Some(b) => rd16(le, b) as usize,
            None if first => return None,
            None => continue,
        };
        let table = if (1..=1024).contains(&n) { c.get(ifd + 2, n * 12 + 4).map(|t| t.to_vec()) } else { None };
        let Some(table) = table else {
            if first {
                return None;
            }
            continue;
        };
        end = end.max(ifd + 2 + n as u64 * 12 + 4);
        let next = rd32(le, &table[n * 12..]);
        if next != 0 {
            queue.push(next);
        }
        // offsets and byte counts of the strips, of the tiles, of the JPEG thumbnail
        let mut offs: [Vec<u64>; 3] = Default::default();
        let mut lens: [Vec<u64>; 3] = Default::default();
        for e in table[..n * 12].chunks(12) {
            let tag = rd16(le, e);
            let typ = rd16(le, &e[2..]);
            let count = rd32(le, &e[4..]);
            let unit = match typ {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 | 13 => 4,
                5 | 10 | 12 => 8,
                _ if first => return None,
                _ => continue,
            };
            let total = unit * count;
            let at = rd32(le, &e[8..]);
            if total > 4 {
                if at + total > max {
                    if first {
                        return None;
                    }
                    continue;
                }
                end = end.max(at + total);
            }
            let read = |c: &mut Cursor, n: u64| -> Vec<u8> {
                if total <= 4 { e[8..12].to_vec() } else { c.get(at, n as usize).map(|v| v.to_vec()).unwrap_or_default() }
            };
            let wants = matches!(tag, 273 | 279 | 324 | 325 | 513 | 514 | 330 | 34665 | 34853 | 40965);
            let vals: Vec<u64> = if wants && matches!(unit, 2 | 4) && count <= 1 << 20 {
                read(c, total).chunks_exact(unit as usize).map(|v| if unit == 2 { rd16(le, v) } else { rd32(le, v) }).collect()
            } else {
                Vec::new()
            };
            match tag {
                273 => offs[0] = vals,
                279 => lens[0] = vals,
                324 => offs[1] = vals,
                325 => lens[1] = vals,
                513 => offs[2] = vals,
                514 => lens[2] = vals,
                330 => {
                    sub = true;
                    queue.extend(vals);
                }
                34665 | 34853 | 40965 => queue.extend(vals),
                50706 => dng = true,
                271 if make.is_none() => {
                    let s = read(c, total.min(16)).to_ascii_uppercase();
                    make = [(&b"NIKON"[..], "nef"), (b"SONY", "arw"), (b"PENTAX", "pef"), (b"RICOH", "pef")]
                        .iter()
                        .find(|(m, _)| s.starts_with(m))
                        .map(|&(_, x)| x);
                }
                _ => {}
            }
        }
        for k in 0..3 {
            for (&o, &l) in offs[k].iter().zip(&lens[k]) {
                if l > 0 && o + l <= max {
                    end = end.max(o + l);
                    has_data = true;
                }
            }
        }
    }
    let ext = if dng {
        "dng"
    } else if matches!(&h[..4], b"IIRO" | b"IIRS" | b"MMOR") {
        "orf"
    } else if &h[..4] == b"IIU\0" {
        "rw2"
    } else if &h[8..10] == b"CR" {
        "cr2"
    } else if sub {
        make.unwrap_or("tif")
    } else {
        "tif"
    };
    has_data.then_some((end, ext))
}

/// Microsoft compound files (the Office 97-2003 formats: doc, xls, ppt...). A header, then
/// sectors chained by a file allocation table (FAT): the file ends after the last sector in use.
fn ole(c: &mut Cursor, max: u64) -> Option<(u64, &'static str)> {
    const FREE: u64 = 0xFFFF_FFFF;
    let h = c.get(0, 512)?.to_vec();
    let shift = le16(&h[30..]);
    if shift != 9 && shift != 12 {
        return None;
    }
    let ssz = 1u64 << shift;
    let per = ssz / 4;
    let nfat = le32(&h[44..]);
    if nfat == 0 || nfat > max / ssz / per + 1 {
        return None;
    }
    // where the FAT lies: 109 sectors named in the header, then a chain of DIFAT sectors
    let mut fat_secs: Vec<u64> = h[76..512].chunks(4).map(le32).filter(|&s| s != FREE).collect();
    let mut difat = le32(&h[68..]);
    let mut hops = 0;
    while (fat_secs.len() as u64) < nfat && difat < 0xFFFF_FFFA && hops <= le32(&h[72..]) {
        let d = c.get((difat + 1) * ssz, ssz as usize)?.to_vec();
        let k = (per - 1) as usize * 4;
        fat_secs.extend(d[..k].chunks(4).map(le32));
        difat = le32(&d[k..]);
        hops += 1;
    }
    if (fat_secs.len() as u64) < nfat {
        return None;
    }
    fat_secs.truncate(nfat as usize);
    let mut fat: Vec<u64> = Vec::with_capacity((nfat * per) as usize);
    for &s in &fat_secs {
        if s >= 0xFFFF_FFFA || (s + 2) * ssz > max {
            return None;
        }
        fat.extend(c.get((s + 1) * ssz, ssz as usize)?.chunks(4).map(le32));
    }
    let last = fat.iter().rposition(|&e| e != FREE)? as u64;
    let len = (last + 2) * ssz;
    if len > max {
        return None;
    }
    // which application wrote it: the names of the streams in the directory
    let mut ext = "ole";
    let mut s = le32(&h[48..]);
    let mut hops = 0;
    'dir: while (s as usize) < fat.len() && hops < 64 {
        let Some(d) = c.get((s + 1) * ssz, ssz as usize) else { break };
        for e in d.chunks(128) {
            let n = (le16(&e[64..]) as usize).min(64);
            let units: Vec<u16> = e[..n].as_chunks::<2>().0.iter().map(|&u| u16::from_le_bytes(u)).take_while(|&u| u != 0).collect();
            ext = match String::from_utf16_lossy(&units).as_str() {
                "WordDocument" => "doc",
                "Workbook" | "Book" => "xls",
                "PowerPoint Document" => "ppt",
                "VisioDocument" => "vsd",
                _ => continue,
            };
            break 'dir;
        }
        s = fat[s as usize];
        hops += 1;
    }
    Some((len, ext))
}

/// PCX (ZSoft): a 128-byte header, run-length encoded scan lines, and for 256-colour images a
/// palette after a 0x0C byte. The file ends once every line has been decoded.
fn pcx(c: &mut Cursor, max: u64) -> Option<u64> {
    let h = c.get(0, 128)?.to_vec();
    let width = le16(&h[8..]) - le16(&h[4..]) + 1;
    let height = le16(&h[10..]) - le16(&h[6..]) + 1;
    let bpl = le16(&h[66..]);
    let planes = h[65] as u64;
    if bpl < (width * h[3] as u64).div_ceil(8) {
        return None;
    }
    let total = planes * bpl * height;
    let (mut pos, mut done) = (128u64, 0u64);
    while done < total {
        let chunk = c.chunk(pos)?;
        let mut i = 0;
        while i < chunk.len() && done < total {
            if chunk[i] & 0xC0 == 0xC0 {
                if i + 1 >= chunk.len() {
                    break;
                }
                done += (chunk[i] & 0x3F) as u64;
                i += 2;
            } else {
                done += 1;
                i += 1;
            }
        }
        if i == 0 {
            // a run split across two blocks: reload from it
            c.get(pos, 2)?;
            continue;
        }
        pos += i as u64;
        if pos > max {
            return None;
        }
    }
    if h[1] == 5 && h[3] == 8 && planes == 1 && c.get(pos, 1) == Some(&[0x0C][..]) {
        pos += 769;
    }
    Some(pos)
}

/// HTML, XHTML, SVG and other XML: text up to the end tag of the root element. The text stops at
/// the first byte no text file holds, or at a sector where another file starts.
fn markup(c: &mut Cursor, max: u64) -> Option<(u64, &'static str)> {
    let mut text = Vec::new();
    let mut pos = 0u64;
    while pos < max {
        let sec = match c.get(pos, 512) {
            Some(s) => s.to_vec(),
            None => c.chunk(pos).map(|s| s.to_vec()).unwrap_or_default(),
        };
        if sec.is_empty() || (pos > 0 && detect(&sec).is_some()) {
            break;
        }
        if let Some(i) = sec.iter().position(|&b| b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r' | 0x0C)) {
            text.extend_from_slice(&sec[..i]);
            break;
        }
        text.extend_from_slice(&sec);
        pos += sec.len() as u64;
    }
    let lower = text.to_ascii_lowercase();
    let (root, doctype_html) = root_element(&lower)?;
    let html = doctype_html || root == "html";
    let close = format!("</{}>", if html { "html" } else { &root });
    let end = match lower.windows(close.len()).rposition(|w| w == close.as_bytes()) {
        Some(i) => {
            let e = i + close.len();
            // with the line breaks that often end a text file, not the old bytes after them
            e + lower[e..].iter().take_while(|&&b| b == b'\r' || b == b'\n').count()
        }
        None => text.len(),
    };
    let ext = if html {
        "html"
    } else if root == "svg" {
        "svg"
    } else {
        "xml"
    };
    Some((end as u64, ext))
}

/// The name of the first element of a (lower-cased) document, after the prolog, and whether a
/// doctype says it is HTML.
fn root_element(t: &[u8]) -> Option<(String, bool)> {
    let find = |r: &[u8], p: &[u8]| r.windows(p.len()).position(|w| w == p);
    let mut i = if t.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    let mut html = false;
    loop {
        i += t.get(i..)?.iter().take_while(|b| b.is_ascii_whitespace()).count();
        let r = &t[i..];
        if r.starts_with(b"<?") {
            i += find(r, b"?>")? + 2;
        } else if r.starts_with(b"<!--") {
            i += find(r, b"-->")? + 3;
        } else if r.starts_with(b"<!") {
            html |= r.starts_with(b"<!doctype html");
            let gt = find(r, b">")?;
            // an internal subset ends with "]>"
            i += if r[..gt].contains(&b'[') { find(r, b"]>")? + 2 } else { gt + 1 };
        } else if r.starts_with(b"<") && r.len() > 1 {
            let name: String = r[1..].iter().take_while(|b| b.is_ascii_alphanumeric() || b"_:.-".contains(b)).map(|&b| b as char).collect();
            return if name.is_empty() { html.then(|| ("html".into(), true)) } else { Some((name, html)) };
        } else {
            return html.then(|| ("html".into(), true));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mem(Vec<u8>);
    impl Source for Mem {
        fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> usize {
            let o = offset as usize;
            if o >= self.0.len() {
                return 0;
            }
            let n = buf.len().min(self.0.len() - o);
            buf[..n].copy_from_slice(&self.0[o..o + n]);
            n
        }
    }

    #[test]
    fn pdf_runs_to_its_last_eof() {
        // incremental update: two bodies, two `%%EOF`
        let mut upd = b"%PDF-1.4\n1 0 obj<<>>stream\n".to_vec();
        upd.extend(std::iter::repeat_n(b'y', 300));
        upd.extend_from_slice(b"endstream endobj\nxref\ntrailer<<>>\n%%EOF\n");
        upd.extend_from_slice(b"2 0 obj<<>>endobj\nxref\ntrailer<<>>\n%%EOF\r\n");
        // linearized: `/L` gives the length, with a `%%EOF` after the first page
        let mut lin = b"%PDF-1.5\n1 0 obj<</Linearized 1/L 0000000000/O 3>>endobj\n".to_vec();
        lin.extend_from_slice(b"xref\ntrailer<<>>\n%%EOF\n");
        lin.extend(std::iter::repeat_n(b'x', 700));
        lin.extend_from_slice(b"\nstartxref\n9\n%%EOF\n");
        let l = format!("{:010}", lin.len());
        let at = lin.windows(10).position(|w| w == b"0000000000").unwrap();
        lin[at..at + 10].copy_from_slice(l.as_bytes());

        let mut disk = vec![0u8; 1024];
        let upd_at = disk.len();
        disk.extend_from_slice(&upd);
        disk.extend_from_slice(b"\0\0garbage %%EOF\n");
        disk.resize(4096, 0);
        let lin_at = disk.len();
        disk.extend_from_slice(&lin);
        disk.extend_from_slice(b"junk 1 0 obj %%EOF\n");
        disk.resize(disk.len() + 4096, 0);

        let mut m = Mem(disk);
        assert_eq!(measure(&mut m, upd_at as u64, Kind::Pdf).unwrap().0, upd.len() as u64);
        assert_eq!(measure(&mut m, lin_at as u64, Kind::Pdf).unwrap().0, lin.len() as u64);
    }

    #[test]
    fn pdf_cut_short_stops_at_the_next_pdf() {
        // a PDF whose end is elsewhere, then another one on a sector boundary, whole
        let mut disk = vec![0u8; 512];
        disk.extend_from_slice(b"%PDF-1.4\n1 0 obj<<>>stream\n");
        disk.resize(512 + 6 * 1024 * 1024, b'q');
        let second = disk.len() as u64;
        disk.extend_from_slice(b"%PDF-1.4\n1 0 obj<<>>endobj\n");
        disk.extend(std::iter::repeat_n(b'z', 400));
        disk.extend_from_slice(b"\nxref\ntrailer<<>>\n%%EOF\n");
        let second_len = disk.len() as u64 - second;
        disk.resize(disk.len() + 4096, 0);
        let mut m = Mem(disk);
        assert_eq!(measure(&mut m, 512, Kind::Pdf).unwrap().0, second - 512);
        assert_eq!(measure(&mut m, second, Kind::Pdf).unwrap().0, second_len);
        // the same with an early `%%EOF`, as after the first page of a linearized PDF: the
        // objects after it still belong to the file, up to the next one
        let mut d = m.0;
        let early = b"%PDF-1.4\n1 0 obj<<>>endobj\nxref\ntrailer<<>>\n%%EOF\n2 0 obj<<>>stream\n";
        d[512..512 + early.len()].copy_from_slice(early);
        let mut m = Mem(d);
        assert_eq!(measure(&mut m, 512, Kind::Pdf).unwrap().0, second - 512);
    }

    #[test]
    fn png_and_jpeg_lengths() {
        let mut img = image::RgbImage::new(64, 48);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgb([(x * 4) as u8, (y * 5) as u8, 90]);
        }
        let mut png_bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).unwrap();
        let mut jpg_bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut jpg_bytes), image::ImageFormat::Jpeg).unwrap();

        let mut disk = vec![0u8; 4096];
        let png_at = disk.len();
        disk.extend_from_slice(&png_bytes);
        disk.resize(disk.len().div_ceil(512) * 512 + 1024, 0xAB);
        let jpg_at = disk.len();
        disk.extend_from_slice(&jpg_bytes);
        disk.extend(std::iter::repeat_n(0x11u8, 3000));

        assert_eq!(detect(&disk[png_at..]), Some(Kind::Png));
        assert_eq!(detect(&disk[jpg_at..]), Some(Kind::Jpeg));
        let mut m = Mem(disk);
        assert_eq!(measure(&mut m, png_at as u64, Kind::Png).unwrap().0, png_bytes.len() as u64);
        assert_eq!(measure(&mut m, jpg_at as u64, Kind::Jpeg).unwrap().0, jpg_bytes.len() as u64);
    }

    /// A ZIP holding one stored entry.
    fn stored_zip(name: &[u8], data: &[u8]) -> Vec<u8> {
        let crc = crc32fast::hash(data);
        let (n, d) = (name.len() as u16, data.len() as u32);
        let mut z = b"PK\x03\x04\x14\0\0\0\0\0\0\0\0\0".to_vec();
        z.extend(crc.to_le_bytes());
        z.extend(d.to_le_bytes());
        z.extend(d.to_le_bytes());
        z.extend(n.to_le_bytes());
        z.extend(0u16.to_le_bytes());
        z.extend_from_slice(name);
        z.extend_from_slice(data);
        let cd = z.len() as u32;
        z.extend(b"PK\x01\x02\x14\0\x14\0\0\0\0\0\0\0\0\0");
        z.extend(crc.to_le_bytes());
        z.extend(d.to_le_bytes());
        z.extend(d.to_le_bytes());
        z.extend(n.to_le_bytes());
        z.extend([0u8; 12]);
        z.extend(0u32.to_le_bytes());
        z.extend_from_slice(name);
        let cd_size = z.len() as u32 - cd;
        z.extend(b"PK\x05\x06\0\0\0\0\x01\0\x01\0");
        z.extend(cd_size.to_le_bytes());
        z.extend(cd.to_le_bytes());
        z.extend(0u16.to_le_bytes());
        z
    }

    #[test]
    fn zip_does_not_borrow_another_archives_end() {
        let a = stored_zip(b"a.txt", &[b'a'; 3000]);
        let b = stored_zip(b"b.txt", &[b'b'; 2000]);
        // the first archive lost its central directory and end record to other data
        let cut = a.len() - 60;
        let mut disk = vec![0u8; 1024];
        disk.extend_from_slice(&a[..cut]);
        disk.resize(disk.len().div_ceil(512) * 512 + 2048, 0x5A);
        let b_at = disk.len();
        disk.extend_from_slice(&b);
        disk.resize(disk.len() + 4096, 0);
        let mut m = Mem(disk);
        assert_eq!(measure(&mut m, 1024, Kind::Zip), None);
        assert_eq!(measure(&mut m, b_at as u64, Kind::Zip).unwrap().0, b.len() as u64);

        // the same archive cut in two, with other data between the pieces: still read to its end
        let mut disk = vec![0u8; 1024];
        disk.extend_from_slice(&a[..2048]);
        disk.extend(std::iter::repeat_n(0x5Au8, 4096));
        disk.extend_from_slice(&a[2048..]);
        disk.resize(disk.len() + 4096, 0);
        assert_eq!(measure(&mut Mem(disk), 1024, Kind::Zip).unwrap().0, a.len() as u64 + 4096);
    }

    /// `file` at sector 2 of a disk, followed by `after`; returns what the carver makes of it.
    fn carve_one(file: &[u8], after: u8) -> Option<(Kind, u64, &'static str)> {
        let mut disk = vec![0u8; 1024];
        disk.extend_from_slice(file);
        disk.extend(std::iter::repeat_n(after, 4096));
        let kind = detect(&disk[1024..])?;
        let (len, ext) = measure(&mut Mem(disk), 1024, kind)?;
        Some((kind, len, ext))
    }

    #[test]
    fn icons_fonts_and_gzip() {
        // an icon holding one PNG
        let img = image::RgbaImage::from_fn(32, 32, |x, y| image::Rgba([(x * 7) as u8, (y * 9) as u8, (x * y) as u8, 255]));
        let mut png_bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).unwrap();
        let mut ico = vec![0, 0, 1, 0, 1, 0, 32, 32, 0, 0, 1, 0, 32, 0];
        ico.extend_from_slice(&(png_bytes.len() as u32).to_le_bytes());
        ico.extend_from_slice(&22u32.to_le_bytes());
        ico.extend_from_slice(&png_bytes);
        assert_eq!(carve_one(&ico, 0x55), Some((Kind::Ico, ico.len() as u64, "ico")));

        // a font with four tables, the last one not a multiple of four long
        let mut ttf = vec![0, 1, 0, 0, 0, 4, 0, 64, 0, 2, 0, 0];
        let mut off = 12 + 16 * 4u32;
        for (tag, len) in [(b"cmap", 100u32), (b"glyf", 300), (b"head", 54), (b"name", 61)] {
            ttf.extend_from_slice(tag);
            ttf.extend_from_slice(&[0; 4]);
            ttf.extend_from_slice(&off.to_be_bytes());
            ttf.extend_from_slice(&len.to_be_bytes());
            off += len.div_ceil(4) * 4;
        }
        ttf.resize(off as usize, 7);
        assert_eq!(carve_one(&ttf, 0x55), Some((Kind::Font, ttf.len() as u64, "ttf")));

        // a gzip file
        let text: Vec<u8> = (0..20_000u32).flat_map(|i| format!("line {i}\n").into_bytes()).collect();
        let mut gz = vec![0x1F, 0x8B, 8, 0, 0, 0, 0, 0, 0, 11];
        gz.extend_from_slice(&miniz_oxide::deflate::compress_to_vec(&text, 6));
        gz.extend_from_slice(&crc32fast::hash(&text).to_le_bytes());
        gz.extend_from_slice(&(text.len() as u32).to_le_bytes());
        assert_eq!(carve_one(&gz, 0x55), Some((Kind::Gzip, gz.len() as u64, "gz")));
    }

    #[test]
    fn tiff_ends_after_its_strips() {
        let mut f = b"II*\0\x08\0\0\0".to_vec();
        f.extend_from_slice(&[3, 0]); // three entries
        for (tag, typ, val) in [(256u16, 3u16, 20u32), (273, 4, 100), (279, 4, 400)] {
            f.extend_from_slice(&tag.to_le_bytes());
            f.extend_from_slice(&typ.to_le_bytes());
            f.extend_from_slice(&1u32.to_le_bytes());
            f.extend_from_slice(&val.to_le_bytes());
        }
        f.extend_from_slice(&0u32.to_le_bytes());
        f.resize(100, 0);
        f.extend((0..400).map(|i| (i % 251) as u8));
        assert_eq!(carve_one(&f, 0x11), Some((Kind::Tiff, 500, "tif")));
    }

    #[test]
    fn ole_ends_after_its_last_sector_and_says_what_it_is() {
        const END: u32 = 0xFFFF_FFFE;
        let mut f = vec![0u8; 512];
        f[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
        f[24..28].copy_from_slice(&[0x3E, 0, 3, 0]);
        f[28..30].copy_from_slice(&[0xFE, 0xFF]);
        f[30] = 9; // 512-byte sectors
        f[32] = 6;
        f[44] = 1; // one FAT sector...
        f[48] = 1; // ...the directory in sector 1
        f[68..72].copy_from_slice(&END.to_le_bytes());
        for e in f[80..512].chunks_mut(4) {
            e.copy_from_slice(&[0xFF; 4]);
        }
        // sector 0: the FAT; 1: the directory; 2..=4: one stream
        let mut fat = [0xFFFF_FFFFu32; 128];
        fat[..5].copy_from_slice(&[0xFFFF_FFFD, END, 3, 4, END]);
        f.extend(fat.iter().flat_map(|e| e.to_le_bytes()));
        let mut dir = vec![0u8; 512];
        for (i, name) in ["Root Entry", "Workbook"].iter().enumerate() {
            let units: Vec<u8> = name.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
            dir[i * 128..i * 128 + units.len()].copy_from_slice(&units);
            dir[i * 128 + 64] = (units.len() + 2) as u8;
        }
        f.extend_from_slice(&dir);
        f.extend(std::iter::repeat_n(0x42u8, 3 * 512));
        assert_eq!(carve_one(&f, 0x11), Some((Kind::Ole, 3072, "xls")));
    }

    #[test]
    fn pcx_ends_after_its_palette() {
        let mut f = vec![0u8; 128];
        f[..4].copy_from_slice(&[0x0A, 5, 1, 8]);
        f[8] = 9; // 10 x 5 pixels
        f[10] = 4;
        f[65] = 1;
        f[66] = 10;
        for _ in 0..5 {
            f.extend_from_slice(&[0xC5, 7, 1, 2, 3, 4, 5]);
        }
        f.push(0x0C);
        f.extend(std::iter::repeat_n(0x80u8, 768));
        assert_eq!(carve_one(&f, 0xFF), Some((Kind::Pcx, 932, "pcx")));
    }

    #[test]
    fn markup_ends_at_the_root_end_tag() {
        let html = format!("\n<!DOCTYPE html>\n<html><body>{}</body></HTML>\r\n", "x".repeat(300));
        assert_eq!(carve_one(html.as_bytes(), b' '), Some((Kind::Markup, html.len() as u64, "html")));
        // the rest of the last sector holds old text, not part of the file
        let xml = format!("<?xml version=\"1.0\"?>\n<!-- note -->\n<root><a>{}</a></root>\n", "y".repeat(200));
        assert_eq!(carve_one(xml.as_bytes(), b'z'), Some((Kind::Markup, xml.len() as u64, "xml")));
        let svg = "<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\"><rect/></svg>";
        assert_eq!(carve_one(svg.as_bytes(), 0).map(|r| r.2), Some("svg"));
    }

    #[test]
    fn headers_inside_a_file() {
        let mut open = Open::default();
        open.push(Kind::Zip, 10_000);
        assert!(!open.allows(512, Kind::Zip), "an entry of the zip");
        assert!(open.allows(1024, Kind::Jpeg), "a photo written into a gap of the zip");
        assert!(open.allows(10_000, Kind::Zip), "past its end");
        open.push(Kind::Jpeg, 20_000);
        assert!(open.allows(12_000, Kind::Jpeg), "a photo inside another one");
    }

    fn boxed(typ: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        b.extend_from_slice(typ);
        b.extend_from_slice(body);
        b
    }

    #[test]
    fn iso_media_named_by_brand() {
        let heic = [boxed(b"ftyp", b"heic\0\0\0\0mif1heic"), boxed(b"meta", &[0; 300]), boxed(b"mdat", &[7; 200])].concat();
        assert_eq!(carve_one(&heic, 0x11), Some((Kind::Mp4, heic.len() as u64, "heic")));
        let avif = [boxed(b"ftyp", b"mif1\0\0\0\0mif1avif"), boxed(b"meta", &[0; 300])].concat();
        assert_eq!(carve_one(&avif, 0x11), Some((Kind::Mp4, avif.len() as u64, "avif")));
        let cr3 = [boxed(b"ftyp", b"crx \0\0\0\x01crx isom"), boxed(b"moov", &[0; 300]), boxed(b"mdat", &[7; 200])].concat();
        assert_eq!(carve_one(&cr3, 0x11), Some((Kind::Mp4, cr3.len() as u64, "cr3")));
        // a HEIF photo needs its meta box, a video its movie box
        let no_moov = [boxed(b"ftyp", b"isom\0\0\0\0isom"), boxed(b"mdat", &[7; 400])].concat();
        assert_eq!(carve_one(&no_moov, 0x11), None);
    }

    #[test]
    fn webm_of_unknown_size_ends_after_its_clusters() {
        let mut f = vec![0x1A, 0x45, 0xDF, 0xA3, 0x87, 0x42, 0x82, 0x84];
        f.extend_from_slice(b"webm");
        f.extend_from_slice(&[0x18, 0x53, 0x80, 0x67, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        f.extend_from_slice(&[0x15, 0x49, 0xA9, 0x66, 0x94]);
        f.extend_from_slice(&[0; 20]);
        for _ in 0..2 {
            f.extend_from_slice(&[0x1F, 0x43, 0xB6, 0x75, 0xFF, 0xE7, 0x81, 0x00]);
            for _ in 0..2 {
                f.extend_from_slice(&[0xA3, 0x40, 200]);
                f.extend_from_slice(&[0x55; 200]);
            }
        }
        assert_eq!(carve_one(&f, 0x11), Some((Kind::Mkv, f.len() as u64, "webm")));
    }

    #[test]
    fn transport_and_program_streams() {
        let packet = |prefix: usize| {
            let mut p = vec![0u8; prefix];
            p.extend_from_slice(&[0x47, 0x40, 0x00, 0x10]);
            p.resize(prefix + 188, 0xFF);
            p
        };
        let ts = packet(0).repeat(20);
        assert_eq!(carve_one(&ts, 0x11), Some((Kind::Ts, ts.len() as u64, "m2t")));
        let m2ts = packet(4).repeat(20);
        assert_eq!(carve_one(&m2ts, 0x11), Some((Kind::Ts, m2ts.len() as u64, "mts")));

        let mut ps = Vec::new();
        for _ in 0..5 {
            ps.extend_from_slice(&[0, 0, 1, 0xBA, 0x44, 0, 4, 0, 4, 1, 0, 0, 3, 0xF8]);
            ps.extend_from_slice(&[0, 0, 1, 0xE0, 0, 100]);
            ps.extend_from_slice(&[0x80; 100]);
        }
        ps.extend_from_slice(&[0, 0, 1, 0xB9]);
        assert_eq!(carve_one(&ps, 0x11), Some((Kind::Mpeg, ps.len() as u64, "mpg")));
    }

    #[test]
    fn flac_ends_with_its_last_sample() {
        let mut f = b"fLaC".to_vec();
        f.extend_from_slice(&[0x80, 0, 0, 34]);
        let mut info = [0u8; 34];
        info[..4].copy_from_slice(&[0x10, 0, 0x10, 0]);
        info[10..13].copy_from_slice(&[0x0A, 0xC4, 0x40]); // 44.1 kHz, mono, 16 bits
        info[14..18].copy_from_slice(&(3 * 4096u32).to_be_bytes());
        f.extend_from_slice(&info);
        for k in 0..3u8 {
            let start = f.len();
            f.extend_from_slice(&[0xFF, 0xF8, 0xC9, 0x08, k]);
            f.push(crc8(&f[start..]));
            f.extend((0..300u32).map(|i| (i * 7 + k as u32 * 13) as u8));
            let crc = f[start..].iter().fold(0u16, |c, &b| (c << 8) ^ CRC16[((c >> 8) as u8 ^ b) as usize]);
            f.extend_from_slice(&crc.to_be_bytes());
        }
        assert_eq!(carve_one(&f, 0x11), Some((Kind::Flac, f.len() as u64, "flac")));
    }

    #[test]
    fn ogg_ends_with_its_last_page() {
        let page = |flags: u8, table: &[u8], body: &[u8]| {
            let mut p = b"OggS\0".to_vec();
            p.push(flags);
            p.extend_from_slice(&[0; 8]);
            p.extend_from_slice(&7u32.to_le_bytes());
            p.extend_from_slice(&[0; 8]);
            p.push(table.len() as u8);
            p.extend_from_slice(table);
            p.extend_from_slice(body);
            p
        };
        let mut head = b"OpusHead".to_vec();
        head.resize(30, 1);
        let f = [page(2, &[30], &head), page(0, &[255, 100], &[9; 355]), page(4, &[50], &[9; 50])].concat();
        assert_eq!(carve_one(&f, 0x11), Some((Kind::Ogg, f.len() as u64, "opus")));
    }

    #[test]
    fn bare_audio_frames() {
        let mut mp3 = Vec::new();
        for _ in 0..40 {
            mp3.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x64]);
            mp3.resize(mp3.len() + 413, 0x22);
        }
        assert_eq!(carve_one(&mp3, 0x11), Some((Kind::Mp3, mp3.len() as u64, "mp3")));
        let mut aac = Vec::new();
        for _ in 0..40 {
            aac.extend_from_slice(&[0xFF, 0xF1, 0x50, 0x80, (300 >> 3) as u8, ((300 & 7) << 5 | 0x1F) as u8, 0xFC]);
            aac.resize(aac.len() + 293, 0x22);
        }
        assert_eq!(carve_one(&aac, 0x11), Some((Kind::Aac, aac.len() as u64, "aac")));
        // a few frames are not a song
        assert_eq!(carve_one(&mp3[..417 * 5], 0x11), None);
    }

    #[test]
    fn asf_takes_its_size_from_its_header() {
        let mut f = ASF_HEADER.to_vec();
        f.extend_from_slice(&150u64.to_le_bytes());
        f.extend_from_slice(&[2, 0, 0, 0, 1, 2]);
        f.extend_from_slice(&[0xA1, 0xDC, 0xAB, 0x8C, 0x47, 0xA9, 0xCF, 0x11, 0x8E, 0xE4, 0x00, 0xC0, 0x0C, 0x20, 0x53, 0x65]);
        let mut props = vec![0u8; 88];
        props[..8].copy_from_slice(&104u64.to_le_bytes());
        props[24..32].copy_from_slice(&600u64.to_le_bytes());
        f.extend_from_slice(&props);
        f.extend_from_slice(&[0x40, 0x9E, 0x69, 0xF8, 0x4D, 0x5B, 0xCF, 0x11, 0xA8, 0xFD, 0x00, 0x80, 0x5F, 0x5C, 0x44, 0x2B]);
        assert_eq!(f.len(), 150);
        f.resize(600, 0x33);
        assert_eq!(carve_one(&f, 0x11), Some((Kind::Asf, 600, "wma")));
    }

    #[test]
    fn flv_aiff_and_caf() {
        let mut flv = b"FLV\x01\x05\0\0\0\x09\0\0\0\0".to_vec();
        for _ in 0..3 {
            flv.extend_from_slice(&[9, 0, 0, 100, 0, 0, 0, 0, 0, 0, 0]);
            flv.extend_from_slice(&[0x44; 100]);
            flv.extend_from_slice(&111u32.to_be_bytes());
        }
        assert_eq!(carve_one(&flv, 0x11), Some((Kind::Flv, flv.len() as u64, "flv")));

        let mut aiff = b"FORM".to_vec();
        aiff.extend_from_slice(&592u32.to_be_bytes());
        aiff.extend_from_slice(b"AIFFCOMM");
        aiff.resize(600, 0x44);
        assert_eq!(carve_one(&aiff, 0x11), Some((Kind::Aiff, 600, "aif")));

        let mut caf = b"caff\0\x01\0\0desc".to_vec();
        caf.extend_from_slice(&32u64.to_be_bytes());
        caf.extend_from_slice(&[0x44; 32]);
        caf.extend_from_slice(b"data");
        caf.extend_from_slice(&400u64.to_be_bytes());
        caf.extend_from_slice(&[0x44; 400]);
        assert_eq!(carve_one(&caf, 0x11), Some((Kind::Caf, caf.len() as u64, "caf")));
    }

    #[test]
    fn raw_photos_that_are_not_tiff() {
        let mut raf = b"FUJIFILMCCD-RAW 0201FF383501".to_vec();
        raf.resize(0x54, b' ');
        for (at, len) in [(0x100u32, 200u32), (0x200, 50), (0x300, 1000)] {
            raf.extend_from_slice(&at.to_be_bytes());
            raf.extend_from_slice(&len.to_be_bytes());
        }
        raf.resize(0x300 + 1000, 0x55);
        assert_eq!(carve_one(&raf, 0x11), Some((Kind::Raf, raf.len() as u64, "raf")));

        // CIFF: a sub-heap holding one value, then the table of the whole heap
        let mut crw = b"II\x1a\0\0\0HEAPCCDRAAAA\0\0\0\0\0\0\0\0".to_vec();
        let entry = |typ: u16, size: u32, at: u32| [&typ.to_le_bytes()[..], &size.to_le_bytes(), &at.to_le_bytes()].concat();
        let mut heap = vec![0xAA; 400];
        let mut sub = vec![0xBB; 20];
        sub.extend_from_slice(&1u16.to_le_bytes());
        sub.extend(entry(0x1010, 20, 0));
        sub.extend_from_slice(&20u32.to_le_bytes());
        heap.extend_from_slice(&sub);
        let table = heap.len() as u32;
        heap.extend_from_slice(&2u16.to_le_bytes());
        heap.extend(entry(0x2007, 400, 0));
        heap.extend(entry(0x300A, sub.len() as u32, 400));
        heap.extend_from_slice(&table.to_le_bytes());
        crw.extend_from_slice(&heap);
        assert_eq!(carve_one(&crw, 0x11), Some((Kind::Crw, crw.len() as u64, "crw")));

        let mut orf = b"IIRO\x08\0\0\0\x02\0".to_vec();
        for (tag, value) in [(273u16, 64u32), (279, 400)] {
            orf.extend_from_slice(&tag.to_le_bytes());
            orf.extend_from_slice(&4u16.to_le_bytes());
            orf.extend_from_slice(&1u32.to_le_bytes());
            orf.extend_from_slice(&value.to_le_bytes());
        }
        orf.resize(464, 0x66);
        assert_eq!(carve_one(&orf, 0x11), Some((Kind::Tiff, 464, "orf")));
    }

    #[test]
    fn psd_ends_after_its_merged_image() {
        let mut f = b"8BPS\0\x01\0\0\0\0\0\0\0\x03".to_vec();
        f.extend_from_slice(&10u32.to_be_bytes()); // height
        f.extend_from_slice(&20u32.to_be_bytes()); // width
        f.extend_from_slice(&[0, 8, 0, 3]);
        f.extend_from_slice(&0u32.to_be_bytes());
        f.extend_from_slice(&4u32.to_be_bytes());
        f.extend_from_slice(b"8BIM");
        f.extend_from_slice(&0u32.to_be_bytes());
        f.extend_from_slice(&1u16.to_be_bytes()); // RLE: the length of each of the 30 lines
        for _ in 0..30 {
            f.extend_from_slice(&6u16.to_be_bytes());
        }
        f.extend_from_slice(&[0xFD, 0x00, 0xFD, 0x00, 0xF9, 0x00].repeat(30));
        assert_eq!(carve_one(&f, 0x11), Some((Kind::Psd, f.len() as u64, "psd")));
    }
}
