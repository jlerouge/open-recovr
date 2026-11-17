//! The quick search on FAT12, FAT16, FAT32 and exFAT (memory cards, USB sticks): a deleted file
//! keeps its entry in its folder, with its name, its size and its first cluster, until the entry
//! is reused. FAT32 loses the first letter of the short name (the long name is kept) and empties
//! the chain of clusters; exFAT only clears a flag, and says whether the file was in one piece.
//!
//! Where the rest of a file cut in pieces lay, FAT forgets. But it hands out the free clusters
//! in order: a file took the free clusters after its first one, jumping over those in use. The
//! clusters still in use now were most likely in use then, so the file is read back the same way.

use crate::carve::{Source, Volume};
use crate::engine::{Category, Chance, Found, Origin, ScanShared, Wanted};
use crate::sys::Disk;
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn is_fat(disk: &Disk) -> bool {
    let fs = disk.fs.to_ascii_uppercase();
    fs.starts_with("FAT") || fs == "EXFAT"
}

/// Whether a boot sector is that of a FAT or exFAT volume.
pub fn is_fat_boot(b: &[u8]) -> bool {
    boot(b).is_some()
}

fn le16(b: &[u8], at: usize) -> u64 {
    u16::from_le_bytes([b[at], b[at + 1]]) as u64
}
fn le32(b: &[u8], at: usize) -> u64 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as u64
}
fn le64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Fat12,
    Fat16,
    Fat32,
    ExFat,
}

#[derive(Clone, Copy, Debug)]
enum Root {
    /// FAT12 and FAT16: a fixed area before the clusters.
    Fixed {
        at: u64,
        len: u64,
    },
    Cluster(u32),
}

#[derive(Debug)]
struct Fs {
    kind: Kind,
    /// Bytes in a cluster.
    cluster: u64,
    /// Where cluster 2, the first one, starts in the volume.
    data: u64,
    /// Clusters in the volume.
    count: u64,
    fat_at: u64,
    root: Root,
}

impl Fs {
    fn valid(&self, c: u32) -> bool {
        c >= 2 && (c as u64) < self.count + 2
    }
    fn at(&self, c: u32) -> u64 {
        self.data + (c as u64 - 2) * self.cluster
    }
}

fn boot(b: &[u8]) -> Option<Fs> {
    if b.len() < 512 || b[510..512] != [0x55, 0xAA] {
        return None;
    }
    if &b[3..11] == b"EXFAT   " {
        let (bs, cs) = (b[108] as u32, b[109] as u32);
        if !(9..=12).contains(&bs) || bs + cs > 25 {
            return None;
        }
        let fs = Fs {
            kind: Kind::ExFat,
            cluster: 1 << (bs + cs),
            data: le32(b, 88) << bs,
            count: le32(b, 92),
            fat_at: le32(b, 80) << bs,
            root: Root::Cluster(le32(b, 96) as u32),
        };
        return (fs.count > 0).then_some(fs);
    }
    if !matches!(b[0], 0xEB | 0xE9) || &b[3..11] == b"NTFS    " {
        return None;
    }
    let (bps, spc, reserved, fats) = (le16(b, 11), b[13] as u64, le16(b, 14), b[16] as u64);
    if !bps.is_power_of_two() || !(512..=4096).contains(&bps) || !spc.is_power_of_two() || reserved == 0 || fats == 0 {
        return None;
    }
    let entries = le16(b, 17);
    let fat_size = if le16(b, 22) != 0 { le16(b, 22) } else { le32(b, 36) };
    let total = if le16(b, 19) != 0 { le16(b, 19) } else { le32(b, 32) };
    let meta = reserved + fats * fat_size + (entries * 32).div_ceil(bps);
    if fat_size == 0 || total <= meta {
        return None;
    }
    // the kind is told by the number of clusters, nothing else
    let count = (total - meta) / spc;
    let kind = match count {
        0..4085 => Kind::Fat12,
        4085..65525 => Kind::Fat16,
        _ => Kind::Fat32,
    };
    let root = match kind {
        Kind::Fat32 => Root::Cluster(le32(b, 44) as u32),
        _ => Root::Fixed { at: (reserved + fats * fat_size) * bps, len: entries * 32 },
    };
    Some(Fs { kind, cluster: bps * spc, data: meta * bps, count, fat_at: reserved * bps, root })
}

/// The end of a chain, or a cluster marked bad.
const END: u32 = u32::MAX;

/// Which clusters are free, and the chains of those in use.
struct Map {
    fat: Vec<u32>,
    /// exFAT: the allocation bitmap, one bit per cluster from cluster 2; a file in one piece
    /// has nothing in the FAT.
    bitmap: Option<Vec<u8>>,
}

