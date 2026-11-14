//! ZIP files (and the formats built on them: Office, OpenDocument, EPUB, JAR...) cut in pieces.
//!
//! Each entry carries the CRC of its content, so inflating it tells whether its bytes are all
//! there, and where the first wrong one is, give or take a few hundred bytes. The end record says
//! where the central directory starts in the file: found on the disk, it places the last piece of
//! the file exactly. What is left is the cut between the first piece and that last one, searched
//! from the top down, each failure pulling the bound down to where the data first went wrong.

use crate::carve::{Kind, Source};
use crate::frag::{self, Claims, Feed, SECTOR};
use miniz_oxide::inflate::stream::{InflateState, inflate};
use miniz_oxide::{DataFormat, MZError, MZFlush, MZStatus};

#[derive(Debug, PartialEq)]
pub enum Walk {
    /// read to the end record: where the file ends
    Done(u64),
    /// the record starting at `.0` does not check; the first wrong byte is at or before `.1`
    Broken(u64, u64),
    /// encrypted, or compressed in a way not checked here
    Unsupported,
}

/// How far from the first piece the end record is looked for.
const AHEAD: u64 = 64 << 20;
const BEHIND: u64 = 32 << 20;

fn le16(b: &[u8]) -> u64 {
    u16::from_le_bytes([b[0], b[1]]) as u64
}
fn le32(b: &[u8]) -> u64 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as u64
}

enum Entry {
    Good(u64),
    Bad(u64),
    Unsupported,
}

/// Inflates from `at`: where the stream ends, the CRC and size of what came out; or where it
/// went wrong.
fn inflate_at(f: &mut Feed, at: u64, csize: Option<u64>) -> Result<(u64, u32, u64), u64> {
    let mut st = InflateState::new_boxed(DataFormat::Raw);
    let mut out = vec![0u8; 64 << 10];
    let mut inbuf = vec![0u8; 16 << 10];
    let mut crc = crc32fast::Hasher::new();
    let (mut pos, mut written) = (at, 0u64);
    loop {
        let want = match csize {
            Some(c) => (at + c - pos).min(inbuf.len() as u64) as usize,
            None => inbuf.len(),
        };
        let n = f.read_some(pos, &mut inbuf[..want]);
        if n == 0 {
            return Err(pos);
        }
        let mut inp = &inbuf[..n];
        loop {
            let r = inflate(&mut st, inp, &mut out, MZFlush::None);
            crc.update(&out[..r.bytes_written]);
            written += r.bytes_written as u64;
            inp = &inp[r.bytes_consumed..];
            pos += r.bytes_consumed as u64;
            match r.status {
                Ok(MZStatus::StreamEnd) => return Ok((pos, crc.finalize(), written)),
                Ok(_) | Err(MZError::Buf) => {}
                Err(_) => return Err(pos),
            }
            if (inp.is_empty() && r.bytes_written < out.len()) || (r.bytes_consumed == 0 && r.bytes_written == 0) {
                break;
            }
        }
    }
}

/// The entry whose local header is at `at`.
fn entry(f: &mut Feed, at: u64) -> Entry {
    let Some(h) = f.bytes(at, 30) else { return Entry::Bad(at) };
    if &h[..4] != b"PK\x03\x04" {
        return Entry::Bad(at);
    }
    let flags = le16(&h[6..]);
    let method = le16(&h[8..]);
    let (crc, csize, usize) = (le32(&h[14..]) as u32, le32(&h[18..]), le32(&h[22..]));
    let data = at + 30 + le16(&h[26..]) + le16(&h[28..]);
    let later = flags & 8 != 0;
    // encrypted, or sizes in a ZIP64 extra field
    if flags & 1 != 0 || (!later && (csize == 0xFFFF_FFFF || usize == 0xFFFF_FFFF)) {
        return Entry::Unsupported;
    }
    let (end, got_crc, got_size) = match method {
        0 if !later => {
            let mut c = crc32fast::Hasher::new();
            let mut buf = vec![0u8; 64 << 10];
            let mut pos = data;
            while pos < data + csize {
                let n = (data + csize - pos).min(buf.len() as u64) as usize;
                if !f.read(pos, &mut buf[..n]) {
                    return Entry::Bad(pos);
                }
                c.update(&buf[..n]);
                pos += n as u64;
            }
            (data + csize, c.finalize(), csize)
        }
        8 => match inflate_at(f, data, (!later).then_some(csize)) {
            Ok(r) => r,
            Err(p) => return Entry::Bad(p),
        },
        _ => return Entry::Unsupported,
    };
    if !later {
        return if end == data + csize && got_crc == crc && got_size == usize { Entry::Good(end) } else { Entry::Bad(end) };
    }
    // sizes and CRC in a descriptor after the data, its signature optional
    let Some(d) = f.bytes(end, 16) else { return Entry::Bad(end) };
    let (d, len) = if &d[..4] == b"PK\x07\x08" { (&d[4..], 16) } else { (&d[..], 12) };
    if le32(d) as u32 == got_crc && le32(&d[8..]) == got_size & 0xFFFF_FFFF { Entry::Good(end + len) } else { Entry::Bad(end) }
}

