//! The quick search: reads the NTFS file table, where a deleted file keeps its name,
//! its folder and the place of its contents until the entry is reused. A few minutes
//! instead of reading the whole drive, and the files come back with their real names.

use crate::carve::{Source, Volume};
use crate::engine::{Category, Chance, Eta, Found, Origin, ScanShared, Wanted};
use crate::sys::{self, Disk};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub fn is_ntfs(disk: &Disk) -> bool {
    disk.fs.eq_ignore_ascii_case("NTFS")
}

/// Folders that only hold the system's own leftovers: nobody looks for those.
const NOISE: &[&str] = &[
    "\\windows\\",
    "\\windows.old\\",
    "\\programdata\\",
    "\\program files",
    "\\appdata\\local\\",
    "\\$extend",
    "\\system volume information",
    "\\$winreagent",
];

fn u16le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn u64le(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

struct Boot {
    cluster: u64,
    record: usize,
    mft_lcn: u64,
}

fn boot(b: &[u8]) -> Option<Boot> {
    if b.len() < 512 || &b[3..11] != b"NTFS    " {
        return None;
    }
    let sector = u16le(b, 0x0B) as u64;
    let spc = b[0x0D] as u64;
    // above 0x80 the value is a power of two written backwards (very large clusters)
    let spc = if spc > 0x80 { 1u64 << (256 - spc) } else { spc };
    let cluster = sector * spc;
    let c = b[0x40] as i8;
    let record = if c < 0 { 1usize << (-(c as i32)) } else { c as usize * cluster as usize };
    if sector < 512 || cluster == 0 || !(256..=65536).contains(&record) {
        return None;
    }
    Some(Boot { cluster, record, mft_lcn: u64le(b, 0x30) })
}

/// Puts back the two bytes NTFS swaps at the end of every 512-byte block of a record.
fn fixup(rec: &mut [u8]) -> bool {
    if rec.len() < 48 || &rec[0..4] != b"FILE" {
        return false;
    }
    let (off, n) = (u16le(rec, 4) as usize, u16le(rec, 6) as usize);
    if n < 2 || off + 2 * n > rec.len() || (n - 1) * 512 > rec.len() {
        return false;
    }
    for i in 1..n {
        let end = i * 512 - 2;
        if rec[end..end + 2] != rec[off..off + 2] {
            return false; // torn write: the record is not trustworthy
        }
        rec[end] = rec[off + 2 * i];
        rec[end + 1] = rec[off + 2 * i + 1];
    }
    true
}

/// The attributes of a record, each as its own slice.
fn attrs(rec: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut p = u16le(rec, 0x14) as usize;
    while p + 16 <= rec.len() {
        let kind = u32le(rec, p);
        let len = u32le(rec, p + 4) as usize;
        if kind == 0xFFFF_FFFF || len < 16 || p + len > rec.len() {
            break;
        }
        out.push(&rec[p..p + len]);
        p += len;
    }
    out
}

fn resident_body(a: &[u8]) -> Option<&[u8]> {
    if a.len() < 0x18 || a[8] != 0 {
        return None;
    }
    let (len, off) = (u32le(a, 0x10) as usize, u16le(a, 0x14) as usize);
    a.get(off..off + len)
}

/// Where the pieces of a non-resident attribute lie, in bytes: (offset or hole, length).
fn runs(a: &[u8], cluster: u64) -> Option<Vec<(Option<u64>, u64)>> {
    if a.len() < 0x40 || a[8] == 0 {
        return None;
    }
    let mut p = u16le(a, 0x20) as usize;
    let mut lcn: i64 = 0;
    let mut out = Vec::new();
    while p < a.len() && a[p] != 0 {
        let h = a[p];
        let (nl, no) = ((h & 0x0F) as usize, (h >> 4) as usize);
        if nl == 0 || nl > 8 || no > 8 || p + 1 + nl + no > a.len() {
            return None;
        }
        let mut len = 0u64;
        for i in 0..nl {
            len |= (a[p + 1 + i] as u64) << (8 * i);
        }
        if no == 0 {
            out.push((None, len * cluster));
        } else {
            let mut d = 0i64;
            for i in 0..no {
                d |= (a[p + 1 + nl + i] as i64) << (8 * i);
            }
            let shift = 64 - 8 * no as u32;
            if shift > 0 {
                d = (d << shift) >> shift; // the jump can go backwards
            }
            lcn += d;
            if lcn < 0 {
                return None;
            }
            out.push((Some(lcn as u64 * cluster), len * cluster));
        }
        p += 1 + nl + no;
    }
    Some(out)
}

enum Data {
    Inside(Vec<u8>),
    Pieces(Vec<(Option<u64>, u64)>),
}

struct Lost {
    name: String,
    parent: u64,
    modified: u64,
    size: u64,
    data: Data,
}

/// What one record says: the best of its names, its parent folder and its date.
fn file_name(rec: &[u8]) -> Option<(String, u64, u64)> {
    let mut best: Option<(String, u64, u64, u8)> = None;
    for a in attrs(rec) {
        if u32le(a, 0) != 0x30 {
            continue;
        }
        let Some(b) = resident_body(a) else { continue };
        if b.len() < 0x42 {
            continue;
        }
        let n = b[0x40] as usize;
        let space = b[0x41];
        let Some(raw) = b.get(0x42..0x42 + 2 * n) else { continue };
        let units: Vec<u16> = raw.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let name = String::from_utf16_lossy(&units);
        // the short "PHOTO~1.JPG" name (namespace 2) only if nothing better
        if best.as_ref().is_none_or(|x| x.3 == 2 && space != 2) {
            best = Some((name, u64le(b, 0) & 0x0000_FFFF_FFFF_FFFF, u64le(b, 0x10), space));
        }
    }
    best.map(|(n, p, t, _)| (n, p, t))
}

fn unnamed_data(rec: &[u8], cluster: u64) -> Option<(u64, Data)> {
    for a in attrs(rec) {
        if u32le(a, 0) != 0x80 || a[9] != 0 {
            continue;
        }
        if a[8] == 0 {
            let b = resident_body(a)?;
            return Some((b.len() as u64, Data::Inside(b.to_vec())));
        }
        // only the first extent lives here; a file split over several records is left to the full search
        if a.len() < 0x40 || u64le(a, 0x10) != 0 {
            return None;
        }
        let size = u64le(a, 0x30);
        let pieces = runs(a, cluster)?;
        let covered: u64 = pieces.iter().map(|p| p.1).sum();
        return (covered >= size).then_some((size, Data::Pieces(pieces)));
    }
    None
}

fn filetime_secs(ft: u64) -> u64 {
    (ft / 10_000_000).saturating_sub(11_644_473_600)
}

/// The original place and name kept in a "$I…" file of the Recycle Bin.
fn bin_info(b: &[u8]) -> Option<String> {
    if b.len() < 28 {
        return None;
    }
    let units: Vec<u16> = match u64le(b, 0) {
        1 => b.get(24..24 + 520)?.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).collect(),
        2 => {
            let n = u32le(b, 24) as usize;
            b.get(28..28 + 2 * n)?.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
        }
        _ => return None,
    };
    let s = String::from_utf16_lossy(&units);
    let s = s.trim_end_matches('\0').to_string();
    (!s.is_empty()).then_some(s)
}

