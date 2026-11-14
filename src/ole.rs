//! OLE compound files (Word, Excel, PowerPoint 97-2003...) cut in pieces.
//!
//! A compound file is a little file system of 512-byte sectors: a FAT chains them into streams,
//! a directory names the streams. None of it carries a checksum, so a file is checked through
//! what its parts say about each other:
//! - every FAT sector holds valid entries, and the FAT marks its own sectors as such;
//! - every chain stays inside the file and ends, the directory reads as one;
//! - an Excel workbook is a chain of records whose lengths lead from one to the next;
//! - in a Word document, the pages that describe the paragraphs say where each one ends: a
//!   paragraph mark must stand there in the text.
//!
//! Writers put the FAT near the end of the file, so in its last piece: the FAT sector that carries
//! the marks of the other FAT sectors is looked for on the disk, and places that piece. The cut
//! between the first piece and the last is then searched from the top down, each check that fails
//! in the first piece pulling the bound down to where it failed.

use crate::carve::{Kind, Source};
use crate::frag::{self, Claims, Feed, SECTOR};

const FREE: u32 = 0xFFFF_FFFF;
const END: u32 = 0xFFFF_FFFE;
const FATSECT: u32 = 0xFFFF_FFFD;
const DIFSECT: u32 = 0xFFFF_FFFC;
const MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

const AHEAD: u64 = 64 << 20;
const BEHIND: u64 = 32 << 20;
/// Cuts tried for one file.
const TRIES: u32 = 4000;

/// No idea where the fault lies.
const NOWHERE: u64 = u64::MAX;

fn le16(b: &[u8]) -> u64 {
    u16::from_le_bytes([b[0], b[1]]) as u64
}
fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn sector_at(s: u32) -> u64 {
    (s as u64 + 1) * SECTOR
}

/// What a fault can be blamed on, the cut being at `k`: a byte past it lies where the last piece
/// puts it, a byte of the first sector where the file starts; either is right. Faults are blamed
/// on the last byte below the cut among those they rest on, 0 when none is, so that no lower
/// cut can help.
fn low(k: u64, p: u64) -> u64 {
    if p == NOWHERE {
        NOWHERE
    } else if p >= k || p < SECTOR {
        0
    } else {
        p
    }
}

#[derive(Debug, PartialEq)]
pub enum Check {
    /// the file holds together: its length, its extension
    Good(u64, &'static str),
    /// something does not hold, in the byte at `.0` of the file or before (`NOWHERE` if unknown)
    Bad(u64),
    /// not a compound file of 512-byte sectors, or encrypted, or not checked here
    Unsupported,
}

struct Header {
    nfat: u32,
    dir: u32,
    mini_cutoff: u64,
    difat: u32,
    ndifat: u32,
    listed: Vec<u32>,
}

fn header(f: &mut Feed) -> Option<Header> {
    let h = f.bytes(0, 512)?;
    if h[..8] != MAGIC || le16(&h[30..]) != 9 {
        return None;
    }
    Some(Header {
        nfat: le32(&h[44..]),
        dir: le32(&h[48..]),
        mini_cutoff: le32(&h[56..]) as u64,
        difat: le32(&h[68..]),
        ndifat: le32(&h[72..]),
        listed: h[76..512].chunks(4).map(le32).collect(),
    })
}

/// Which sectors hold the FAT, from the header and the DIFAT chain.
fn fat_sectors(f: &mut Feed, h: &Header) -> Result<Vec<u32>, u64> {
    let mut secs: Vec<u32> = h.listed.iter().copied().filter(|&s| s != FREE).collect();
    let mut d = h.difat;
    let mut hops = 0;
    while (secs.len() as u64) < h.nfat as u64 && d < DIFSECT && hops <= h.ndifat {
        let b = f.bytes(sector_at(d), 512).ok_or(sector_at(d))?;
        secs.extend(b[..508].chunks(4).map(le32).filter(|&s| s != FREE));
        d = le32(&b[508..]);
        hops += 1;
    }
    if (secs.len() as u64) < h.nfat as u64 || h.nfat == 0 || h.nfat > 1 << 16 {
        return Err(NOWHERE);
    }
    secs.truncate(h.nfat as usize);
    Ok(secs)
}

/// Entries that can stand in a FAT of `n` entries.
fn fat_entries_ok(b: &[u8], n: u64) -> bool {
    b.chunks(4).map(le32).all(|e| (e as u64) < n || e >= DIFSECT)
}

struct Stream {
    name: String,
    size: u64,
    /// where each of its sectors lies in the file
    at: Vec<u64>,
}

impl Stream {
    fn read(&self, f: &mut Feed) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(self.size as usize);
        let mut buf = [0u8; 512];
        for &a in &self.at {
            if !f.read(a, &mut buf) {
                return None;
            }
            out.extend_from_slice(&buf);
        }
        out.truncate(self.size as usize);
        Some(out)
    }
    fn file_offset(&self, o: u64) -> u64 {
        self.at.get((o / SECTOR) as usize).map_or(NOWHERE, |a| a + o % SECTOR)
    }
}