pub fn walk(f: &mut Feed, from: u64) -> Walk {
    let mut pos = from;
    loop {
        let Some(sig) = f.bytes(pos, 4) else { return Walk::Broken(pos, pos) };
        match &sig[..] {
            b"PK\x03\x04" => match entry(f, pos) {
                Entry::Good(end) => pos = end,
                Entry::Bad(p) => return Walk::Broken(pos, p),
                Entry::Unsupported => return Walk::Unsupported,
            },
            b"PK\x01\x02" => {
                let Some(h) = f.bytes(pos, 46) else { return Walk::Broken(pos, pos) };
                pos += 46 + le16(&h[28..]) + le16(&h[30..]) + le16(&h[32..]);
            }
            b"PK\x06\x06" => {
                let Some(h) = f.bytes(pos, 12) else { return Walk::Broken(pos, pos) };
                pos += 12 + u64::from_le_bytes(h[4..12].try_into().unwrap());
            }
            b"PK\x06\x07" => pos += 20,
            b"PK\x05\x05" => {
                let Some(h) = f.bytes(pos, 6) else { return Walk::Broken(pos, pos) };
                pos += 6 + le16(&h[4..]);
            }
            b"PK\x05\x06" => {
                let Some(h) = f.bytes(pos, 22) else { return Walk::Broken(pos, pos) };
                return Walk::Done(pos + 22 + le16(&h[20..]));
            }
            _ => return Walk::Broken(pos, pos),
        }
    }
}

/// Reads a ZIP that starts at `at` straight through.
pub fn measure(src: &mut dyn Source, at: u64) -> Walk {
    let mut f = Feed::new(src, vec![(at, Kind::Zip.max_size())]);
    walk(&mut f, 0)
}

/// The end record of this file on the disk near `near`: where it lies, and its offset in the
/// file. Its central directory must open with the file's first entry, at offset 0.
fn find_end(f: &mut Feed, near: u64, size: u64, name: &[u8], claims: &Claims) -> Option<(u64, u64, u64)> {
    let lo = near.saturating_sub(BEHIND);
    let hi = (near + AHEAD).min(size);
    let check = |f: &mut Feed, p: u64| -> Option<(u64, u64, u64)> {
        let e = disk(f, p, 22)?;
        let (cd_size, cd_start) = (le32(&e[12..]), le32(&e[16..]));
        let end_file = cd_start.checked_add(cd_size)?;
        let cd_disk = p.checked_sub(cd_size)?;
        let c = disk(f, cd_disk, 46 + name.len())?;
        let ok = &c[..4] == b"PK\x01\x02"
            && le32(&c[42..]) == 0
            && le16(&c[28..]) as usize == name.len()
            && &c[46..] == name
            && claims.covering(p).is_none();
        ok.then_some((p, end_file, end_file + 22 + le16(&e[20..])))
    };
    // nearest first, ahead and behind in turn
    const STEP: u64 = 1 << 20;
    let mut buf = vec![0u8; (STEP + 3) as usize];
    for i in 0.. {
        let ahead = near + i * STEP;
        // the window that ends `i` steps behind, clipped at the low bound
        let behind = (near > lo + i * STEP).then(|| near.saturating_sub((i + 1) * STEP).max(lo));
        if ahead >= hi && behind.is_none() {
            return None;
        }
        for start in [Some(ahead).filter(|&a| a < hi), behind].into_iter().flatten() {
            let n = f.disk_bytes(start, &mut buf);
            for j in memchr_all(&buf[..n], b"PK\x05\x06") {
                if let Some(r) = check(f, start + j as u64) {
                    return Some(r);
                }
            }
        }
    }
    None
}