pub fn scan(
    disk: &Disk,
    wanted: &Wanted,
    s: &Mutex<ScanShared>,
    cancel: &AtomicBool,
    next_id: &mut usize,
    ctx: &eframe::egui::Context,
) -> Result<(), String> {
    let file = std::fs::File::open(&disk.device).map_err(|e| t!("lecture du disque refusée ({})", e))?;
    let bitmap = sys::volume_bitmap(&file);
    let mut vol = Volume { inner: file, size: u64::MAX / 2 };
    let root = disk.root.to_string_lossy().trim_end_matches(['\\', '/']).to_string();
    scan_volume(&mut vol, &disk.device, &root, bitmap.as_deref(), wanted, s, cancel, next_id, ctx)
}

#[allow(clippy::too_many_arguments)]
pub fn scan_volume(
    vol: &mut impl Source,
    device: &str,
    root: &str,
    bitmap: Option<&[u8]>,
    wanted: &Wanted,
    s: &Mutex<ScanShared>,
    cancel: &AtomicBool,
    next_id: &mut usize,
    ctx: &eframe::egui::Context,
) -> Result<(), String> {
    let mut first = vec![0u8; 512];
    if vol.read_at(0, &mut first) < 512 {
        return Err(t!("début du disque illisible").into());
    }
    let b = boot(&first).ok_or(t!("ce n'est pas un disque NTFS"))?;
    let mut rec0 = vec![0u8; b.record];
    vol.read_at(b.mft_lcn * b.cluster, &mut rec0);
    if !fixup(&mut rec0) {
        return Err(t!("table des fichiers illisible").into());
    }
    let mft_runs = attrs(&rec0)
        .into_iter()
        .find(|a| u32le(a, 0) == 0x80 && a[9] == 0)
        .and_then(|a| runs(a, b.cluster))
        .ok_or(t!("table des fichiers illisible"))?;
    let total: u64 = mft_runs.iter().filter(|r| r.0.is_some()).map(|r| r.1).sum();
    {
        let mut g = s.lock().unwrap();
        g.phase = t!("Analyse rapide : lecture de la liste des fichiers…").into();
        g.progress = 0.0;
        g.note(t!(
            "Analyse rapide : table NTFS de {} ({} entrées, blocs de {} o)",
            crate::app::human_size(total),
            total / b.record as u64,
            b.cluster
        ));
    }

    // One pass over the table: every folder (to rebuild paths) and every deleted file.
    let mut dirs: HashMap<u64, (String, u64)> = HashMap::new();
    let mut lost: Vec<Lost> = Vec::new();
    let mut bin_names: HashMap<String, String> = HashMap::new();
    let mut recno = 0u64;
    let mut read = 0u64;
    let eta = Eta::new(0.0);
    let mut last = std::time::Instant::now();
    let mut carry: Vec<u8> = Vec::new();
    let chunk = (4usize << 20) / b.record * b.record;
    for &(at, len) in &mft_runs {
        let Some(at) = at else {
            recno += len / b.record as u64;
            continue;
        };
        let mut done = 0u64;
        while done < len {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            let want = (len - done).min(chunk as u64) as usize;
            let mut buf = vec![0u8; want];
            let got = vol.read_at(at + done, &mut buf);
            if got == 0 {
                break;
            }
            buf.truncate(got);
            done += got as u64;
            read += got as u64;
            carry.extend_from_slice(&buf);
            let whole = carry.len() / b.record * b.record;
            for rec in carry[..whole].chunks_exact_mut(b.record) {
                let n = recno;
                recno += 1;
                if !fixup(rec) {
                    continue;
                }
                let flags = u16le(rec, 0x16);
                let (in_use, is_dir) = (flags & 1 != 0, flags & 2 != 0);
                if u64le(rec, 0x20) != 0 {
                    continue; // an extension of another record
                }
                let Some((name, parent, modified)) = file_name(rec) else { continue };
                if is_dir {
                    dirs.insert(n, (name, parent));
                } else if !in_use && let Some((size, data)) = unnamed_data(rec, b.cluster) {
                    if let Data::Inside(bytes) = &data
                        && let Some(rest) = name.strip_prefix("$I")
                        && let Some(orig) = bin_info(bytes)
                    {
                        bin_names.insert(rest.to_lowercase(), orig);
                    }
                    if size > 0 {
                        lost.push(Lost { name, parent, modified, size, data });
                    }
                }
            }
            carry.drain(..whole);
            if last.elapsed().as_millis() > 200 {
                last = std::time::Instant::now();
                let p = (read as f32 / total.max(1) as f32).min(1.0) * 0.9;
                let mut g = s.lock().unwrap();
                g.progress = p;
                g.eta = eta.secs(p);
                drop(g);
                ctx.request_repaint();
            }
        }
    }
    s.lock().unwrap().note(t!("{} dossiers, {} fichiers effacés dans la table", dirs.len(), lost.len()));

    let path_of = |mut p: u64| -> String {
        let mut parts = Vec::new();
        for _ in 0..64 {
            if p == 5 {
                break; // the root folder
            }
            match dirs.get(&p) {
                Some((name, up)) => {
                    parts.push(name.as_str());
                    p = *up;
                }
                None => {
                    parts.push(t!("(dossier effacé)"));
                    break;
                }
            }
        }
        parts.reverse();
        let mut out = root.to_string();
        for part in parts {
            out.push(std::path::MAIN_SEPARATOR);
            out.push_str(part);
        }
        out
    };
    let in_use =
        |off: u64| bitmap.is_some_and(|bm| bm.get((off / b.cluster / 8) as usize).is_some_and(|x| x & (1 << ((off / b.cluster) % 8)) != 0));

    let (mut kept, mut noise, mut skipped) = (0usize, 0usize, 0usize);
    let mut batch = Vec::new();
    let count = lost.len().max(1);
    for (i, l) in lost.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let folder = path_of(l.parent);
        // a file emptied from the Recycle Bin: "$R…" holds the data, "$I…" its real name
        let (name, place) = match l.name.strip_prefix("$R").and_then(|r| bin_names.get(&r.to_lowercase())) {
            Some(orig) => {
                let (f, n) = orig.rsplit_once(['\\', '/']).unwrap_or(("", orig.as_str()));
                (n.to_string(), t!("Corbeille vidée · {}", f))
            }
            None => (l.name.clone(), folder.clone()),
        };
        if name.starts_with("$I") && folder.to_lowercase().contains("$recycle.bin") {
            continue;
        }
        let low = format!("{}\\", folder.to_lowercase().replace('/', "\\"));
        if NOISE.iter().any(|n| low.contains(n)) || name.starts_with('~') || name.ends_with(".tmp") {
            noise += 1;
            continue;
        }
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
        let cat = Category::from_ext(&ext);
        if !wanted.accepts(&ext, cat, l.size, false) {
            skipped += 1;
            continue;
        }
        let (origin, chance) = match l.data {
            Data::Inside(bytes) => (Origin::Bytes(Arc::from(bytes)), Chance::Excellent),
            Data::Pieces(pieces) => {
                let mut left = l.size;
                let mut reused = false;
                for &(at, n) in &pieces {
                    let n = n.min(left);
                    if let Some(at) = at {
                        reused |= (0..n.div_ceil(b.cluster)).any(|c| in_use(at + c * b.cluster));
                    }
                    left -= n;
                }
                // a drive that blanks freed space (SSD) reads back zeros
                let blank = pieces.iter().find_map(|p| p.0).is_some_and(|at| {
                    let mut head = vec![0u8; 4096.min(l.size as usize).max(512)];
                    let got = vol.read_at(at, &mut head);
                    got > 0 && head[..got].iter().all(|&x| x == 0)
                });
                let chance = if reused || blank { Chance::Low } else { Chance::Good };
                (Origin::Runs { device: device.to_string(), runs: pieces }, chance)
            }
        };
        let secs = filetime_secs(l.modified);
        batch.push(Found {
            id: *next_id,
            name,
            ext,
            category: cat,
            size: l.size,
            place,
            date: crate::engine::secs_to_date(secs),
            stamp: secs,
            chance,
            origin,
            checked: false,
        });
        *next_id += 1;
        kept += 1;
        if batch.len() >= 200 || i + 1 == count {
            let mut g = s.lock().unwrap();
            g.progress = 0.9 + 0.1 * (i + 1) as f32 / count as f32;
            for f in batch.drain(..) {
                g.add(f);
            }
            drop(g);
            ctx.request_repaint();
        }
    }
    let mut g = s.lock().unwrap();
    for f in batch.drain(..) {
        g.add(f);
    }
    g.note(t!("Analyse rapide : {} fichier(s) gardé(s), {} fichier(s) système ignoré(s), {} hors des critères", kept, noise, skipped));
    Ok(())
}

