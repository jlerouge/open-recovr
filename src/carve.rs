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
        }
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
        Kind::Zip => return zip(&mut c, max),
        Kind::Mp4 | Kind::Mov | Kind::M4a => mp4(&mut c, max),
        Kind::Avi | Kind::Wav | Kind::Webp => {
            let h = c.get(0, 12)?;
            let size = le32(&h[4..]) + 8;
            (size > 64 && size <= max).then_some(size + (size & 1))
        }
        Kind::Mp3 => mp3(&mut c, max),
    }?;
    let ext = match kind {
        Kind::Jpeg => "jpg",
        Kind::Png => "png",
        Kind::Gif => "gif",
        Kind::Bmp => "bmp",
        Kind::Webp => "webp",
        Kind::Pdf => "pdf",
        Kind::Zip => "zip",
        Kind::Mp4 => "mp4",
        Kind::Mov => "mov",
        Kind::M4a => "m4a",
        Kind::Avi => "avi",
        Kind::Wav => "wav",
        Kind::Mp3 => "mp3",
    };
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
}