fn memchr_all<'a>(hay: &'a [u8], pat: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
    hay.windows(pat.len()).enumerate().filter(move |(_, w)| *w == pat).map(|(i, _)| i)
}

fn disk(f: &mut Feed, at: u64, n: usize) -> Option<Vec<u8>> {
    let mut b = vec![0u8; n];
    (f.disk_bytes(at, &mut b) == n).then_some(b)
}

/// Puts back together a ZIP whose entries stop checking before its end: a first piece, and a
/// last one placed by the end record.
pub fn rebuild(src: &mut dyn Source, size: u64, at: u64, claims: &Claims) -> Option<Vec<(u64, u64)>> {
    let first = frag::open_run(at, size, claims, Kind::Zip.max_size());
    let mut f = Feed::new(src, vec![first]);
    let (e0, mut bad) = match walk(&mut f, 0) {
        Walk::Done(end) => return Some(frag::cut(&[first], end)),
        Walk::Broken(e0, p) => (e0, p),
        Walk::Unsupported => return None,
    };
    let h = f.bytes(0, 30)?;
    let name = f.bytes(30, le16(&h[26..]) as usize)?;
    let (end_disk, end_off, file_len) = find_end(&mut f, at + first.1, size, &name, claims)?;
    if end_disk < end_off || !(end_disk - end_off).is_multiple_of(SECTOR) || end_disk < at {
        return None;
    }
    let delta = end_disk - end_off;
    // the cut: past what reads well, before the end of the first piece, of the end record, and
    // of where the data first went wrong; tried from the top down
    let low = e0.max(1).next_multiple_of(SECTOR);
    let mut k = first.1.min(end_off).min(bad) / SECTOR * SECTOR;
    while k >= low {
        let runs = vec![(at, k), (k + delta, file_len - k)];
        f.set_runs(runs.clone());
        match walk(&mut f, e0) {
            Walk::Done(end) if end == file_len => return Some(runs),
            // wrong already in the first piece: the cut is no later than that
            Walk::Broken(_, p) if p < k => bad = p,
            _ => {}
        }
        k = (k - SECTOR).min(bad / SECTOR * SECTOR);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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

    fn archive(seed: u32) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        let mut z = zip::ZipWriter::new(&mut out);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for i in 0..6u32 {
            z.start_file(format!("part{i}.txt"), opts).unwrap();
            // text that compresses some, so the stream has room to go wrong
            let words: Vec<String> = noise(6000, seed + i).iter().map(|b| format!("w{} ", b % 97)).collect();
            z.write_all(words.concat().as_bytes()).unwrap();
        }
        z.finish().unwrap();
        out.into_inner()
    }

    #[test]
    fn whole_archive_reads_to_its_end() {
        let a = archive(1);
        let mut disk = a.clone();
        disk.extend(noise(3000, 2));
        assert_eq!(measure(&mut Mem(disk), 0), Walk::Done(a.len() as u64));
    }

    #[test]
    fn archive_in_two_pieces_comes_back() {
        let a = archive(3);
        let cut = (a.len() / 2 / 512) * 512;
        let mut disk = a[..cut].to_vec();
        let other = disk.len() as u64;
        disk.extend(noise(70 * 512, 4));
        let second = disk.len() as u64;
        disk.extend(&a[cut..]);
        disk.extend(noise(5000, 5));
        let size = disk.len() as u64;
        let mut claims = Claims::default();
        claims.add(other, other + 512);
        let mut src = Mem(disk);
        assert!(matches!(measure(&mut src, 0), Walk::Broken(..)));
        let runs = rebuild(&mut src, size, 0, &claims).unwrap();
        assert_eq!(runs, vec![(0, cut as u64), (second, (a.len() - cut) as u64)]);
    }

    #[test]
    fn archive_whose_first_piece_runs_on_comes_back() {
        // nothing claimed after the first piece: the cut is found from the data alone
        let a = archive(6);
        let cut = (a.len() / 3 / 512) * 512;
        let mut disk = a[..cut].to_vec();
        disk.extend(noise(90 * 512, 7));
        let second = disk.len() as u64;
        disk.extend(&a[cut..]);
        let size = disk.len() as u64;
        let mut src = Mem(disk);
        let runs = rebuild(&mut src, size, 0, &Claims::default()).unwrap();
        assert_eq!(runs, vec![(0, cut as u64), (second, (a.len() - cut) as u64)]);
    }
}