/// The chain of sectors from `start`.
fn chain(fat: &[u32], fat_secs: &[u32], start: u32, max: usize, k: u64) -> Result<Vec<u32>, u64> {
    let mut out = Vec::new();
    let mut s = start;
    // a wrong link is blamed on the last of the FAT sectors the chain went through
    let holder = |s: u32| fat_secs.get(s as usize / 128).map_or(NOWHERE, |&x| low(k, sector_at(x) + SECTOR - 1));
    let mut blame = 0;
    while s != END {
        if s as usize >= fat.len() || out.len() > max {
            return Err(if out.is_empty() { NOWHERE } else { blame });
        }
        out.push(s);
        blame = blame.max(holder(s));
        s = fat[s as usize];
    }
    Ok(out)
}

/// Checks the compound file seen through `f`.
pub fn check(f: &mut Feed) -> Check {
    let Some(h) = header(f) else { return Check::Unsupported };
    let k = f.first_len();
    let fat_secs = match fat_sectors(f, &h) {
        Ok(s) => s,
        Err(p) => return Check::Bad(p),
    };
    let n = fat_secs.len() as u64 * 128;
    let mut fat = Vec::with_capacity(n as usize);
    for &x in &fat_secs {
        let Some(b) = f.bytes(sector_at(x), 512) else { return Check::Bad(low(k, sector_at(x) + SECTOR - 1)) };
        if !fat_entries_ok(&b, n) {
            return Check::Bad(low(k, sector_at(x) + SECTOR - 1));
        }
        fat.extend(b.chunks(4).map(le32));
    }
    for &x in &fat_secs {
        if fat.get(x as usize) != Some(&FATSECT) {
            return Check::Bad(fat_secs.get(x as usize / 128).map_or(NOWHERE, |&s| low(k, sector_at(s) + SECTOR - 1)));
        }
    }
    let Some(last) = fat.iter().rposition(|&e| e != FREE) else { return Check::Bad(NOWHERE) };
    let len = sector_at(last as u32 + 1);
    // the directory
    let dir = match chain(&fat, &fat_secs, h.dir, 1 << 16, k) {
        Ok(d) => d,
        Err(p) => return Check::Bad(p),
    };
    // what is read through the directory rests on its sectors, and on the FAT that places them
    let dir_end = dir.iter().map(|&d| low(k, sector_at(d) + SECTOR - 1)).max().unwrap_or(0);
    let fat_low = fat_secs.iter().map(|&x| low(k, sector_at(x) + SECTOR - 1)).max().unwrap_or(0);
    let dir_low = dir_end.max(fat_low);
    let mut streams = Vec::new();
    let mut root: Option<(u32, u64)> = None;
    for &d in &dir {
        let Some(b) = f.bytes(sector_at(d), 512) else { return Check::Bad(low(k, sector_at(d) + SECTOR - 1)) };
        for (i, e) in b.chunks(128).enumerate() {
            let typ = e[66];
            let name_len = le16(&e[64..]) as usize;
            if typ == 0 {
                continue;
            }
            if !matches!(typ, 1 | 2 | 5) || name_len > 64 || !name_len.is_multiple_of(2) || name_len < 2 {
                return Check::Bad(low(k, sector_at(d) + i as u64 * 128 + 127).max(fat_low));
            }
            let units: Vec<u16> = e[..name_len - 2].chunks(2).map(|u| le16(u) as u16).collect();
            let name = String::from_utf16_lossy(&units);
            let start = le32(&e[116..]);
            let size = le32(&e[120..]) as u64;
            if typ == 5 {
                root = Some((start, size));
            } else if typ == 2 {
                streams.push((name, start, size));
            }
        }
    }
    let Some(_) = root else { return Check::Bad(dir_low) };
    let mut ext = "ole";
    let mut big = Vec::new();
    for (name, start, size) in streams {
        if size < h.mini_cutoff {
            continue;
        }
        let secs = match chain(&fat, &fat_secs, start, (len / SECTOR) as usize, k) {
            Ok(s) => s,
            Err(p) => return Check::Bad(p.max(dir_low)),
        };
        if (secs.len() as u64) * SECTOR < size {
            return Check::Bad(NOWHERE);
        }
        match name.as_str() {
            "WordDocument" => ext = "doc",
            "Workbook" | "Book" if ext == "ole" => ext = "xls",
            "PowerPoint Document" => ext = "ppt",
            _ => {}
        }
        big.push(Stream { name, size, at: secs.iter().map(|&s| sector_at(s)).collect() });
    }
    // what the streams say of themselves
    for s in &big {
        let r = match s.name.as_str() {
            "Workbook" | "Book" => records(f, s, k),
            "WordDocument" => {
                let table = flags_table(f, s).and_then(|t| big.iter().find(|b| b.name == t));
                paragraphs(f, s, table, k)
            }
            _ => Ok(()),
        };
        if let Err(p) = r {
            return Check::Bad(p.max(dir_low));
        }
    }
    Check::Good(len, ext)
}

