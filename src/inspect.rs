//! Looks inside each file once it is listed: the date, camera or title written in it, a
//! better name for files rebuilt from raw bytes, whether the file is whole, and a
//! fingerprint of its content to spot copies of the same file.

use crate::engine::{Category, Found, Origin, read_found, read_raw};
use eframe::egui;
use std::hash::Hasher;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};

/// The most read of each file: enough for nearly every photo and document.
const CAP: u64 = 64 << 20;
/// Archives are unpacked to check them, up to this much.
const UNPACK_CAP: u64 = 512 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Not checked, or no check exists for this kind of file.
    Unknown,
    /// Read from end to end without a fault.
    Intact,
    /// Will not open properly, and why.
    Damaged(&'static str),
    /// Nothing but zeros: the space was wiped.
    Empty,
}

#[derive(Clone, Debug)]
pub struct Insight {
    pub id: usize,
    /// A name drawn from what the file says about itself, for files that only had a number.
    pub name: Option<String>,
    /// When the photo was taken, the video filmed, the document created (seconds since 1970).
    pub stamp: u64,
    /// Camera, picture size, title…
    pub detail: Option<String>,
    pub verdict: Verdict,
    /// Same content, same fingerprint.
    pub digest: Option<u64>,
}

/// What a file says about itself.
#[derive(Default, Debug, PartialEq)]
struct Meta {
    when: u64,
    title: Option<String>,
    artist: Option<String>,
    camera: Option<String>,
    dims: Option<(u32, u32)>,
    seconds: Option<u64>,
}

// ---------------------------------------------------------------- the worker

/// Inspects the found files one after the other in the background.
pub struct Inspector {
    tx: mpsc::Sender<(u64, Found)>,
    out: Arc<Mutex<Vec<(u64, Insight)>>>,
    pending: Arc<AtomicUsize>,
    generation: Arc<AtomicU64>,
}

impl Inspector {
    pub fn start(ctx: egui::Context) -> Inspector {
        let (tx, rx) = mpsc::channel::<(u64, Found)>();
        let out = Arc::new(Mutex::new(Vec::new()));
        let pending = Arc::new(AtomicUsize::new(0));
        let generation = Arc::new(AtomicU64::new(0));
        let (o, p, g) = (out.clone(), pending.clone(), generation.clone());
        std::thread::spawn(move || {
            for (gen_of, f) in rx {
                // files of an earlier search are dropped unread
                if gen_of == g.load(Ordering::Relaxed) {
                    let ins = inspect(&f);
                    o.lock().unwrap().push((gen_of, ins));
                    ctx.request_repaint();
                }
                p.fetch_sub(1, Ordering::Relaxed);
            }
        });
        Inspector { tx, out, pending, generation }
    }

    pub fn send(&self, f: &Found) {
        self.pending.fetch_add(1, Ordering::Relaxed);
        if self.tx.send((self.generation.load(Ordering::Relaxed), f.clone())).is_err() {
            self.pending.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// A new search starts: what is still queued no longer matters.
    pub fn reset(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.out.lock().unwrap().clear();
    }

    pub fn take(&self) -> Vec<Insight> {
        let now = self.generation.load(Ordering::Relaxed);
        let got = std::mem::take(&mut *self.out.lock().unwrap());
        got.into_iter().filter(|(g, _)| *g == now).map(|(_, i)| i).collect()
    }

    pub fn busy(&self) -> bool {
        self.pending.load(Ordering::Relaxed) > 0
    }
}

// ---------------------------------------------------------------- reading

/// Where the pieces of a file too big to read whole can be read from.
enum Source<'a> {
    Bytes(&'a [u8]),
    Local(std::fs::File),
    Raw(String, u64),
}

impl Source<'_> {
    fn of(f: &Found) -> Option<Source<'static>> {
        match &f.origin {
            Origin::Recycled { data, is_dir: false } => std::fs::File::open(data).ok().map(Source::Local),
            Origin::Staged { path } => std::fs::File::open(path).ok().map(Source::Local),
            Origin::Raw { device, offset } => Some(Source::Raw(device.clone(), *offset)),
            _ => None,
        }
    }

