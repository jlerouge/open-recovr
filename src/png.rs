//! PNG files cut in pieces.
//!
//! A PNG is a chain of chunks, each one closed by a CRC of its type and data. Writers split the
//! image data into chunks of a few kilobytes, so a cut always falls inside a chunk that then fails
//! its CRC: the bytes before that chunk are known good, the cut is one of the few sector bounds
//! inside it. A next piece is accepted only when that chunk's CRC comes out right through it and
//! the chunk after reads as one: a wrong piece passes that with odds of one in four billion.

use crate::carve::{Kind, Source};
use crate::frag::{self, Candidates, Claims, Feed, SECTOR};

/// How the chain of chunks reads from a point.
#[derive(Debug, PartialEq)]
pub enum Walk {
    /// up to the end chunk: where the file ends
    Done(u64),
    /// up to the chunk that starts here, which does not check
    Broken(u64),
}

/// Biggest chunk whose sectors are all tried as the cut.
const CUTS_WITHIN: u64 = 128 << 10;
/// Hypotheses tried for one cut before giving up.
const BUDGET: u64 = 4_000_000;

fn header(f: &mut Feed, at: u64, max: u64) -> Option<(u64, [u8; 4])> {
    let h = f.bytes(at, 8)?;
    let len = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as u64;
    let typ = [h[4], h[5], h[6], h[7]];
    (typ.iter().all(|b| b.is_ascii_alphabetic()) && len <= max.min(1 << 31)).then_some((len, typ))
}

/// The chunk at `at`, if its CRC is right: where it ends, and its type.
fn chunk(f: &mut Feed, at: u64, max: u64) -> Option<(u64, [u8; 4])> {
    let (len, typ) = header(f, at, max)?;
    let mut crc = crc32fast::Hasher::new();
    crc.update(&typ);
    let mut buf = vec![0u8; 64 << 10];
    let mut pos = at + 8;
    let end = pos + len;
    while pos < end {
        let n = (end - pos).min(buf.len() as u64) as usize;
        if !f.read(pos, &mut buf[..n]) {
            return None;
        }
        crc.update(&buf[..n]);
        pos += n as u64;
    }
    let stored = f.bytes(end, 4)?;
    (crc.finalize() == u32::from_be_bytes([stored[0], stored[1], stored[2], stored[3]])).then_some((end + 4, typ))
}

pub fn walk(f: &mut Feed, from: u64, max: u64) -> Walk {
    let mut pos = from;
    loop {
        match chunk(f, pos, max) {
            Some((end, typ)) if &typ == b"IEND" => return Walk::Done(end),
            Some((end, _)) => pos = end,
            None => return Walk::Broken(pos),
        }
    }
}

/// Reads a PNG that starts at `at` straight through. `None` when not even its first chunk checks.
pub fn measure(src: &mut dyn Source, at: u64) -> Option<Walk> {
    let max = Kind::Png.max_size();
    let mut f = Feed::new(src, vec![(at, max)]);
    match walk(&mut f, 8, max) {
        Walk::Broken(8) => None,
        w => Some(w),
    }
}