/// An Excel workbook (BIFF5 and 8): records of a type and a length, one after the other.
fn records(f: &mut Feed, s: &Stream, k: u64) -> Result<(), u64> {
    let data = s.read(f).ok_or(NOWHERE)?;
    let mut pos = 0usize;
    let mut first = true;
    while pos + 4 <= data.len() {
        let id = le16(&data[pos..]);
        let len = le16(&data[pos + 2..]) as usize;
        if id == 0 && len == 0 {
            // padding after the last record
            break;
        }
        let ok = len <= 8224 && id < 0x1200 && (!first || id == 0x0809 || id == 0x0009 || id == 0x0209 || id == 0x0409);
        if !ok || pos + 4 + len > data.len() {
            return Err(low(k, s.file_offset(pos as u64 + 3)).max(low(k, s.file_offset(pos as u64))));
        }
        first = false;
        pos += 4 + len;
    }
    Ok(())
}

/// Which table stream a Word document uses, from its FIB.
fn flags_table(f: &mut Feed, s: &Stream) -> Option<&'static str> {
    let fib = f.bytes(*s.at.first()?, 12)?;
    if !matches!(le16(&fib[0..]), 0xA5EC | 0xA5DC) {
        return None;
    }
    Some(if le16(&fib[10..]) & 0x0200 != 0 { "1Table" } else { "0Table" })
}

/// A Word document: every paragraph its formatting pages describe ends with a mark in the text.
fn paragraphs(f: &mut Feed, doc: &Stream, table: Option<&Stream>, k: u64) -> Result<(), u64> {
    let start = doc.file_offset(0);
    // what a fault is blamed on: the last byte of the file among those it rests on
    let fib_end = low(k, doc.file_offset(0x109));
    let fib = f.bytes(start, 0x10A).ok_or(fib_end)?;
    if !matches!(le16(&fib[0..]), 0xA5EC | 0xA5DC) {
        return Err(fib_end);
    }
    let flags = le16(&fib[10..]);
    // older than Word 97, or encrypted: nothing more to check
    if le16(&fib[2..]) < 0xC1 || flags & 0x0100 != 0 {
        return Ok(());
    }
    let Some(table) = table else { return Ok(()) };
    let (fc, lcb) = (le32(&fib[0x102..]) as u64, le32(&fib[0x106..]) as u64);
    if lcb < 12 || fc + lcb > table.size {
        return Ok(());
    }
    let t = table.read(f).ok_or(NOWHERE)?;
    let plc = &t[fc as usize..(fc + lcb) as usize];
    let n = ((lcb - 4) / 8) as usize;
    let text = doc.read(f).ok_or(NOWHERE)?;
    let mark = |b: u8| matches!(b, 0x0D | 0x07 | 0x0C);
    for i in 0..n {
        let entry = (n + 1) * 4 + i * 4;
        let entry_end = fib_end.max(low(k, table.file_offset(fc + entry as u64 + 3)));
        let pn = (le32(&plc[entry..]) & 0x3F_FFFF) as u64;
        let page_at = pn * SECTOR;
        if page_at + SECTOR > text.len() as u64 {
            return Err(entry_end);
        }
        let page_end = entry_end.max(low(k, doc.file_offset(page_at + SECTOR - 1)));
        let page = &text[page_at as usize..(page_at + SECTOR) as usize];
        let crun = page[511] as usize;
        if crun == 0 || crun > 0x1D {
            return Err(page_end);
        }
        let fcs: Vec<u64> = (0..=crun).map(|j| le32(&page[j * 4..]) as u64).collect();
        if fcs.windows(2).any(|w| w[1] <= w[0]) {
            return Err(page_end);
        }
        for &end in &fcs[1..] {
            let e = end as usize;
            if e < 2 || e > text.len() {
                return Err(page_end);
            }
            if !(mark(text[e - 1]) || (text[e - 1] == 0 && mark(text[e - 2]))) {
                return Err(page_end.max(low(k, doc.file_offset(end - 1))));
            }
        }
    }
    Ok(())
}