    /// `n` bytes at `off`, or nothing when the file is shorter.
    fn at(&mut self, off: u64, n: usize) -> Option<Vec<u8>> {
        let mut buf = Vec::with_capacity(n);
        match self {
            Source::Bytes(d) => buf.extend_from_slice(d.get(off as usize..(off as usize).checked_add(n)?)?),
            Source::Local(file) => {
                file.seek(SeekFrom::Start(off)).ok()?;
                file.by_ref().take(n as u64).read_to_end(&mut buf).ok()?;
            }
            Source::Raw(device, base) => {
                read_raw(device, *base + off, n as u64, |b| {
                    buf.extend_from_slice(b);
                    true
                })
                .ok()?;
            }
        }
        (buf.len() == n).then_some(buf)
    }
}

/// The start of the file (all of it when it is not too big).
fn content(f: &Found) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    match &f.origin {
        Origin::Recycled { is_dir: true, .. } => return None,
        Origin::Recycled { data: path, .. } | Origin::Staged { path } => {
            std::fs::File::open(path).ok()?.take(CAP).read_to_end(&mut buf).ok()?;
        }
        _ => {
            read_found(f, CAP, |b| {
                buf.extend_from_slice(b);
                true
            })
            .ok()?;
        }
    }
    Some(buf)
}

/// Files rebuilt from raw bytes only have a number: "photo_0012.jpg".
pub fn has_generic_name(f: &Found) -> bool {
    if !matches!(f.origin, Origin::Raw { .. } | Origin::Staged { .. }) {
        return false;
    }
    let Some(rest) = f.name.strip_prefix(f.category.stem()).and_then(|r| r.strip_prefix('_')) else { return false };
    let number = rest.strip_suffix(&format!(".{}", f.ext)).unwrap_or(rest);
    number.len() >= 4 && number.bytes().all(|b| b.is_ascii_digit())
}

pub fn inspect(f: &Found) -> Insight {
    let mut ins = Insight { id: f.id, name: None, stamp: 0, detail: None, verdict: Verdict::Unknown, digest: None };
    if f.ext == "dossier" || f.size == 0 {
        return ins;
    }
    let Some(data) = content(f) else { return ins };
    if data.is_empty() {
        return ins;
    }
    let whole = data.len() as u64 >= f.size;
    let mut h = std::hash::DefaultHasher::new();
    h.write_u64(f.size);
    h.write(&data);
    ins.digest = Some(h.finish());
    if data.iter().all(|&b| b == 0) {
        ins.verdict = Verdict::Empty;
        return ins;
    }
    let ext = f.ext.to_ascii_lowercase();
    let meta = read_meta(&ext, &data);
    ins.verdict = if whole { check(&ext, &data, f.size) } else { check_big(&ext, f) };
    ins.stamp = meta.when;
    ins.detail = describe(&meta);
    if has_generic_name(f) {
        ins.name = better_name(f, &meta);
    }
    ins
}

fn describe(m: &Meta) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(c) = &m.camera {
        parts.push(c.clone());
    }
    if let Some((w, h)) = m.dims {
        parts.push(format!("{w} × {h}"));
    }
    if let Some(s) = m.seconds {
        parts.push(if s >= 60 { format!("{} min {:02} s", s / 60, s % 60) } else { format!("{s} s") });
    }
    match (&m.artist, &m.title) {
        (Some(a), Some(t)) => parts.push(format!("{a} – {t}")),
        (None, Some(t)) => parts.push(format!("« {t} »")),
        _ => {}
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn better_name(f: &Found, m: &Meta) -> Option<String> {
    let base = match f.category {
        Category::Photos | Category::Videos if m.when > 0 => format!("{}_{}", f.category.stem(), stamp_name(m.when)),
        Category::Music => match (&m.artist, &m.title) {
            (Some(a), Some(t)) => format!("{a} - {t}"),
            (None, Some(t)) => t.clone(),
            _ => return None,
        },
        Category::Documents | Category::Archives => m.title.clone()?,
        _ => return None,
    };
    let base = clean_name(&base);
    (!base.is_empty()).then(|| format!("{base}.{}", f.ext))
}

/// Keeps a name usable on every system.
fn clean_name(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { ' ' } else { c }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s: String = s.chars().take(80).collect();
    s.trim_matches(|c: char| c == '.' || c == ' ').to_string()
}

// ---------------------------------------------------------------- dates

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "2023-05-12_14-30-22"
fn stamp_name(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}_{:02}-{:02}-{:02}", rest / 3600, rest / 60 % 60, rest % 60)
}