impl Map {
    fn read(vol: &mut impl Source, fs: &Fs) -> Map {
        // beyond 64 M clusters (256 MB of FAT) the rest counts as in use
        let top = (fs.count + 2).min(1 << 26) as usize;
        let len = match fs.kind {
            Kind::Fat12 => top * 3 / 2 + 2,
            Kind::Fat16 => top * 2,
            _ => top * 4,
        };
        let raw = read_bytes(vol, fs.fat_at, len);
        let fat = (0..top)
            .map(|n| match fs.kind {
                Kind::Fat12 => {
                    let i = n * 3 / 2;
                    let Some(w) = raw.get(i..i + 2) else { return END };
                    let w = u16::from_le_bytes([w[0], w[1]]) as u32;
                    let v = if n % 2 == 0 { w & 0xFFF } else { w >> 4 };
                    if v >= 0xFF7 { END } else { v }
                }
                Kind::Fat16 => match raw.get(n * 2..n * 2 + 2) {
                    Some(w) if u16::from_le_bytes([w[0], w[1]]) < 0xFFF7 => u16::from_le_bytes([w[0], w[1]]) as u32,
                    _ => END,
                },
                Kind::Fat32 | Kind::ExFat => match raw.get(n * 4..n * 4 + 4) {
                    Some(w) => {
                        let v = u32::from_le_bytes(w.try_into().unwrap());
                        let v = if fs.kind == Kind::Fat32 { v & 0x0FFF_FFFF } else { v };
                        if v >= if fs.kind == Kind::Fat32 { 0x0FFF_FFF7 } else { 0xFFFF_FFF7 } { END } else { v }
                    }
                    None => END,
                },
            })
            .collect();
        Map { fat, bitmap: None }
    }

    fn free(&self, c: u32) -> bool {
        match &self.bitmap {
            Some(bm) => {
                let i = (c as usize).wrapping_sub(2);
                bm.get(i / 8).is_some_and(|x| x & (1 << (i % 8)) == 0)
            }
            None => self.fat.get(c as usize) == Some(&0),
        }
    }

    /// The clusters of a chain, at most `max`, stopping at its end, a loop or a free cluster.
    fn chain(&self, start: u32, max: usize) -> Vec<u32> {
        let (mut out, mut seen, mut c) = (Vec::new(), HashSet::new(), start);
        while c >= 2 && (c as usize) < self.fat.len() && out.len() < max && seen.insert(c) {
            out.push(c);
            c = self.fat[c as usize];
        }
        out
    }
}

fn read_bytes(vol: &mut impl Source, at: u64, len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    let mut done = 0;
    while done < len {
        let n = (len - done).min(4 << 20);
        let got = vol.read_at(at + done as u64, &mut out[done..done + n]);
        if got == 0 {
            break;
        }
        done += got;
    }
    out.truncate(done);
    out
}

/// Where a folder's entries lie.
#[derive(Clone, Copy, Debug)]
enum Loc {
    Root,
    Chain(u32),
    /// Clusters one after the other; 0 for a deleted FAT folder, whose length is not known.
    Run(u32, u64),
}

/// A folder may not be larger than this (65 536 entries).
const DIR_MAX: u64 = 2 << 20;

fn read_dir(vol: &mut impl Source, fs: &Fs, map: &Map, loc: Loc) -> Vec<u8> {
    let max = (DIR_MAX / fs.cluster).max(1) as usize;
    let clusters: Vec<u32> = match loc {
        Loc::Root => match fs.root {
            Root::Fixed { at, len } => return read_bytes(vol, at, len.min(DIR_MAX) as usize),
            Root::Cluster(c) => map.chain(c, max),
        },
        Loc::Chain(c) => map.chain(c, max),
        Loc::Run(c, n) if n > 0 => (c..).take((n as usize).min(max)).filter(|&c| fs.valid(c)).collect(),
        // a deleted FAT folder: its free clusters from the first, while they read as entries
        Loc::Run(c, _) => {
            let mut out = Vec::new();
            for k in 0..max.min(64) as u32 {
                let c = c + k;
                if !fs.valid(c) || (k > 0 && !map.free(c)) {
                    break;
                }
                let buf = read_bytes(vol, fs.at(c), fs.cluster as usize);
                // the first holds "." and "..", the others nothing but entries
                if (k == 0 && !buf.starts_with(b".          ")) || (k > 0 && !looks_like_entries(&buf)) {
                    break;
                }
                let end = buf.as_chunks::<32>().0.iter().any(|e| e[0] == 0);
                out.extend_from_slice(&buf);
                if end {
                    break;
                }
            }
            return out;
        }
    };
    let mut out = Vec::with_capacity(clusters.len() * fs.cluster as usize);
    for c in clusters {
        let buf = read_bytes(vol, fs.at(c), fs.cluster as usize);
        let short = buf.len() < fs.cluster as usize;
        out.extend_from_slice(&buf);
        if short {
            break;
        }
    }
    out
}