/// Reads a compound file that starts at `at` straight through.
pub fn measure(src: &mut dyn Source, at: u64) -> Check {
    let mut f = Feed::new(src, vec![(at, Kind::Ole.max_size())]);
    check(&mut f)
}

/// Where the FAT sector `fat_secs[j]`, which covers entries `j*128..`, could lie on the disk: sectors whose
/// entries are all valid and carry the marks of the FAT sectors in its range.
fn find_fat_sector(
    f: &mut Feed,
    near: u64,
    size: u64,
    j: usize,
    fat_secs: &[u32],
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
) -> Vec<u64> {
    let j = j as u32;
    let base = j * 128;
    let marks: Vec<usize> = fat_secs.iter().filter(|&&s| s / 128 == j).map(|&s| (s - base) as usize).collect();
    let n = fat_secs.len() as u64 * 128;
    let lo = near.saturating_sub(BEHIND);
    let hi = (near + AHEAD).min(size);
    let mut out = Vec::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut start = lo - lo % SECTOR;
    while start < hi {
        let got = f.disk_read(start, &mut buf);
        if got < 512 {
            break;
        }
        for (i, b) in buf[..got - got % 512].chunks(512).enumerate() {
            let d = start + i as u64 * SECTOR;
            if marks.iter().all(|&m| le32(&b[m * 4..]) == FATSECT) && fat_entries_ok(b, n) && claims.covering(d).is_none() && free(d) {
                out.push(d);
            }
        }
        start += got as u64 - got as u64 % 512;
    }
    // nearest the place it would have if the file were in one piece first
    out.sort_by_key(|&d| d.abs_diff(near));
    out.truncate(8);
    out
}