/// The first 14 digits of "2023:05:12 14:30:22", "2023-05-12T14:30:22Z" or "D:20230512143022".
pub(crate) fn parse_when(s: &str) -> u64 {
    let digits: Vec<i64> = s.bytes().filter(u8::is_ascii_digit).take(14).map(|b| (b - b'0') as i64).collect();
    if digits.len() < 8 {
        return 0;
    }
    let num = |a: usize, n: usize| digits.get(a..a + n).map(|d| d.iter().fold(0, |acc, x| acc * 10 + x));
    let (Some(y), Some(mo), Some(d)) = (num(0, 4), num(4, 2), num(6, 2)) else { return 0 };
    if !(1980..=2100).contains(&y) || !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return 0;
    }
    let (h, mi, se) = (num(8, 2).unwrap_or(0), num(10, 2).unwrap_or(0), num(12, 2).unwrap_or(0));
    if h > 23 || mi > 59 || se > 60 {
        return 0;
    }
    (days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se) as u64
}

// ---------------------------------------------------------------- what files say about themselves

fn be16(b: &[u8], at: usize) -> Option<u32> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?) as u32)
}
fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn be64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}
fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn read_meta(ext: &str, d: &[u8]) -> Meta {
    let mut m = Meta::default();
    match ext {
        "jpg" | "jpeg" => jpeg_meta(d, &mut m),
        "tif" | "tiff" | "dng" | "nef" | "cr2" | "arw" | "orf" | "rw2" => tiff_meta(d, &mut m),
        "png" => png_meta(d, &mut m),
        "mp4" | "mov" | "m4v" | "m4a" | "3gp" => mp4_meta(d, &mut m),
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" | "epub" => office_meta(d, &mut m),
        "pdf" => pdf_meta(d, &mut m),
        "mp3" => id3_meta(d, &mut m),
        _ => {}
    }
    m
}

fn jpeg_meta(d: &[u8], m: &mut Meta) {
    let mut pos = 2;
    while pos + 4 <= d.len() && d[pos] == 0xFF {
        let mk = d[pos + 1];
        if mk == 0xFF {
            pos += 1;
            continue;
        }
        if mk == 0xD9 || mk == 0xDA {
            break;
        }
        let Some(len) = be16(d, pos + 2).map(|l| l as usize) else { break };
        let seg = d.get(pos + 4..(pos + 2 + len).min(d.len())).unwrap_or(&[]);
        if mk == 0xE1 && seg.starts_with(b"Exif\0\0") {
            tiff_meta(&seg[6..], m);
        }
        // the picture's size, from the frame header
        if matches!(mk, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF)
            && let (Some(h), Some(w)) = (be16(seg, 1), be16(seg, 3))
            && w > 0
            && h > 0
        {
            m.dims = Some((w, h));
        }
        pos += 2 + len;
    }
}