/// Puts back together a PNG whose chunks stop checking before its end.
pub fn rebuild(
    src: &mut dyn Source,
    size: u64,
    at: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
    max_pieces: usize,
) -> Option<Vec<(u64, u64)>> {
    let max = Kind::Png.max_size();
    let first = frag::open_run(at, size, claims, max);
    let mut f = Feed::new(src, vec![first]);
    let mut runs = vec![first];
    let mut from = 8;
    loop {
        let c0 = match walk(&mut f, from, max) {
            Walk::Done(end) => return Some(frag::cut(&runs, end)),
            Walk::Broken(c0) => c0,
        };
        if runs.len() >= max_pieces {
            return None;
        }
        let last_start: u64 = runs[..runs.len() - 1].iter().map(|r| r.1).sum();
        let total: u64 = runs.iter().map(|r| r.1).sum();
        // everything before the chunk that fails is good; the cut lies inside it, or inside its
        // header when even that does not read
        let upper = match header(&mut f, c0, max) {
            Some((len, _)) if len <= CUTS_WITHIN => c0 + 12 + len,
            Some(_) => return None,
            None => c0 + 8,
        }
        .min(total);
        let low = c0.max(last_start + 1).next_multiple_of(SECTOR);
        let cuts: Vec<u64> = (low..=upper).step_by(SECTOR as usize).collect();
        let k0 = *cuts.first()?;
        let own = frag::cut(&runs, k0);
        let origin = f.locate(k0 - 1)? + 1;
        let mut cands = Candidates::new(origin, size);
        let mut tried = 0u64;
        let mut found = None;
        'search: while let Some(s) = cands.next(claims, &own, free) {
            for &k in &cuts {
                tried += 1;
                let mut hyp = frag::cut(&runs, k);
                hyp.push(frag::open_run(s, size, claims, max));
                f.set_runs(hyp);
                // cheap first: the chunk's header, and the next one's
                let Some((len, _)) = header(&mut f, c0, max) else { continue };
                if header(&mut f, c0 + 12 + len, max).is_none() && f.bytes(c0 + 12 + len, 1).is_some() {
                    continue;
                }
                if chunk(&mut f, c0, max).is_some() {
                    found = Some((k, s));
                    break 'search;
                }
            }
            if tried >= BUDGET {
                break;
            }
        }
        let (k, s) = found?;
        runs = frag::cut(&runs, k);
        runs.push(frag::open_run(s, size, claims, max));
        f.set_runs(runs.clone());
        from = c0;
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

    /// A PNG of noise, its data split in chunks of 8 KB the way libpng writes them.
    fn picture(w: u32, h: u32, seed: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png_encoder(&mut out, w, h);
        enc.set_chunk(8192);
        enc.write(&noise((w * h * 3) as usize, seed));
        out
    }

    struct Enc<'a> {
        out: &'a mut Vec<u8>,
        w: u32,
        h: u32,
        size: usize,
    }
    fn png_encoder(out: &mut Vec<u8>, w: u32, h: u32) -> Enc<'_> {
        Enc { out, w, h, size: 8192 }
    }
    impl Enc<'_> {
        fn set_chunk(&mut self, n: usize) {
            self.size = n;
        }
        fn put(&mut self, typ: &[u8; 4], data: &[u8]) {
            self.out.extend((data.len() as u32).to_be_bytes());
            self.out.extend(typ);
            self.out.extend(data);
            let mut c = crc32fast::Hasher::new();
            c.update(typ);
            c.update(data);
            self.out.extend(c.finalize().to_be_bytes());
        }
        fn write(&mut self, rgb: &[u8]) {
            self.out.extend(b"\x89PNG\r\n\x1a\n");
            let mut ihdr = Vec::new();
            ihdr.extend(self.w.to_be_bytes());
            ihdr.extend(self.h.to_be_bytes());
            ihdr.extend([8, 2, 0, 0, 0]);
            self.put(b"IHDR", &ihdr);
            let mut raw = Vec::new();
            for row in rgb.chunks((self.w * 3) as usize) {
                raw.push(0);
                raw.extend(row);
            }
            let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 1);
            for part in z.chunks(self.size) {
                self.put(b"IDAT", part);
            }
            self.put(b"IEND", &[]);
        }
    }

    fn pad(v: &mut Vec<u8>) {
        v.resize(v.len().next_multiple_of(512), 0);
    }

    #[test]
    fn whole_picture_reads_to_its_end() {
        let p = picture(120, 90, 1);
        let mut disk = p.clone();
        disk.extend(noise(4096, 9));
        assert_eq!(measure(&mut Mem(disk), 0), Some(Walk::Done(p.len() as u64)));
    }

    #[test]
    fn picture_in_two_pieces_with_one_before_comes_back() {
        let p = picture(200, 150, 2);
        let cut = 20 * 512;
        // [noise][second piece][noise][first piece][noise]
        let mut disk = noise(8 * 512, 3);
        let second_at = disk.len() as u64;
        disk.extend(&p[cut..]);
        pad(&mut disk);
        disk.extend(noise(30 * 512, 4));
        let first_at = disk.len() as u64;
        disk.extend(&p[..cut]);
        disk.extend(noise(40 * 512, 5));
        let size = disk.len() as u64;
        let mut src = Mem(disk);
        assert!(matches!(measure(&mut src, first_at), Some(Walk::Broken(_))));
        let runs = rebuild(&mut src, size, first_at, &Claims::default(), &|_| true, 8).unwrap();
        assert_eq!(runs, vec![(first_at, cut as u64), (second_at, (p.len() - cut) as u64)]);
    }

    #[test]
    fn another_picture_in_the_way_is_not_taken() {
        let p = picture(200, 150, 6);
        let q = picture(200, 150, 7);
        let cut = 31 * 512;
        let mut disk = p[..cut].to_vec();
        let q_at = disk.len() as u64;
        disk.extend(&q);
        pad(&mut disk);
        let second_at = disk.len() as u64;
        disk.extend(&p[cut..]);
        disk.extend(noise(4096, 8));
        let size = disk.len() as u64;
        let mut src = Mem(disk);
        let mut claims = Claims::default();
        claims.add(q_at, q_at + 512);
        let runs = rebuild(&mut src, size, 0, &claims, &|_| true, 8).unwrap();
        assert_eq!(runs, vec![(0, cut as u64), (second_at, (p.len() - cut) as u64)]);
    }
}