/// Puts back together a compound file whose parts stop agreeing before its end: a first piece,
/// and a last one placed by its FAT.
pub fn rebuild(
    src: &mut dyn Source,
    size: u64,
    at: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
) -> Option<(Vec<(u64, u64)>, &'static str)> {
    let first = frag::open_run(at, size, claims, Kind::Ole.max_size());
    let mut f = Feed::new(src, vec![first]);
    let h = header(&mut f)?;
    let fat_secs: Vec<u32> = h.listed.iter().copied().filter(|&s| s != FREE).take(h.nfat as usize).collect();
    if fat_secs.is_empty() || h.ndifat != 0 {
        return None;
    }
    // the FAT sector found last in the file among those that hold marks of FAT sectors: the
    // likeliest to lie in the last piece, and the easiest to tell from others
    let j = (0..fat_secs.len()).filter(|&j| fat_secs.iter().any(|&s| s as usize / 128 == j)).max_by_key(|&j| fat_secs[j])?;
    let x = fat_secs[j];
    let near = at + sector_at(x);
    let mut tries = 0u32;
    let anchors = find_fat_sector(&mut f, near, size, j, &fat_secs, claims, free);
    for d in anchors {
        let Some(delta) = d.checked_sub(sector_at(x)) else { continue };
        if delta <= at {
            // the last piece must start after the first one does
            continue;
        }
        // the cut: before the end of the first piece and before the FAT sector placed by `d`
        let top = first.1.min(sector_at(x));
        let mut k = top / SECTOR * SECTOR;
        while k >= SECTOR && tries < TRIES {
            tries += 1;
            let runs = vec![(at, k), (delta + k, Kind::Ole.max_size())];
            f.set_runs(runs.clone());
            match check(&mut f) {
                // and the last piece runs over no other file
                Check::Good(len, ext) if len > k && claims.next(delta + k).is_none_or(|c| c >= delta + len) => {
                    let runs = vec![(at, k), (delta + k, len - k)];
                    return Some((runs, ext));
                }
                Check::Bad(p) if p < k => k = (k - SECTOR).min(p / SECTOR * SECTOR),
                _ => k -= SECTOR,
            }
        }
    }
    None
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

    fn noise(n: usize, seed: u32) -> Vec<u8> {
        let mut x = seed.wrapping_mul(2654435761) | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect()
    }

    /// An Excel workbook: records of every length one after the other.
    fn workbook(seed: u32) -> Vec<u8> {
        let mut out = vec![0x09, 0x08, 16, 0];
        out.extend(noise(16, seed));
        let mut r = noise(4000, seed + 1);
        let mut i = 0;
        while out.len() < 40_000 {
            let len = (r[i % r.len()] as usize) * 3 % 700;
            out.extend((0x200u16 + r[(i + 1) % r.len()] as u16).to_le_bytes());
            out.extend((len as u16).to_le_bytes());
            let body = noise(len, seed + 2 + i as u32);
            out.extend(body);
            i += 2;
            r.rotate_left(1);
        }
        out.extend([0x0A, 0, 0, 0]);
        out
    }

    /// A compound file holding it: its sectors, then the directory, then the FAT, as writers lay
    /// them out.
    fn compound(data: &[u8]) -> Vec<u8> {
        let n = data.len().div_ceil(512) as u32;
        let (dir, fat_at) = (n, n + 1);
        let mut h = vec![0u8; 512];
        h[..8].copy_from_slice(&MAGIC);
        h[24..26].copy_from_slice(&0x3Eu16.to_le_bytes());
        h[26..28].copy_from_slice(&3u16.to_le_bytes());
        h[28..30].copy_from_slice(&0xFFFEu16.to_le_bytes());
        h[30..32].copy_from_slice(&9u16.to_le_bytes());
        h[32..34].copy_from_slice(&6u16.to_le_bytes());
        h[44..48].copy_from_slice(&1u32.to_le_bytes());
        h[48..52].copy_from_slice(&dir.to_le_bytes());
        h[56..60].copy_from_slice(&4096u32.to_le_bytes());
        h[60..64].copy_from_slice(&END.to_le_bytes());
        h[68..72].copy_from_slice(&END.to_le_bytes());
        for (i, c) in h[76..].chunks_mut(4).enumerate() {
            c.copy_from_slice(&(if i == 0 { fat_at } else { FREE }).to_le_bytes());
        }
        let mut out = h;
        out.extend(data);
        out.resize(512 * (n as usize + 1), 0);
        // the directory: the root, and the stream
        let mut d = vec![0u8; 512];
        let entry = |e: &mut [u8], name: &str, typ: u8, start: u32, size: u32| {
            let units: Vec<u16> = name.encode_utf16().chain([0]).collect();
            for (i, u) in units.iter().enumerate() {
                e[i * 2..i * 2 + 2].copy_from_slice(&u.to_le_bytes());
            }
            e[64..66].copy_from_slice(&((units.len() * 2) as u16).to_le_bytes());
            e[66] = typ;
            e[116..120].copy_from_slice(&start.to_le_bytes());
            e[120..124].copy_from_slice(&size.to_le_bytes());
        };
        entry(&mut d[..128], "Root Entry", 5, END, 0);
        entry(&mut d[128..256], "Workbook", 2, 0, data.len() as u32);
        out.extend(d);
        let mut fat: Vec<u32> = (1..=n).collect();
        fat[n as usize - 1] = END;
        fat.extend([END, FATSECT]);
        fat.resize(128, FREE);
        out.extend(fat.iter().flat_map(|e| e.to_le_bytes()));
        out
    }

    #[test]
    fn whole_workbook_holds_together() {
        let file = compound(&workbook(1));
        let mut disk = file.clone();
        disk.extend(noise(4096, 2));
        assert_eq!(measure(&mut Mem(disk), 0), Check::Good(file.len() as u64, "xls"));
    }

    #[test]
    fn workbook_in_two_pieces_comes_back() {
        let file = compound(&workbook(3));
        let cut = 30 * 512;
        // [first piece][noise][last piece][noise]
        let mut disk = file[..cut].to_vec();
        disk.extend(noise(25 * 512, 4));
        let second_at = disk.len() as u64;
        disk.extend(&file[cut..]);
        disk.extend(noise(8 * 512, 5));
        let size = disk.len() as u64;
        let mut src = Mem(disk);
        assert!(matches!(measure(&mut src, 0), Check::Bad(_)));
        let (runs, ext) = rebuild(&mut src, size, 0, &Claims::default(), &|_| true).unwrap();
        assert_eq!((runs, ext), (vec![(0, cut as u64), (second_at, (file.len() - cut) as u64)], "xls"));
    }
}