/// EXIF: camera make and model, when the picture was taken.
fn tiff_meta(t: &[u8], m: &mut Meta) {
    let le = match t.get(..4) {
        Some(b"II*\0") => true,
        Some(b"MM\0*") => false,
        _ => return,
    };
    let u16_at = |at: usize| -> Option<u32> {
        let b: [u8; 2] = t.get(at..at + 2)?.try_into().ok()?;
        Some(if le { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) } as u32)
    };
    let u32_at = |at: usize| -> Option<u32> {
        let b: [u8; 4] = t.get(at..at + 4)?.try_into().ok()?;
        Some(if le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
    };
    // (tag, type, count, where the value is)
    let entries = |ifd: usize| -> Vec<(u32, u32, u32, usize)> {
        let Some(n) = u16_at(ifd) else { return Vec::new() };
        (0..n.min(200) as usize)
            .filter_map(|i| {
                let e = ifd + 2 + i * 12;
                let (tag, typ, count) = (u16_at(e)?, u16_at(e + 2)?, u32_at(e + 4)?);
                let size = match typ {
                    3 => 2,
                    4 => 4,
                    _ => 1,
                } * count as usize;
                let at = if size <= 4 { e + 8 } else { u32_at(e + 8)? as usize };
                Some((tag, typ, count, at))
            })
            .collect()
    };
    let text = |at: usize, count: u32| -> Option<String> {
        let raw = t.get(at..at + count as usize)?;
        let s = String::from_utf8_lossy(raw).trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string();
        (!s.is_empty()).then_some(s)
    };
    let number = |typ: u32, at: usize| if typ == 3 { u16_at(at) } else { u32_at(at) };
    let Some(ifd0) = u32_at(4) else { return };
    let (mut make, mut model, mut changed, mut taken) = (None, None, 0, 0);
    let (mut w, mut h) = (None, None);
    for (tag, _, count, at) in entries(ifd0 as usize) {
        match tag {
            0x010F => make = text(at, count),
            0x0110 => model = text(at, count),
            0x0132 => changed = text(at, count).map_or(0, |s| parse_when(&s)),
            0x8769 => {
                if let Some(sub) = u32_at(at) {
                    for (tag, typ, count, at) in entries(sub as usize) {
                        match tag {
                            0x9003 => taken = text(at, count).map_or(0, |s| parse_when(&s)),
                            0xA002 => w = number(typ, at),
                            0xA003 => h = number(typ, at),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let when = if taken > 0 { taken } else { changed };
    if when > 0 {
        m.when = when;
    }
    m.camera = match (make, model) {
        // "Canon" + "Canon EOS 80D": the model often repeats the make
        (Some(a), Some(b)) if b.to_lowercase().starts_with(&a.to_lowercase().split(' ').next().unwrap_or("").to_string()) => Some(b),
        (Some(a), Some(b)) => Some(format!("{a} {b}")),
        (a, b) => a.or(b),
    };
    if let (Some(w), Some(h)) = (w, h)
        && w > 0
        && h > 0
        && m.dims.is_none()
    {
        m.dims = Some((w, h));
    }
}

fn png_meta(d: &[u8], m: &mut Meta) {
    if let (Some(w), Some(h)) = (be32(d, 16), be32(d, 20)) {
        m.dims = Some((w, h));
    }
    let mut pos = 8;
    while let (Some(len), Some(typ)) = (be32(d, pos).map(|l| l as usize), d.get(pos + 4..pos + 8)) {
        if typ == b"eXIf"
            && let Some(body) = d.get(pos + 8..pos + 8 + len)
        {
            let dims = m.dims;
            tiff_meta(body, m);
            m.dims = dims;
        }
        if typ == b"IEND" {
            break;
        }
        pos += 12 + len;
    }
}

/// Videos: when they were filmed and how long they last, from the movie header.
fn mp4_meta(d: &[u8], m: &mut Meta) {
    let Some(moov) = atom(d, 0, d.len(), b"moov") else { return };
    let Some(mvhd) = atom(d, moov.0, moov.1, b"mvhd") else { return };
    let b = &d[mvhd.0..mvhd.1];
    let (created, scale, dur) = if b.first() == Some(&1) {
        (be64(b, 4), be32(b, 20), be64(b, 24))
    } else {
        (be32(b, 4).map(u64::from), be32(b, 12), be32(b, 16).map(u64::from))
    };
    // seconds since 1904
    if let Some(c) = created.filter(|&c| c > 2_082_844_800) {
        let secs = c - 2_082_844_800;
        if parse_when("19800101") <= secs {
            m.when = secs;
        }
    }
    if let (Some(s), Some(d)) = (scale.filter(|&s| s > 0), dur) {
        m.seconds = Some(d / s as u64);
    }
}

/// The body (start, end) of the first `name` box between `from` and `to`.
fn atom(d: &[u8], from: usize, to: usize, name: &[u8; 4]) -> Option<(usize, usize)> {
    let mut pos = from;
    while pos + 8 <= to {
        let size = be32(d, pos)? as usize;
        let (head, size) = match size {
            1 => (16, be64(d, pos + 8)? as usize),
            0 => (8, to - pos),
            s => (8, s),
        };
        if size < head {
            return None;
        }
        if &d[pos + 4..pos + 8] == name {
            return Some((pos + head, (pos + size).min(to)));
        }
        pos = pos.checked_add(size)?;
    }
    None
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// The text inside the first `<tag …>…</tag>`.
fn xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = xml.find(&format!("<{tag}"))?;
    let body = open + xml[open..].find('>')? + 1;
    if xml[..body].ends_with("/>") {
        return None;
    }
    let end = body + xml[body..].find(&format!("</{tag}>"))?;
    let s = xml_unescape(xml[body..end].trim());
    (!s.is_empty()).then_some(s)
}

/// Word, Excel, PowerPoint and LibreOffice files: their title and when they were written.
fn office_meta(d: &[u8], m: &mut Meta) {
    let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(d)) else { return };
    let mut read = |name: &str| -> Option<String> {
        let mut s = String::new();
        zip.by_name(name).ok()?.take(1 << 20).read_to_string(&mut s).ok()?;
        Some(s)
    };
    if let Some(core) = read("docProps/core.xml") {
        m.title = xml_tag(&core, "dc:title");
        m.artist = None;
        m.when = xml_tag(&core, "dcterms:created").map_or(0, |s| parse_when(&s));
    } else if let Some(meta) = read("meta.xml") {
        m.title = xml_tag(&meta, "dc:title");
        m.when = xml_tag(&meta, "meta:creation-date").map_or(0, |s| parse_when(&s));
    } else if let Some(opf) = read("OEBPS/content.opf").or_else(|| read("content.opf")) {
        m.title = xml_tag(&opf, "dc:title");
        m.artist = xml_tag(&opf, "dc:creator");
    }
}

/// A PDF string: "(…)" with its escapes, or "<FEFF…>" in hex, maybe in UTF-16.
fn pdf_string(d: &[u8]) -> Option<String> {
    let start = d.iter().position(|&b| !b.is_ascii_whitespace())?;
    let mut bytes = Vec::new();
    match d[start] {
        b'(' => {
            let mut depth = 0;
            let mut i = start + 1;
            while i < d.len() && bytes.len() < 1000 {
                let c = d[i];
                match c {
                    b'\\' => {
                        i += 1;
                        let e = *d.get(i)?;
                        match e {
                            b'n' => bytes.push(b'\n'),
                            b'r' => bytes.push(b'\r'),
                            b't' => bytes.push(b'\t'),
                            b'0'..=b'7' => {
                                let mut v = 0u32;
                                let mut k = 0;
                                while k < 3 && d.get(i + k).is_some_and(|x| (b'0'..=b'7').contains(x)) {
                                    v = v * 8 + (d[i + k] - b'0') as u32;
                                    k += 1;
                                }
                                i += k - 1;
                                bytes.push(v as u8);
                            }
                            other => bytes.push(other),
                        }
                    }
                    b'(' => {
                        depth += 1;
                        bytes.push(c);
                    }
                    b')' if depth == 0 => break,
                    b')' => {
                        depth -= 1;
                        bytes.push(c);
                    }
                    _ => bytes.push(c),
                }
                i += 1;
            }
        }
        b'<' => {
            let end = start + d[start..].iter().position(|&b| b == b'>')?;
            let hex: Vec<u8> = d[start + 1..end].iter().copied().filter(u8::is_ascii_hexdigit).collect();
            for pair in hex.chunks(2) {
                let s = std::str::from_utf8(pair).ok()?;
                bytes.push(u8::from_str_radix(&format!("{s:0<2}"), 16).ok()?);
            }
        }
        _ => return None,
    }
    let s = if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..].as_chunks::<2>().0.iter().map(|&c| u16::from_be_bytes(c)).collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    };
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn pdf_meta(d: &[u8], m: &mut Meta) {
    if let Some(i) = find(d, b"/Title") {
        m.title = pdf_string(&d[i + 6..]);
    }
    if let Some(i) = find(d, b"/CreationDate") {
        m.when = pdf_string(&d[i + 13..]).map_or(0, |s| parse_when(&s));
    }
}

/// MP3: title and artist from the ID3 tag.
fn id3_meta(d: &[u8], m: &mut Meta) {
    if !d.starts_with(b"ID3") || d.len() < 10 {
        return;
    }
    let ver = d[3];
    let syncsafe = |b: &[u8]| b.iter().fold(0usize, |acc, &x| (acc << 7) | (x & 0x7F) as usize);
    let end = (10 + syncsafe(&d[6..10])).min(d.len());
    let mut pos = 10;
    while pos + 10 <= end {
        let id = &d[pos..pos + 4];
        if id[0] == 0 {
            break;
        }
        let size = if ver >= 4 { syncsafe(&d[pos + 4..pos + 8]) } else { be32(d, pos + 4).unwrap_or(0) as usize };
        let Some(body) = d.get(pos + 10..pos + 10 + size) else { break };
        match id {
            b"TIT2" => m.title = id3_text(body),
            b"TPE1" => m.artist = id3_text(body),
            _ => {}
        }
        pos += 10 + size;
    }
}

fn id3_text(b: &[u8]) -> Option<String> {
    let (&enc, rest) = b.split_first()?;
    let s = match enc {
        1 | 2 => {
            let (be, rest) = match rest {
                [0xFE, 0xFF, r @ ..] => (true, r),
                [0xFF, 0xFE, r @ ..] => (false, r),
                r => (enc == 2, r),
            };
            let units: Vec<u16> =
                rest.as_chunks::<2>().0.iter().map(|&c| if be { u16::from_be_bytes(c) } else { u16::from_le_bytes(c) }).collect();
            String::from_utf16_lossy(&units)
        }
        3 => String::from_utf8_lossy(rest).into_owned(),
        _ => rest.iter().map(|&b| b as char).collect(),
    };
    let s = s.trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string();
    (!s.is_empty()).then_some(s)
}

// ---------------------------------------------------------------- is it whole?

fn check(ext: &str, d: &[u8], size: u64) -> Verdict {
    match ext {
        "jpg" | "jpeg" => check_jpeg(d),
        "png" => {
            if find(&d[d.len().saturating_sub(64)..], b"IEND").is_none() {
                return Verdict::Damaged(t!("la fin de l'image manque"));
            }
            decodes(d, image::ImageFormat::Png)
        }
        "gif" => decodes(d, image::ImageFormat::Gif),
        "bmp" => decodes(d, image::ImageFormat::Bmp),
        "webp" => decodes(d, image::ImageFormat::WebP),
        "zip" | "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" | "epub" | "jar" | "apk" => check_zip(d),
        "pdf" => {
            let end = d.iter().rposition(|b| !b.is_ascii_whitespace() && *b != 0).map_or(0, |p| p + 1);
            let tail = &d[end.saturating_sub(1024)..end];
            if find(tail, b"%%EOF").is_some() && find(&d[end.saturating_sub(64 << 10)..end], b"startxref").is_some() {
                Verdict::Intact
            } else {
                Verdict::Damaged(t!("la fin du document manque"))
            }
        }
        "mp4" | "mov" | "m4v" | "m4a" | "3gp" => check_mp4(&mut Source::Bytes(d), size),
        "wav" | "avi" | "webm" => check_riff(d, size),
        _ => Verdict::Unknown,
    }
}

/// Files bigger than what is read: only what can be told from a few headers.
fn check_big(ext: &str, f: &Found) -> Verdict {
    match ext {
        "mp4" | "mov" | "m4v" | "m4a" | "3gp" => Source::of(f).map_or(Verdict::Unknown, |mut r| check_mp4(&mut r, f.size)),
        _ => Verdict::Unknown,
    }
}

fn decodes(d: &[u8], format: image::ImageFormat) -> Verdict {
    match image::load_from_memory_with_format(d, format) {
        Ok(_) => Verdict::Intact,
        Err(_) => Verdict::Damaged(t!("l'image ne se décode pas")),
    }
}

fn check_jpeg(d: &[u8]) -> Verdict {
    if !d.ends_with(&[0xFF, 0xD9]) {
        return Verdict::Damaged(t!("la fin de l'image manque"));
    }
    // after the picture data starts, a new "start of image" means two files run into each other
    let mut pos = 2;
    let mut in_scan = false;
    while pos + 1 < d.len() {
        if in_scan {
            match d[pos..].iter().position(|&b| b == 0xFF) {
                Some(i) => pos += i,
                None => break,
            }
            match d.get(pos + 1) {
                Some(0x00 | 0xFF | 0xD0..=0xD7) => {
                    pos += 2;
                    continue;
                }
                Some(0xD8) => return Verdict::Damaged(t!("l'image est mélangée avec une autre")),
                Some(0xD9) => break,
                _ => in_scan = false,
            }
        }
        if d[pos] != 0xFF {
            return Verdict::Damaged(t!("l'image ne se décode pas"));
        }
        let mk = d[pos + 1];
        match mk {
            0xFF => pos += 1,
            0xD8 => return Verdict::Damaged(t!("l'image est mélangée avec une autre")),
            0x01 | 0xD0..=0xD7 => pos += 2,
            0xD9 => break,
            _ => {
                let Some(len) = be16(d, pos + 2) else { break };
                pos += 2 + len as usize;
                in_scan = mk == 0xDA;
            }
        }
    }
    decodes(d, image::ImageFormat::Jpeg)
}

fn check_zip(d: &[u8]) -> Verdict {
    let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(d)) else {
        return Verdict::Damaged(t!("le sommaire de l'archive manque"));
    };
    let mut left = UNPACK_CAP;
    for i in 0..zip.len() {
        let Ok(entry) = zip.by_index(i) else { return Verdict::Damaged(t!("une partie du contenu est abîmée")) };
        // reading to the end checks each piece against its CRC
        match std::io::copy(&mut entry.take(left), &mut std::io::sink()) {
            Ok(n) => left = left.saturating_sub(n),
            Err(_) => return Verdict::Damaged(t!("une partie du contenu est abîmée")),
        }
        if left == 0 {
            return Verdict::Unknown;
        }
    }
    Verdict::Intact
}

fn check_riff(d: &[u8], size: u64) -> Verdict {
    match le32(d, 4) {
        Some(n) if (d.starts_with(b"RIFF") || d.starts_with(b"RF64")) && n as u64 + 8 > size + 1 => {
            Verdict::Damaged(t!("le fichier est coupé avant la fin"))
        }
        Some(_) if d.starts_with(b"RIFF") => Verdict::Intact,
        _ => Verdict::Unknown,
    }
}

/// A video is whole when its boxes fill the file exactly and the index ("moov") is there.
fn check_mp4(r: &mut Source, size: u64) -> Verdict {
    let (mut pos, mut moov, mut mdat, mut boxes) = (0u64, false, false, 0);
    while pos + 8 <= size {
        let Some(h) = r.at(pos, 16.min((size - pos) as usize)) else { return Verdict::Unknown };
        let len = match be32(&h, 0) {
            Some(1) => match be64(&h, 8) {
                Some(l) => l,
                None => return Verdict::Unknown,
            },
            Some(0) => size - pos,
            Some(l) => l as u64,
            None => return Verdict::Unknown,
        };
        if len < 8 || !h[4..8].iter().all(|b| b.is_ascii_graphic()) {
            return if boxes == 0 { Verdict::Unknown } else { Verdict::Damaged(t!("le fichier est coupé avant la fin")) };
        }
        moov |= &h[4..8] == b"moov";
        mdat |= &h[4..8] == b"mdat";
        boxes += 1;
        if pos + len > size {
            return Verdict::Damaged(t!("le fichier est coupé avant la fin"));
        }
        pos += len;
    }
    if !moov {
        Verdict::Damaged(t!("l'index de la vidéo manque"))
    } else if !mdat {
        Verdict::Damaged(t!("les images de la vidéo manquent"))
    } else {
        Verdict::Intact
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(stamp_name(parse_when("2023:05:12 14:30:22")), "2023-05-12_14-30-22");
        assert_eq!(stamp_name(parse_when("2021-01-31T08:05:00Z")), "2021-01-31_08-05-00");
        assert_eq!(stamp_name(parse_when("D:20200229235959+01'00'")), "2020-02-29_23-59-59");
        assert_eq!(parse_when("0000:00:00 00:00:00"), 0);
        assert_eq!(parse_when("garbage"), 0);
    }

    #[test]
    fn exif_in_jpeg() {
        let d = crate::bench::jpeg_with_exif(64, 48, 7, "Canon", "Canon EOS 80D", "2023:05:12 14:30:22");
        let m = read_meta("jpg", &d);
        assert_eq!(m.camera.as_deref(), Some("Canon EOS 80D"));
        assert_eq!(stamp_name(m.when), "2023-05-12_14-30-22");
        assert_eq!(m.dims, Some((64, 48)));
        assert_eq!(check("jpg", &d, d.len() as u64), Verdict::Intact);
        // cut short, or run into the next picture
        let cut = &d[..d.len() * 2 / 3];
        assert!(matches!(check("jpg", cut, cut.len() as u64), Verdict::Damaged(_)));
        let mut mixed = d[..d.len() - 300].to_vec();
        mixed.extend_from_slice(&d);
        assert!(matches!(check("jpg", &mixed, mixed.len() as u64), Verdict::Damaged(_)));
    }

    #[test]
    fn office_pdf_and_mp3() {
        let docx = crate::bench::docx("Budget vacances 2024", "2024-03-02T10:00:00Z");
        let m = read_meta("docx", &docx);
        assert_eq!(m.title.as_deref(), Some("Budget vacances 2024"));
        assert_eq!(stamp_name(m.when), "2024-03-02_10-00-00");
        assert_eq!(check("docx", &docx, docx.len() as u64), Verdict::Intact);
        let mut broken = docx.clone();
        let mid = broken.len() / 3;
        broken[mid] ^= 0xFF;
        assert!(matches!(check("docx", &broken, broken.len() as u64), Verdict::Damaged(_)));

        let pdf = crate::bench::pdf_titled("Contrat de location", "D:20220915120000");
        let m = read_meta("pdf", &pdf);
        assert_eq!(m.title.as_deref(), Some("Contrat de location"));
        assert_eq!(check("pdf", &pdf, pdf.len() as u64), Verdict::Intact);
        assert!(matches!(check("pdf", &pdf[..pdf.len() / 2], (pdf.len() / 2) as u64), Verdict::Damaged(_)));

        let mp3 = crate::bench::mp3("Nina Simone", "Feeling Good");
        let m = read_meta("mp3", &mp3);
        assert_eq!((m.artist.as_deref(), m.title.as_deref()), (Some("Nina Simone"), Some("Feeling Good")));
    }

    #[test]
    fn videos() {
        let good = crate::bench::mp4(true, "2022-07-14T21:00:00Z", 95);
        let m = read_meta("mp4", &good);
        assert_eq!(stamp_name(m.when), "2022-07-14_21-00-00");
        assert_eq!(m.seconds, Some(95));
        assert_eq!(check("mp4", &good, good.len() as u64), Verdict::Intact);
        let no_index = crate::bench::mp4(false, "2022-07-14T21:00:00Z", 95);
        assert!(matches!(check("mp4", &no_index, no_index.len() as u64), Verdict::Damaged(_)));
        assert!(matches!(check("mp4", &good[..good.len() - 10], good.len() as u64 - 10), Verdict::Damaged(_)));
    }

    #[test]
    fn generic_names() {
        let f = |name: &str, origin| Found {
            id: 0,
            name: name.into(),
            ext: "jpg".into(),
            category: Category::Photos,
            size: 1,
            place: String::new(),
            date: None,
            stamp: 0,
            chance: crate::engine::Chance::Good,
            origin,
            checked: false,
        };
        let raw = || Origin::Raw { device: String::new(), offset: 0 };
        assert!(has_generic_name(&f(&format!("{}_0012.jpg", Category::Photos.stem()), raw())));
        assert!(!has_generic_name(&f("vacances.jpg", raw())));
        assert!(!has_generic_name(&f(&format!("{}_0012.jpg", Category::Photos.stem()), Origin::Bytes(Arc::from(vec![])))));
        assert_eq!(clean_name(" Mon/rapport: final? "), "Mon rapport final");
    }
}