/// Whether a cluster reads as FAT folder entries, up to the end marker.
fn looks_like_entries(buf: &[u8]) -> bool {
    for e in buf.as_chunks::<32>().0.iter() {
        if e[0] == 0 {
            return e.iter().all(|&x| x == 0);
        }
        let lfn = e[11] & 0x3F == 0x0F;
        if e[11] & 0xC0 != 0 || (!lfn && e[0] != 0xE5 && e[..11].iter().any(|&x| x < 0x20)) {
            return false;
        }
    }
    true
}

#[derive(Clone, Debug)]
struct Entry {
    name: String,
    deleted: bool,
    dir: bool,
    start: u32,
    size: u64,
    /// Last written, in seconds since 1970 (local time), 0 if unknown.
    stamp: u64,
    /// exFAT: the file was in one piece, its chain never written.
    contiguous: bool,
}

/// Seconds since 1970 of a FAT date and time.
fn dos_secs(date: u64, time: u64) -> u64 {
    let (y, m, d) = (1980 + (date >> 9) as i64, ((date >> 5) & 15) as i64, (date & 31) as i64);
    if !(1..=12).contains(&m) || d == 0 {
        return 0;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    let secs = days * 86_400 + (time >> 11) as i64 * 3600 + ((time >> 5) & 63) as i64 * 60 + (time & 31) as i64 * 2;
    secs.max(0) as u64
}

fn utf16_name(units: impl Iterator<Item = u16>) -> String {
    let units: Vec<u16> = units.take_while(|&u| u != 0 && u != 0xFFFF).collect();
    String::from_utf16_lossy(&units)
}

/// The entries of a FAT folder, deleted ones included.
fn parse_fat(buf: &[u8], fat32: bool) -> Vec<Entry> {
    let mut out = Vec::new();
    // the parts of a long name, last part first, and whether they were deleted
    let mut long: Vec<&[u8]> = Vec::new();
    let mut long_deleted = false;
    for e in buf.as_chunks::<32>().0.iter() {
        if e[0] == 0 {
            break;
        }
        let deleted = e[0] == 0xE5;
        if e[11] & 0x3F == 0x0F {
            if long.is_empty() || long_deleted != deleted {
                long.clear();
                long_deleted = deleted;
            }
            long.push(e);
            continue;
        }
        let parts = std::mem::take(&mut long);
        if e[11] & 0x08 != 0 || e[..11] == *b".          " || e[..11] == *b"..         " {
            continue;
        }
        let name = if !parts.is_empty() && long_deleted == deleted {
            utf16_name(parts.iter().rev().flat_map(|p| {
                [1..11, 14..26, 28..32]
                    .into_iter()
                    .flat_map(move |r| p[r].as_chunks::<2>().0.iter().map(|w| u16::from_le_bytes([w[0], w[1]])))
            }))
        } else {
            String::new()
        };
        let name = if name.is_empty() { short_name(e, deleted) } else { name };
        let hi = if fat32 { le16(e, 20) << 16 } else { 0 };
        out.push(Entry {
            name,
            deleted,
            dir: e[11] & 0x10 != 0,
            start: (hi | le16(e, 26)) as u32,
            size: le32(e, 28),
            stamp: dos_secs(le16(e, 24), le16(e, 22)),
            contiguous: false,
        });
    }
    out
}

/// "PHOTO.JPG" as written; the first letter, lost when deleted, as "_".
fn short_name(e: &[u8], deleted: bool) -> String {
    let text = |b: &[u8], lower: bool| -> String {
        let s: String = b.iter().map(|&x| if (0x20..0x7F).contains(&x) { x as char } else { '_' }).collect();
        let s = s.trim_end().to_string();
        if lower { s.to_lowercase() } else { s }
    };
    let mut base = e[..8].to_vec();
    if deleted {
        base[0] = b'_';
    }
    let (base, ext) = (text(&base, e[12] & 0x08 != 0), text(&e[8..11], e[12] & 0x10 != 0));
    if ext.is_empty() { base } else { format!("{base}.{ext}") }
}

/// The entries of an exFAT folder, deleted ones included, and where the allocation bitmap lies
/// (in the root folder).
fn parse_exfat(buf: &[u8], bitmap: &mut Option<(u32, u64)>) -> Vec<Entry> {
    let mut out = Vec::new();
    let entries: Vec<&[u8; 32]> = buf.as_chunks::<32>().0.iter().collect();
    let mut i = 0;
    while i < entries.len() {
        let e = entries[i];
        match e[0] {
            0 => break,
            0x81 if bitmap.is_none() => *bitmap = Some((le32(e, 20) as u32, le64(e, 24))),
            0x85 | 0x05 => {
                let deleted = e[0] == 0x05;
                let used = if deleted { 0 } else { 0x80 };
                let n = e[1] as usize;
                let set = &entries[i + 1..(i + 1 + n).min(entries.len())];
                // a stream entry, then the name, all with the same "in use" flag
                if n >= 2
                    && set.len() == n
                    && set[0][0] == 0x40 | used
                    && set[1..].iter().all(|x| x[0] & 0x7F == 0x41 || x[0] & 0x80 != used)
                {
                    let s = set[0];
                    let len = s[3] as usize;
                    let name = utf16_name(
                        set[1..]
                            .iter()
                            .filter(|x| x[0] == 0x41 | used)
                            .flat_map(|x| x[2..32].as_chunks::<2>().0.iter().map(|w| u16::from_le_bytes([w[0], w[1]])))
                            .take(len),
                    );
                    let ts = le32(e, 12);
                    out.push(Entry {
                        name,
                        deleted,
                        dir: le16(e, 4) & 0x10 != 0,
                        start: le32(s, 20) as u32,
                        size: le64(s, 24),
                        stamp: dos_secs(ts >> 16, ts & 0xFFFF),
                        contiguous: s[1] & 2 != 0,
                    });
                    i += n;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// Where a folder found in another lies, if it can be read.
fn dir_loc(fs: &Fs, map: &Map, e: &Entry, gone: bool) -> Option<Loc> {
    let n = e.size.div_ceil(fs.cluster);
    if gone && !map.free(e.start) {
        return None;
    }
    Some(match (fs.kind, e.contiguous, gone) {
        (Kind::ExFat, true, _) => Loc::Run(e.start, n),
        (Kind::ExFat, false, true) if map.chain(e.start, n as usize).len() as u64 != n => Loc::Run(e.start, n),
        (Kind::ExFat, _, _) | (_, _, false) => Loc::Chain(e.start),
        _ => Loc::Run(e.start, 0),
    })
}

/// Windows clears the upper half of the first cluster of a file it deletes on FAT32. Of the
/// clusters it may have been, the one that reads as the start of that folder or file, else one
/// still free, nearest its folder.
fn first_cluster(vol: &mut impl Source, fs: &Fs, map: &Map, e: &Entry, near: u32) -> u32 {
    if fs.kind != Kind::Fat32 || !e.deleted || e.start > 0xFFFF || fs.count + 2 <= 0x1_0000 {
        return e.start;
    }
    let mut best = (i64::MIN, e.start);
    for hi in 0..=((fs.count + 1) >> 16) as u32 {
        let c = hi << 16 | e.start;
        if !fs.valid(c) {
            continue;
        }
        let score = (fits(vol, fs, e, c).abs() << 40) + ((map.free(c) as i64) << 39) - c.abs_diff(near) as i64;
        if score > best.0 {
            best = (score, c);
        }
    }
    best.1
}

/// How well cluster `c` reads as the start of a folder (3: its "." entry names it) or a file
/// (-1: the header of another kind, 0: none, 1: a header whose kind can't be told,
/// 2: a header of the kind its name says, 3: and of its size).
fn fits(vol: &mut impl Source, fs: &Fs, e: &Entry, c: u32) -> i64 {
    let head = read_bytes(vol, fs.at(c), 512);
    if e.dir {
        return (head.len() >= 32 && head.starts_with(b".          ") && (le16(&head, 20) << 16 | le16(&head, 26)) as u32 == c) as i64 * 3;
    }
    let ext = norm_ext(e.name.rsplit_once('.').map(|(_, x)| x).unwrap_or(""));
    match crate::carve::detect(&head) {
        None => 0,
        Some(kind) => match crate::carve::measure(vol as &mut dyn Source, fs.at(c), kind) {
            Some((len, x)) if norm_ext(x) == ext => 2 + (len.div_ceil(fs.cluster) == e.size.div_ceil(fs.cluster)) as i64,
            Some(_) => -1,
            None => 1,
        },
    }
}

/// Kinds of file whose first bytes the carver always recognises.
const ALWAYS_HEADED: &[&str] = &["jpg", "png", "gif", "bmp", "webp", "tif", "pdf", "zip", "ole", "ogg", "flac", "psd"];

/// One name for the extensions of a kind of file.
fn norm_ext(x: &str) -> String {
    let x = x.to_ascii_lowercase();
    match x.as_str() {
        "jpeg" | "jpe" | "jfif" => "jpg",
        "tiff" => "tif",
        "htm" | "xhtml" => "html",
        "mpeg" | "mpe" => "mpg",
        "doc" | "xls" | "ppt" | "pps" | "msg" | "dot" | "xlt" | "pot" => "ole",
        // one container under several names: the carver may give the family's name
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" | "epub" | "jar" | "apk" | "xps" => "zip",
        "mov" | "m4a" | "m4v" | "3gp" | "3g2" | "heic" | "heif" | "avif" => "mp4",
        "webm" | "mka" => "mkv",
        "cr2" | "nef" | "arw" | "dng" | "orf" | "rw2" | "pef" => "tif",
        "opus" | "oga" | "ogv" => "ogg",
        _ => return x,
    }
    .to_string()
}

/// The clusters a deleted file held, as (first, count) runs, whether some are in use now, and
/// whether the runs are only guessed from the free clusters around it.
fn place(fs: &Fs, map: &Map, e: &Entry, starts: &HashSet<u32>) -> (Vec<(u32, u64)>, bool, bool) {
    let n = e.size.div_ceil(fs.cluster);
    let whole = |reused| (vec![(e.start, n)], reused, false);
    let top = fs.count + 2;
    if !map.free(e.start) {
        return whole(true);
    }
    if e.contiguous {
        return whole((e.start as u64..(e.start as u64 + n).min(top)).any(|c| !map.free(c as u32)));
    }
    let mut runs: Vec<(u32, u64)> = Vec::new();
    let mut push = |c: u32| match runs.last_mut() {
        Some(r) if r.0 as u64 + r.1 == c as u64 => r.1 += 1,
        _ => runs.push((c, 1)),
    };
    // exFAT may leave the chain of a deleted file in the FAT
    if fs.kind == Kind::ExFat {
        let chain = map.chain(e.start, n as usize);
        if chain.len() as u64 == n && n > 1 && chain.iter().all(|&c| map.free(c)) {
            chain.into_iter().for_each(&mut push);
            return (runs, false, false);
        }
    }
    let taken = |c: u64| !map.free(c as u32) || (c != e.start as u64 && starts.contains(&(c as u32)));
    // the free run from the first cluster
    let mut c = e.start as u64;
    while c < top && c - (e.start as u64) < n && !taken(c) {
        c += 1;
    }
    let first = c - e.start as u64;
    if first == n {
        return whole(false);
    }
    // then, as Windows hands out space it cannot give in one piece: the largest free runs first,
    // all from one block of 65 536 clusters
    const BLOCK: u64 = 1 << 16;
    let free_runs = |lo: u64, hi: u64| {
        let mut out: Vec<(u64, u64)> = Vec::new();
        let mut c = lo.max(2);
        while c < hi.min(top) {
            if taken(c) || (c >= e.start as u64 && c < e.start as u64 + first) {
                c += 1;
                continue;
            }
            let s = c;
            while c < hi.min(top) && !taken(c) && !(c >= e.start as u64 && c < e.start as u64 + first) {
                c += 1;
                // another deleted file starts here: its own run
                if c < top && starts.contains(&(c as u32)) {
                    break;
                }
            }
            out.push((s, c - s));
        }
        out
    };
    let block =
        (0..top.div_ceil(BLOCK)).max_by_key(|&b| free_runs(b * BLOCK, (b + 1) * BLOCK).iter().map(|r| r.1).sum::<u64>()).unwrap_or(0);
    let mut pool = free_runs(block * BLOCK, (block + 1) * BLOCK);
    pool.sort_by_key(|&(s, len)| (std::cmp::Reverse(len), s));
    let mut out = vec![(e.start, first)];
    let mut left = n - first;
    for (s, len) in pool {
        if left == 0 {
            break;
        }
        let k = len.min(left);
        out.push((s as u32, k));
        left -= k;
    }
    if left > 0 {
        return whole(true);
    }
    (out, false, true)
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
    let mut vol = Volume { inner: file, size: u64::MAX / 2 };
    let root = disk.root.to_string_lossy().trim_end_matches(['\\', '/']).to_string();
    scan_volume(&mut vol, &disk.device, &root, wanted, s, cancel, next_id, ctx)
}

#[allow(clippy::too_many_arguments)]
pub fn scan_volume(
    vol: &mut impl Source,
    device: &str,
    root: &str,
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
    let fs = boot(&first).ok_or(t!("ce n'est pas un disque FAT ou exFAT"))?;
    {
        let mut g = s.lock().unwrap();
        g.phase = t!("Analyse rapide : lecture de la liste des fichiers…").into();
        g.progress = 0.0;
        let kind = match fs.kind {
            Kind::Fat12 => "FAT12",
            Kind::Fat16 => "FAT16",
            Kind::Fat32 => "FAT32",
            Kind::ExFat => "exFAT",
        };
        g.note(t!("Analyse rapide : table {} de {} blocs de {} o", kind, fs.count, fs.cluster));
    }
    ctx.request_repaint();
    let mut map = Map::read(vol, &fs);

    // every folder from the root, deleted ones too: (where, path, deleted, depth, first cluster)
    let root_at = match fs.root {
        Root::Cluster(c) => c,
        Root::Fixed { .. } => 2,
    };
    let mut todo = vec![(Loc::Root, String::new(), false, 0usize, root_at)];
    let mut seen = HashSet::new();
    let mut lost: Vec<(Entry, String)> = Vec::new();
    while let Some((loc, path, gone, depth, near)) = todo.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let buf = read_dir(vol, &fs, &map, loc);
        let entries = if fs.kind == Kind::ExFat {
            let mut bitmap = None;
            let entries = parse_exfat(&buf, &mut bitmap);
            if let (Loc::Root, Some((c, len))) = (loc, bitmap)
                && fs.valid(c)
            {
                map.bitmap = Some(read_bytes(vol, fs.at(c), len.min(fs.count.div_ceil(8)) as usize));
            }
            entries
        } else {
            parse_fat(&buf, fs.kind == Kind::Fat32)
        };
        for mut e in entries {
            let gone = gone || e.deleted;
            e.start = first_cluster(vol, &fs, &map, &e, near);
            if !fs.valid(e.start) {
                continue;
            }
            if e.dir {
                if depth < 32
                    && seen.insert(e.start)
                    && let Some(at) = dir_loc(&fs, &map, &e, gone)
                {
                    todo.push((at, format!("{path}\\{}", e.name), gone, depth + 1, e.start));
                }
            } else if gone && e.size > 0 {
                lost.push((e, path.clone()));
            }
        }
    }
    s.lock().unwrap().progress = 0.5;
    ctx.request_repaint();

    // where the other deleted files start was not free when a file was written after them
    let starts: HashSet<u32> = lost.iter().filter(|(e, _)| fits(vol, &fs, e, e.start) >= 2).map(|(e, _)| e.start).collect();
    // two deleted files at one place: the one whose size the content measures was written last
    let mut sizes: std::collections::HashMap<u32, Vec<u64>> = std::collections::HashMap::new();
    for (e, _) in &lost {
        sizes.entry(e.start).or_default().push(e.size);
    }
    let mut last: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    for (&c, sz) in sizes.iter().filter(|(_, sz)| sz.len() > 1) {
        let head = read_bytes(vol, fs.at(c), 512);
        if let Some(kind) = crate::carve::detect(&head)
            && let Some((len, _)) = crate::carve::measure(vol as &mut dyn Source, fs.at(c), kind)
            && sz.contains(&len)
        {
            last.insert(c, len);
        }
    }
    let (mut kept, mut noise, mut skipped) = (0usize, 0usize, 0usize);
    let mut batch = Vec::new();
    for (e, folder) in lost {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let place_name = format!("{root}{folder}");
        let low = format!("{}\\", place_name.to_lowercase());
        if crate::ntfs::NOISE.iter().any(|n| low.contains(n))
            || e.name.starts_with('~')
            || e.name.starts_with("._")
            || e.name.ends_with(".tmp")
        {
            noise += 1;
            continue;
        }
        let ext = e.name.rsplit_once('.').map(|(_, x)| x.to_lowercase()).unwrap_or_default();
        let cat = Category::from_ext(&ext);
        if !wanted.accepts(&ext, cat, e.size, false) {
            skipped += 1;
            continue;
        }
        let (runs, reused, guess) = place(&fs, &map, &e, &starts);
        // a drive that blanks freed space (SSD) reads back zeros
        let mut head = vec![0u8; 4096.min(e.size as usize).max(512)];
        let got = vol.read_at(fs.at(e.start), &mut head);
        let blank = got > 0 && head[..got].iter().all(|&x| x == 0);
        // another file written there since, and deleted in turn: the clusters are free again, but
        // they start with another kind of file, or with no header where this kind always has one
        let fit = if reused || blank { 3 } else { fits(vol, &fs, &e, e.start) };
        let overwritten = fit == -1
            || (fit == 0 && ALWAYS_HEADED.contains(&norm_ext(&ext).as_str()))
            || last.get(&e.start).is_some_and(|&len| len != e.size);
        let runs = runs.into_iter().map(|(c, k)| (Some(fs.at(c)), k * fs.cluster)).collect();
        batch.push(Found {
            id: *next_id,
            name: e.name,
            ext,
            category: cat,
            size: e.size,
            place: if folder.is_empty() { format!("{root}\\") } else { place_name },
            date: crate::engine::secs_to_date(e.stamp),
            stamp: e.stamp,
            chance: if reused || blank || overwritten { Chance::Low } else { Chance::Good },
            origin: Origin::Runs { device: device.to_string(), runs, guess },
            checked: false,
        });
        *next_id += 1;
        kept += 1;
    }
    let mut g = s.lock().unwrap();
    g.progress = 1.0;
    for f in batch {
        g.add(f);
    }
    g.note(t!("Analyse rapide : {} fichier(s) gardé(s), {} fichier(s) système ignoré(s), {} hors des critères", kept, noise, skipped));
    ctx.request_repaint();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mem(Vec<u8>);
    impl Source for Mem {
        fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> usize {
            let o = (offset as usize).min(self.0.len());
            let n = buf.len().min(self.0.len() - o);
            buf[..n].copy_from_slice(&self.0[o..o + n]);
            n
        }
    }

    const CL: usize = 512;
    const DATA: usize = 32 * 512 + 2 * 1024 * 512;

    /// A FAT32 volume of 70 000 clusters of 512 bytes (FAT of 1024 sectors, root at cluster 2).
    fn fat32() -> Vec<u8> {
        let mut v = vec![0u8; DATA + 70_000 * CL];
        let b = &mut v[..512];
        b[0] = 0xEB;
        b[3..11].copy_from_slice(b"MSDOS5.0");
        b[11..13].copy_from_slice(&512u16.to_le_bytes());
        b[13] = 1;
        b[14..16].copy_from_slice(&32u16.to_le_bytes());
        b[16] = 2;
        b[32..36].copy_from_slice(&((DATA / 512 + 70_000) as u32).to_le_bytes());
        b[36..40].copy_from_slice(&1024u32.to_le_bytes());
        b[44..48].copy_from_slice(&2u32.to_le_bytes());
        b[82..87].copy_from_slice(b"FAT32");
        b[510] = 0x55;
        b[511] = 0xAA;
        set_fat(&mut v, 2, END);
        v
    }

    fn set_fat(v: &mut [u8], c: u32, next: u32) {
        let at = 32 * 512 + c as usize * 4;
        v[at..at + 4].copy_from_slice(&(next & 0x0FFF_FFFF).to_le_bytes());
    }

    fn cl(c: u32) -> usize {
        DATA + (c as usize - 2) * CL
    }

    /// A short entry; deleted ones start with 0xE5.
    fn entry(name: &[u8; 11], attr: u8, start: u32, size: u32) -> [u8; 32] {
        let mut e = [0u8; 32];
        e[..11].copy_from_slice(name);
        e[11] = attr;
        e[20..22].copy_from_slice(&((start >> 16) as u16).to_le_bytes());
        e[26..28].copy_from_slice(&(start as u16).to_le_bytes());
        e[28..32].copy_from_slice(&size.to_le_bytes());
        e[24..26].copy_from_slice(&((45 << 9 | 3 << 5 | 14) as u16).to_le_bytes());
        e
    }

    fn long_part(text: &str, deleted: bool) -> [u8; 32] {
        let mut e = [0xFFu8; 32];
        let mut u: Vec<u16> = text.encode_utf16().collect();
        u.push(0);
        e[0] = if deleted { 0xE5 } else { 0x41 };
        e[11] = 0x0F;
        e[12] = 0;
        e[26] = 0;
        e[27] = 0;
        for (i, at) in [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30].into_iter().enumerate() {
            if let Some(&c) = u.get(i) {
                e[at..at + 2].copy_from_slice(&c.to_le_bytes());
            }
        }
        e
    }

    fn scan_mem(v: Vec<u8>) -> Vec<Found> {
        let s = Mutex::new(ScanShared::default());
        let mut id = 0;
        scan_volume(&mut Mem(v), "img", "E:", &Wanted::default(), &s, &AtomicBool::new(false), &mut id, &eframe::egui::Context::default())
            .unwrap();
        std::mem::take(&mut s.lock().unwrap().new_files)
    }

    #[test]
    fn deleted_fat32_file_cut_by_a_live_one() {
        let mut v = fat32();
        // a live file in clusters 12..14, a deleted one of 5 clusters from 10, around it
        let root = cl(2);
        v[root..root + 32].copy_from_slice(&entry(b"LIVE    BIN", 0x20, 12, 1500));
        v[root + 32..root + 64].copy_from_slice(&long_part("photo.jpg", true));
        let mut del = entry(b"\xE5HOTO   JPG", 0x20, 10, 2400);
        del[0] = 0xE5;
        v[root + 64..root + 96].copy_from_slice(&del);
        set_fat(&mut v, 12, 13);
        set_fat(&mut v, 13, 14);
        set_fat(&mut v, 14, END);
        v[cl(10)..cl(10) + 4].copy_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0]);
        let found = scan_mem(v);
        assert_eq!(found.len(), 1);
        let f = &found[0];
        assert_eq!((f.name.as_str(), f.size, f.chance), ("photo.jpg", 2400, Chance::Good));
        assert_eq!(f.place, "E:\\");
        assert_eq!(f.stamp, dos_secs(45 << 9 | 3 << 5 | 14, 0));
        let Origin::Runs { runs, .. } = &f.origin else { panic!() };
        assert_eq!(runs, &vec![(Some(cl(10) as u64), 2 * CL as u64), (Some(cl(15) as u64), 3 * CL as u64)]);
    }

    #[test]
    fn deleted_folder_and_short_name() {
        let mut v = fat32();
        let root = cl(2);
        let mut dir = entry(b"\xE5OTOS      ", 0x10, 20, 0);
        dir[0] = 0xE5;
        v[root..root + 32].copy_from_slice(&dir);
        let d = cl(20);
        v[d..d + 32].copy_from_slice(&entry(b".          ", 0x10, 20, 0));
        v[d + 32..d + 64].copy_from_slice(&entry(b"..         ", 0x10, 0, 0));
        // its file, deleted with it
        let mut f = entry(b"\xE5MG_0001JPG", 0x20, 21, 700);
        f[0] = 0xE5;
        v[d + 64..d + 96].copy_from_slice(&f);
        let found = scan_mem(v);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "_MG_0001.JPG");
        assert_eq!(found[0].place, "E:\\_OTOS");
        let Origin::Runs { runs, .. } = &found[0].origin else { panic!() };
        assert_eq!(runs, &vec![(Some(cl(21) as u64), 2 * CL as u64)]);
    }

    #[test]
    fn reused_first_cluster_is_a_low_chance() {
        let mut v = fat32();
        let root = cl(2);
        v[root..root + 32].copy_from_slice(&entry(b"NEW     TXT", 0x20, 30, 100));
        let mut del = entry(b"\xE5LD     TXT", 0x20, 30, 100);
        del[0] = 0xE5;
        v[root + 32..root + 64].copy_from_slice(&del);
        set_fat(&mut v, 30, END);
        let found = scan_mem(v);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].chance, Chance::Low);
    }

    #[test]
    fn exfat_deleted_entry_set() {
        // exFAT, clusters of 4 KiB from byte 64 KiB, root at cluster 2, bitmap at cluster 3
        let mut v = vec![0u8; 65536 + 100 * 4096];
        let b = &mut v[..512];
        b[3..11].copy_from_slice(b"EXFAT   ");
        b[80..84].copy_from_slice(&8u32.to_le_bytes());
        b[84..88].copy_from_slice(&8u32.to_le_bytes());
        b[88..92].copy_from_slice(&128u32.to_le_bytes());
        b[92..96].copy_from_slice(&100u32.to_le_bytes());
        b[96..100].copy_from_slice(&2u32.to_le_bytes());
        b[108] = 9;
        b[109] = 3;
        b[510] = 0x55;
        b[511] = 0xAA;
        let fat = 8 * 512;
        for c in [2u32, 3] {
            v[fat + c as usize * 4..fat + c as usize * 4 + 4].copy_from_slice(&END.to_le_bytes());
        }
        let root = 65536;
        let mut bm = [0u8; 32];
        bm[0] = 0x81;
        bm[20..24].copy_from_slice(&3u32.to_le_bytes());
        bm[24..32].copy_from_slice(&13u64.to_le_bytes());
        v[root..root + 32].copy_from_slice(&bm);
        // bitmap: clusters 2 and 3 in use, and 6 (a live file)
        v[65536 + 4096] = 0b0001_0011;
        // a deleted, fragmented file "clip.mp4" of 3 clusters from 4: 4, 5, then 7 (6 is in use)
        let mut f = [0u8; 32];
        f[0] = 0x05;
        f[1] = 2;
        let mut st = [0u8; 32];
        st[0] = 0x40;
        st[1] = 1;
        st[3] = 8;
        st[20..24].copy_from_slice(&4u32.to_le_bytes());
        st[24..32].copy_from_slice(&10_000u64.to_le_bytes());
        let mut nm = [0u8; 32];
        nm[0] = 0x41;
        for (i, c) in "clip.mp4".encode_utf16().enumerate() {
            nm[2 + 2 * i..4 + 2 * i].copy_from_slice(&c.to_le_bytes());
        }
        v[root + 32..root + 64].copy_from_slice(&f);
        v[root + 64..root + 96].copy_from_slice(&st);
        v[root + 96..root + 128].copy_from_slice(&nm);
        let found = scan_mem(v.clone());
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].name.as_str(), found[0].size), ("clip.mp4", 10_000));
        let at = |c: u64| Some(65536 + (c - 2) * 4096);
        let Origin::Runs { runs, .. } = &found[0].origin else { panic!() };
        assert_eq!(runs, &vec![(at(4), 2 * 4096), (at(7), 4096)]);
        // marked as in one piece: read straight through
        v[root + 65] = 3;
        let found = scan_mem(v);
        let Origin::Runs { runs, .. } = &found[0].origin else { panic!() };
        assert_eq!(runs, &vec![(at(4), 3 * 4096)]);
    }
}
