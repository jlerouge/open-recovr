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
            Kind::Mp3 | Kind::M4a => 80 * MB,
            Kind::Mp4 | Kind::Mov | Kind::Avi => 8192 * MB,
            Kind::Wav => 2048 * MB,
            Kind::Tiff => 400 * MB,
            Kind::Ole => 512 * MB,
            Kind::Pcx => 100 * MB,
            Kind::Markup => 64 * MB,
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
            Mp4 | Mov | M4a | Avi => matches!(inner, Mp4 | Mov | M4a | Avi | Jpeg),
            Tiff => matches!(inner, Tiff | Jpeg),
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
    if (b.starts_with(b"II*\0") && le32(&b[4..]) >= 8) || (b.starts_with(b"MM\0*") && be32(&b[4..]) >= 8) {
        return Some(Kind::Tiff);
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
    None
}

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
}

impl<'a> Cursor<'a> {
    fn new(src: &'a mut dyn Source, base: u64) -> Self {
        Self { src, base, cache_at: 0, cache: Vec::new() }
    }

    /// `n` bytes at `pos` (relative to the file start), or None past the end of the disk.
    fn get(&mut self, pos: u64, n: usize) -> Option<&[u8]> {
        let have = pos >= self.cache_at && pos + n as u64 <= self.cache_at + self.cache.len() as u64;
        if !have {
            let want = BLOCK.max(n);
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
        while pos < max {
            let c = self.get_window(pos, pat.len())?;
            if let Some(i) = c.windows(pat.len()).position(|w| w == pat) {
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

/// The file found at `offset`: its length and, for zip files, the more precise extension.
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
        Kind::Mp4 => "mp4",
        Kind::Mov => "mov",
        Kind::M4a => "m4a",
        Kind::Avi => "avi",
        Kind::Wav => "wav",
        Kind::Mp3 => "mp3",
        Kind::Pcx => "pcx",
        // worked out from the file itself
        Kind::Zip | Kind::Tiff | Kind::Ole | Kind::Markup => "",
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
        Kind::Pdf => {
            let end = c.find(8, b"%%EOF", max)?;
            // keep the line break that usually follows
            let tail = c.get(end, 2).map(|t| t.to_vec()).unwrap_or_default();
            let extra = tail.iter().take_while(|&&x| x == b'\r' || x == b'\n').count() as u64;
            Some(end + extra)
        }
        Kind::Mp4 | Kind::Mov | Kind::M4a => mp4(&mut c, max),
        Kind::Avi | Kind::Wav | Kind::Webp => {
            let h = c.get(0, 12)?;
            let size = le32(&h[4..]) + 8;
            (size > 64 && size <= max).then_some(size + (size & 1))
        }
        Kind::Mp3 => mp3(&mut c, max),
        Kind::Pcx => pcx(&mut c, max),
        Kind::Zip => return zip(&mut c, max),
        Kind::Tiff => return tiff(&mut c, max),
        Kind::Ole => return ole(&mut c, max),
        Kind::Markup => return markup(&mut c, max).filter(|&(len, _)| len >= 64),
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

fn mp4(c: &mut Cursor, max: u64) -> Option<u64> {
    const KNOWN: [&[u8; 4]; 12] =
        [b"ftyp", b"moov", b"mdat", b"free", b"skip", b"wide", b"uuid", b"meta", b"pnot", b"udta", b"moof", b"mfra"];
    let mut pos = 0u64;
    let (mut moov, mut mdat) = (false, false);
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
        pos += size;
        if pos > max {
            return None;
        }
    }
    (moov && mdat).then_some(pos)
}

fn mp3(c: &mut Cursor, max: u64) -> Option<u64> {
    const BR1: [u64; 15] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
    const BR2: [u64; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
    let h = c.get(0, 10)?;
    let tag = h[6..10].iter().fold(0u64, |a, &b| (a << 7) | b as u64);
    let mut pos = 10 + tag + if h[5] & 0x10 != 0 { 10 } else { 0 };
    let mut frames = 0;
    loop {
        if pos > max {
            return None;
        }
        let Some(f) = c.get(pos, 4) else { break };
        if f[0] != 0xFF || f[1] & 0xE0 != 0xE0 {
            break;
        }
        let version = (f[1] >> 3) & 3; // 3 = MPEG1, 2 = MPEG2, 0 = MPEG2.5
        let layer = (f[1] >> 1) & 3; // 1 = layer III
        let bri = (f[2] >> 4) as usize;
        let sri = ((f[2] >> 2) & 3) as usize;
        if version == 1 || layer != 1 || bri == 0 || bri == 15 || sri == 3 {
            break;
        }
        let pad = ((f[2] >> 1) & 1) as u64;
        let base = [44100u64, 48000, 32000][sri];
        let (sr, br, k) = match version {
            3 => (base, BR1[bri], 144),
            2 => (base / 2, BR2[bri], 72),
            _ => (base / 4, BR2[bri], 72),
        };
        pos += k * br * 1000 / sr + pad;
        frames += 1;
    }
    if c.get(pos, 3).is_some_and(|t| t == b"TAG") {
        pos += 128;
    }
    (frames >= 10).then_some(pos)
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
}