/// Pieces of an NTFS file table, written by hand: for the tests and the benchmark.
pub mod make {
    pub const REC: usize = 1024;

    pub fn record(flags: u16, attrs: &[Vec<u8>]) -> Vec<u8> {
        let mut r = vec![0u8; REC];
        r[0..4].copy_from_slice(b"FILE");
        r[4..6].copy_from_slice(&0x30u16.to_le_bytes());
        r[6..8].copy_from_slice(&3u16.to_le_bytes());
        r[0x14..0x16].copy_from_slice(&0x38u16.to_le_bytes());
        r[0x16..0x18].copy_from_slice(&flags.to_le_bytes());
        let mut p = 0x38;
        for a in attrs {
            r[p..p + a.len()].copy_from_slice(a);
            p += a.len();
        }
        r[p..p + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        // update sequence: number 0x0101, saved bytes from the end of each 512 block
        r[0x30..0x32].copy_from_slice(&[1, 1]);
        for i in 1..3 {
            let end = i * 512 - 2;
            r[0x30 + 2 * i] = r[end];
            r[0x30 + 2 * i + 1] = r[end + 1];
            r[end] = 1;
            r[end + 1] = 1;
        }
        r
    }
    pub fn resident(kind: u32, body: &[u8]) -> Vec<u8> {
        let len = (0x18 + body.len()).div_ceil(8) * 8;
        let mut a = vec![0u8; len];
        a[0..4].copy_from_slice(&kind.to_le_bytes());
        a[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        a[0x10..0x14].copy_from_slice(&(body.len() as u32).to_le_bytes());
        a[0x14..0x16].copy_from_slice(&0x18u16.to_le_bytes());
        a[0x18..0x18 + body.len()].copy_from_slice(body);
        a
    }
    pub fn name(parent: u64, n: &str) -> Vec<u8> {
        let units: Vec<u16> = n.encode_utf16().collect();
        let mut b = vec![0u8; 0x42 + 2 * units.len()];
        b[0..8].copy_from_slice(&(parent | (1 << 48)).to_le_bytes());
        b[0x10..0x18].copy_from_slice(&133_000_000_000_000_000u64.to_le_bytes());
        b[0x40] = units.len() as u8;
        b[0x41] = 1;
        for (i, u) in units.iter().enumerate() {
            b[0x42 + 2 * i..0x44 + 2 * i].copy_from_slice(&u.to_le_bytes());
        }
        resident(0x30, &b)
    }
    pub fn outside(size: u64, runs: &[u8]) -> Vec<u8> {
        let len = (0x40 + runs.len() + 1).div_ceil(8) * 8;
        let mut a = vec![0u8; len];
        a[0..4].copy_from_slice(&0x80u32.to_le_bytes());
        a[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        a[8] = 1;
        a[0x20..0x22].copy_from_slice(&0x40u16.to_le_bytes());
        a[0x30..0x38].copy_from_slice(&size.to_le_bytes());
        a[0x40..0x40 + runs.len()].copy_from_slice(runs);
        a
    }

    /// The run list of a file stored in pieces of (first cluster, cluster count).
    pub fn run_list(pieces: &[(u64, u64)]) -> Vec<u8> {
        fn bytes(v: i64, signed: bool) -> Vec<u8> {
            let mut b = v.to_le_bytes().to_vec();
            while b.len() > 1 {
                let last = b[b.len() - 1];
                let prev_top = b[b.len() - 2] & 0x80;
                let drop = if signed { (last == 0 && prev_top == 0) || (last == 0xFF && prev_top != 0) } else { last == 0 };
                if !drop {
                    break;
                }
                b.pop();
            }
            if !signed && b[b.len() - 1] & 0x80 != 0 {
                b.push(0);
            }
            b
        }
        let mut out = Vec::new();
        let mut prev = 0i64;
        for &(lcn, count) in pieces {
            let len = bytes(count as i64, false);
            let off = bytes(lcn as i64 - prev, true);
            prev = lcn as i64;
            out.push(((off.len() as u8) << 4) | len.len() as u8);
            out.extend(len);
            out.extend(off);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::make::*;
    use super::*;
    /// Builds a small NTFS volume by hand: a file table with the root, a "Photos" folder,
    /// a deleted photo stored outside the table, a deleted note stored inside, and a live file.
    fn image() -> Vec<u8> {
        const CL: usize = 4096;
        let mut img = vec![0u8; 64 * CL];
        // boot sector
        img[3..11].copy_from_slice(b"NTFS    ");
        img[0x0B..0x0D].copy_from_slice(&512u16.to_le_bytes());
        img[0x0D] = 8;
        img[0x30..0x38].copy_from_slice(&4u64.to_le_bytes()); // table at cluster 4
        img[0x40] = (-10i8) as u8; // records of 2^10 bytes

        let table = 4 * CL;
        let put = |img: &mut Vec<u8>, n: usize, r: Vec<u8>| img[table + n * REC..table + (n + 1) * REC].copy_from_slice(&r);
        // $MFT: 32 records = 8 clusters at cluster 4
        put(&mut img, 0, record(1, &[name(5, "$MFT"), outside(32 * REC as u64, &[0x11, 8, 4])]));
        // emptied Recycle Bin: "$I" tells the real name of "$R"
        put(&mut img, 14, record(3, &[name(5, "$Recycle.Bin")]));
        let path: Vec<u16> = "C:\\Users\\Mme\\Images\\chat.jpg".encode_utf16().collect();
        let mut info = vec![0u8; 28];
        info[0] = 2;
        info[24..28].copy_from_slice(&(path.len() as u32).to_le_bytes());
        info.extend(path.iter().flat_map(|u| u.to_le_bytes()));
        put(&mut img, 15, record(0, &[name(14, "$IABC12.jpg"), resident(0x80, &info)]));
        put(&mut img, 16, record(0, &[name(14, "$RABC12.jpg"), outside(100, &[0x11, 1, 40])]));
        img[40 * CL..41 * CL].fill(0xCC);
        put(&mut img, 5, record(3, &[name(5, ".")]));
        put(&mut img, 10, record(3, &[name(5, "Photos")]));
        // deleted photo of 6000 bytes in two pieces: cluster 20 then cluster 30
        put(&mut img, 11, record(0, &[name(10, "vacances.jpg"), outside(6000, &[0x11, 1, 20, 0x11, 1, 10])]));
        put(&mut img, 12, record(0, &[name(5, "note.txt"), resident(0x80, b"bonjour")]));
        put(&mut img, 13, record(1, &[name(5, "vivant.txt"), resident(0x80, b"encore la")]));
        img[20 * CL..21 * CL].fill(0xAA);
        img[30 * CL..31 * CL].fill(0xBB);
        img
    }

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
    fn finds_deleted_files_with_their_names() {
        let s = Mutex::new(ScanShared::default());
        let mut id = 0;
        let ctx = eframe::egui::Context::default();
        scan_volume(&mut Mem(image()), "img", "C:", None, &Wanted::default(), &s, &AtomicBool::new(false), &mut id, &ctx).unwrap();
        let g = s.lock().unwrap();
        let mut names: Vec<(&str, &str, u64)> = g.new_files.iter().map(|f| (f.name.as_str(), f.place.as_str(), f.size)).collect();
        names.sort();
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(
            names,
            vec![
                ("chat.jpg", "Corbeille vidée · C:\\Users\\Mme\\Images", 100),
                ("note.txt", "C:", 7),
                ("vacances.jpg", &*format!("C:{sep}Photos"), 6000)
            ]
        );
        let photo = g.new_files.iter().find(|f| f.name == "vacances.jpg").unwrap();
        let Origin::Runs { runs, .. } = &photo.origin else { panic!() };
        assert_eq!(runs, &vec![(Some(20 * 4096), 4096), (Some(30 * 4096), 4096)]);
        assert_eq!(photo.date.as_deref(), Some("18/06/2022"));
    }
}
