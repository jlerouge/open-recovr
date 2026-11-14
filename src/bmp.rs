//! BMP files cut in pieces.
//!
//! An uncompressed bitmap has no checksum and no markers: its pixels are all the structure it
//! has. The rows of a photograph change little from one to the next, so each byte is close to the
//! byte one row above it. Where a piece ends, the bytes that follow come from some other file and
//! that closeness breaks, all at once, at the start of a sector. The next piece is the free sector
//! whose bytes, put after the cut, carry on the rows above as smoothly as the picture did before
//! it, and keep reading as a picture for a while after.

use std::collections::VecDeque;

use crate::carve::{Kind, Source};
use crate::frag::{self, Candidates, Claims, Feed, SECTOR};

/// A sector whose bytes stray from the row above more than this many times the picture's usual,
/// plus `ROUGH_C`, breaks off from it.
const ROUGH_K: f32 = 3.0;
const ROUGH_C: f32 = 12.0;
/// A join as smooth as this, against the rows before it, is taken unless a smoother one turns up
/// within `EXTRA` more tries.
const SMOOTH_K: f32 = 1.5;
const SMOOTH_C: f32 = 4.0;
const EXTRA: u32 = 256;
/// Candidates tried for a next piece at most.
const TRIES: u32 = 40_000;
/// Sectors whose usual gap makes the baseline a sector is judged against.
const RECENT: usize = 16;
/// Bytes a next piece must read on as a picture before it is believed.
const TRIAL: u64 = 16 << 10;
/// Pictures whose rows stray from each other more than this on the whole are not photographs:
/// nothing can be told from their pixels.
const NOT_A_PHOTO: f32 = 40.0;

/// What the header tells: where the pixels start, how long a row is, how long the file is, and
/// a value for each byte that rows can be compared on.
pub(crate) struct Pic {
    data: u64,
    stride: u64,
    len: u64,
    lut: [u8; 256],
}

impl Pic {
    /// Uncompressed rows of bytes, each compared as is with the byte one row above: pixels
    /// from `data` to `len`, `stride` bytes a row.
    pub(crate) fn raw(data: u64, stride: u64, len: u64) -> Self {
        let mut lut = [0u8; 256];
        for (i, v) in lut.iter_mut().enumerate() {
            *v = i as u8;
        }
        Self { data, stride, len, lut }
    }

    /// Where the first byte that has a row above it lies.
    fn first(&self) -> u64 {
        self.data + self.stride
    }
}

fn le16(b: &[u8], i: usize) -> u64 {
    u16::from_le_bytes([b[i], b[i + 1]]) as u64
}
fn le32(b: &[u8], i: usize) -> u64 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as u64
}

/// Reads the header of the bitmap the feed starts with; `None` for those whose pixels cannot be
/// compared row to row (compressed, fewer than 8 bits per pixel, 16 bits).
fn pic(f: &mut Feed, max: u64) -> Option<Pic> {
    let h = f.bytes(0, 54)?;
    if &h[..2] != b"BM" {
        return None;
    }
    let size = le32(&h, 2);
    let data = le32(&h, 10);
    let dib = le32(&h, 14);
    let (w, ht, bpp, comp, colors) = if dib == 12 {
        (le16(&h, 18), le16(&h, 20), le16(&h, 24), 0, 0)
    } else {
        let ht = i32::from_le_bytes([h[22], h[23], h[24], h[25]]).unsigned_abs() as u64;
        (le32(&h, 18), ht, le16(&h, 28), le32(&h, 30), le32(&h, 46))
    };
    let ok = match bpp {
        8 | 24 => comp == 0,
        32 => matches!(comp, 0 | 3 | 6),
        _ => false,
    };
    if !ok || w == 0 || ht == 0 || w > 1 << 16 || ht > 1 << 16 || data < 14 + dib || data > 1 << 20 {
        return None;
    }
    let stride = (w * bpp).div_ceil(32) * 4;
    let mut len = data + stride * ht;
    // some writers pad the file a little past its pixels, and say so
    if size > len && size <= len + 1024 {
        len = size;
    }
    if len > max || stride < 48 {
        return None;
    }
    let mut lut = [0u8; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        *v = i as u8;
    }
    if bpp == 8 {
        // palette indices, compared on the brightness of their colours
        let entry = if dib == 12 { 3 } else { 4 };
        let n = if colors == 0 || colors > 256 { 256 } else { colors };
        let at = 14 + dib;
        if at + n * entry > data {
            return None;
        }
        let pal = f.bytes(at, (n * entry) as usize)?;
        let entry = entry as usize;
        for i in 0..n as usize {
            let (b, g, r) = (pal[i * entry] as u32, pal[i * entry + 1] as u32, pal[i * entry + 2] as u32);
            lut[i] = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
        }
    }
    Some(Pic { data, stride, len, lut })
}

/// Mean gap between the bytes of file range `a..b` and those one row above, `buf` holding the
/// file from `base`.
fn gap(buf: &[u8], base: u64, a: u64, b: u64, p: &Pic) -> f32 {
    let s = p.stride as usize;
    let (i, j) = ((a - base) as usize, (b - base) as usize);
    if j <= i {
        return 0.0;
    }
    let lut = &p.lut;
    let sum: u32 = (i..j).map(|x| (lut[buf[x] as usize] as i32 - lut[buf[x - s] as usize] as i32).unsigned_abs()).sum();
    sum as f32 / (j - i) as f32
}

/// The gap over file range `k..k + n` with the pieces the feed shows: `None` when they do not
/// reach that far.
fn score(f: &mut Feed, p: &Pic, k: u64, n: u64) -> Option<f32> {
    let from = k.checked_sub(p.stride)?;
    let mut buf = vec![0u8; (n + p.stride) as usize];
    f.read(from, &mut buf).then(|| gap(&buf, from, k, k + n, p))
}

fn median(v: &VecDeque<f32>) -> f32 {
    let mut s: Vec<f32> = v.iter().copied().collect();
    s.sort_by(f32::total_cmp);
    s[s.len() / 2]
}

fn rough(g: f32, base: f32) -> bool {
    g > ROUGH_K * base + ROUGH_C
}

enum Scan {
    /// the rows carry on up to the end asked for: their mean gap
    Clean(f32),
    /// the sector that starts here breaks off from the picture
    Fault(u64),
    /// the pieces end here first
    Short(u64),
}

/// Whether the rough sector at `k` is only a hard line across the picture: one row further on,
/// the rows read smoothly again against each other. Where a piece ends, the bytes after it come
/// from another file and stay rough against the rows above them.
fn edge(f: &mut Feed, p: &Pic, k: u64, base: f32) -> bool {
    let at = k + p.stride;
    if at + SECTOR > p.len {
        return false;
    }
    // rough from the very first bytes of the sector, and over a whole row on: another piece of
    // the same picture, rows as long, put there by the cut. A line in the picture seldom starts
    // right on a sector, and seldom runs straight across all of it.
    let n = 64.min(p.stride);
    let from = k.saturating_sub(p.stride + n);
    let mut row = vec![0u8; (at - from) as usize];
    if from + p.stride + n == k && f.read(from, &mut row) {
        let (tail, head) = (gap(&row, from, k - n, k, p), gap(&row, from, k, k + n, p));
        let whole = (k..at).step_by(SECTOR as usize).all(|a| rough(gap(&row, from, a, (a + SECTOR).min(at), p), base));
        if whole && head > 2.0 * tail + 4.0 {
            return false;
        }
    }
    let mut buf = vec![0u8; (SECTOR + p.stride) as usize];
    if !f.read(k, &mut buf) {
        return false;
    }
    varied(&buf[p.stride as usize..]) && !rough(gap(&buf, k, at, at + SECTOR, p), base)
}

/// How the rows read on through `from..to`, each sector against the ones just before it (from
/// before `from` too, so a scan may start anywhere).
fn scan(f: &mut Feed, p: &Pic, from: u64, to: u64) -> Scan {
    const BLOCK: u64 = 1 << 20;
    let first = p.first().next_multiple_of(SECTOR);
    let mut recent: VecDeque<f32> = VecDeque::with_capacity(RECENT + 1);
    let mut m = from.saturating_sub(RECENT as u64 * SECTOR).max(first);
    let (mut sum, mut n) = (0f64, 0u64);
    let mut buf = Vec::new();
    while m < to {
        let want = (m + BLOCK).min(to);
        let base = m - p.stride;
        buf.resize((want - base) as usize, 0);
        let have = base + f.read_some(base, &mut buf) as u64;
        let end = want.min(have);
        let mut k = m;
        while k < end {
            let e = (k + SECTOR).min(end);
            let g = gap(&buf, base, k, e, p);
            if k >= from && !recent.is_empty() && rough(g, median(&recent)) {
                if !edge(f, p, k, median(&recent)) {
                    return Scan::Fault(k);
                }
                // a hard line in the picture: kept out of the baseline
                sum += g as f64 * (e - k) as f64;
                n += e - k;
                k = e;
                continue;
            }
            recent.push_back(g);
            if recent.len() > RECENT {
                recent.pop_front();
            }
            sum += g as f64 * (e - k) as f64;
            n += e - k;
            k = e;
        }
        if end < want {
            return Scan::Short(end.max(from));
        }
        m = end;
    }
    Scan::Clean(if n == 0 { 0.0 } else { (sum / n as f64) as f32 })
}

/// How a bitmap that starts at `at` reads straight through.
pub enum Check {
    /// as one picture to its end: its length
    Good(u64),
    /// its rows break off before their end
    Bad,
    /// nothing can be told from its pixels
    Unsupported,
}

pub fn measure(src: &mut dyn Source, at: u64) -> Check {
    let max = Kind::Bmp.max_size();
    let mut f = Feed::new(src, vec![(at, max)]);
    let Some(p) = pic(&mut f, max) else { return Check::Unsupported };
    check(&mut f, &p)
}

/// How the rows of `p` read straight through the feed: `Good` with the end of the pixels.
pub(crate) fn check(f: &mut Feed, p: &Pic) -> Check {
    match scan(f, p, p.first().next_multiple_of(SECTOR), p.len) {
        Scan::Clean(g) if g <= NOT_A_PHOTO => Check::Good(p.len),
        Scan::Clean(_) => Check::Unsupported,
        Scan::Fault(_) | Scan::Short(_) => Check::Bad,
    }
}

/// Whether a sector holds more than a flat run of one or two values: those match any flat part
/// of any picture.
fn varied(s: &[u8]) -> bool {
    let mut seen = [false; 256];
    let mut n = 0;
    for &b in s {
        if !seen[b as usize] {
            seen[b as usize] = true;
            n += 1;
            if n >= 4 {
                return true;
            }
        }
    }
    false
}

/// Puts back together a bitmap whose rows break off before their end.
pub fn rebuild(
    src: &mut dyn Source,
    size: u64,
    at: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
    max_pieces: usize,
) -> Option<Vec<(u64, u64)>> {
    let max = Kind::Bmp.max_size();
    let first = frag::open_run(at, size, claims, max);
    let mut f = Feed::new(src, vec![first]);
    let p = pic(&mut f, max)?;
    let end = p.len;
    rows(f, &p, Fit { size, claims, free, max_pieces, max, end, anchor: None })
}

/// What a rebuild from rows works with: the disk, what the file may take of it, where the file
/// ends past its pixels, and a byte of its last piece already found (its place in the file, its
/// place on the disk), if one is.
pub(crate) struct Fit<'c> {
    pub size: u64,
    pub claims: &'c Claims,
    pub free: &'c dyn Fn(u64) -> bool,
    pub max_pieces: usize,
    pub max: u64,
    pub end: u64,
    pub anchor: Option<(u64, u64)>,
}

/// Puts back together, from the first piece the feed holds, a picture of uncompressed rows that
/// break off before their end.
pub(crate) fn rows(mut f: Feed, p: &Pic, fit: Fit) -> Option<Vec<(u64, u64)>> {
    let Fit { size, claims, free, max_pieces, max, end, anchor } = fit;
    let mut runs = f.runs().to_vec();
    let mut from = p.first().next_multiple_of(SECTOR);
    let mut sector = vec![0u8; SECTOR as usize];
    loop {
        let k = match scan(&mut f, p, from, p.len) {
            Scan::Clean(g) if g <= NOT_A_PHOTO => return Some(frag::cut(&runs, end)),
            Scan::Clean(_) => return None,
            Scan::Fault(k) => k,
            Scan::Short(e) => e - e % SECTOR,
        };
        if runs.len() >= max_pieces || k < p.first() {
            return None;
        }
        let win = p.stride.clamp(SECTOR, 8 << 10).min(p.len - k);
        // the picture just before the cut, and the data straight on after it
        let lo = k.saturating_sub(win).max(p.first());
        let base = if lo < k { score(&mut f, p, lo, k - lo)? } else { 20.0 };
        let straight = score(&mut f, p, k, win);
        // a row on, the data straight on reads as smoothly as the picture: a flaw in it, not a cut
        if straight.is_some_and(|t| !rough(t, base)) {
            from = k + SECTOR;
            continue;
        }
        let kept = frag::cut(&runs, k);
        // the last piece, already found, may start right here
        if let Some(s) = anchor.and_then(|(fo, d)| d.checked_sub(fo.checked_sub(k)?)) {
            let mine = kept.iter().any(|&(a, l)| s < a + l && a < s + SECTOR);
            if s % SECTOR == 0 && !mine && claims.covering(s).is_none() && free(s) {
                let mut hyp = kept.clone();
                hyp.push(frag::open_run(s, size, claims, max));
                f.set_runs(hyp.clone());
                let until = (k + TRIAL.max(2 * p.stride)).min(p.len);
                let fits =
                    score(&mut f, p, k, win).is_some_and(|j| !rough(j, base)) && !matches!(scan(&mut f, p, k, until), Scan::Fault(_));
                if fits {
                    runs = hyp;
                    from = k + SECTOR;
                    continue;
                }
            }
        }
        let origin = f.locate(k - 1)? + 1;
        let mut cands = Candidates::new(origin, size);
        let mut best: Option<(f32, u64)> = None;
        let (mut tries, mut limit) = (0u32, u32::MAX);
        while let Some(s) = cands.next(claims, &kept, free) {
            if f.disk_bytes(s, &mut sector) < sector.len() || !varied(&sector) {
                continue;
            }
            tries += 1;
            if tries >= limit || tries >= TRIES {
                break;
            }
            let mut hyp = kept.clone();
            hyp.push(frag::open_run(s, size, claims, max));
            f.set_runs(hyp);
            // the first sector alone first, then a row or so, then a good stretch further on
            let Some(g) = score(&mut f, p, k, SECTOR.min(win)) else { continue };
            if rough(g, base) {
                continue;
            }
            let Some(j) = score(&mut f, p, k, win) else { continue };
            if best.is_some_and(|(b, _)| j >= b) {
                continue;
            }
            let until = (k + TRIAL.max(2 * p.stride)).min(p.len);
            if matches!(scan(&mut f, p, k, until), Scan::Fault(_)) {
                continue;
            }
            best = Some((j, s));
            if j <= SMOOTH_K * base + SMOOTH_C && limit == u32::MAX {
                limit = tries + EXTRA;
            }
        }
        let (j, s) = best?;
        // the data straight on is not much worse: the picture itself changes there
        if straight.is_some_and(|t| j >= 0.5 * t) {
            f.set_runs(runs.clone());
            from = k + SECTOR;
            continue;
        }
        runs = kept;
        runs.push(frag::open_run(s, size, claims, max));
        f.set_runs(runs.clone());
        from = k + SECTOR;
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

    /// A 24-bit bitmap of soft shapes and a little grain, the way a photograph reads row to row;
    /// with `edge`, a hard line across it halfway down.
    fn picture(w: u32, h: u32, seed: u32, edge: bool) -> Vec<u8> {
        let stride = (w * 3).div_ceil(4) * 4;
        let mut out = Vec::new();
        out.extend(b"BM");
        out.extend((54 + stride * h).to_le_bytes());
        out.extend([0u8; 4]);
        out.extend(54u32.to_le_bytes());
        out.extend(40u32.to_le_bytes());
        out.extend(w.to_le_bytes());
        out.extend(h.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(24u16.to_le_bytes());
        out.extend([0u8; 24]);
        let grain = noise((stride * h) as usize, seed);
        let s = seed as f32;
        for y in 0..h {
            for x in 0..stride {
                let i = (y * stride + x) as usize;
                if x >= w * 3 {
                    out.push(0);
                    continue;
                }
                let (px, c) = ((x / 3) as f32, (x % 3) as f32);
                let mut v = 128.0
                    + 60.0 * ((px * 0.031 + s).sin() * (y as f32 * 0.023 + c).cos())
                    + 30.0 * ((px + y as f32) * 0.011 * (1.0 + c)).sin();
                if edge && y >= h / 2 {
                    v = 255.0 - v;
                }
                out.push((v + (grain[i] % 7) as f32 - 3.0).clamp(0.0, 255.0) as u8);
            }
        }
        out
    }

    fn pad(v: &mut Vec<u8>) {
        v.resize(v.len().next_multiple_of(512), 0);
    }

    #[test]
    fn whole_picture_reads_to_its_end() {
        let p = picture(300, 200, 1, false);
        let mut disk = p.clone();
        pad(&mut disk);
        disk.extend(noise(8192, 2));
        assert!(matches!(measure(&mut Mem(disk), 0), Check::Good(n) if n == p.len() as u64));
    }

    #[test]
    fn a_hard_line_across_the_picture_is_not_a_cut() {
        let p = picture(300, 200, 3, true);
        let mut disk = p.clone();
        pad(&mut disk);
        disk.extend(noise(8192, 4));
        let size = disk.len() as u64;
        let mut src = Mem(disk);
        match measure(&mut src, 0) {
            Check::Good(n) => assert_eq!(n, p.len() as u64),
            Check::Bad => {
                let runs = rebuild(&mut src, size, 0, &Claims::default(), &|_| true, 8).unwrap();
                assert_eq!(runs, vec![(0, p.len() as u64)]);
            }
            Check::Unsupported => panic!("unsupported"),
        }
    }

    #[test]
    fn picture_in_two_pieces_with_one_before_comes_back() {
        let p = picture(400, 300, 5, false);
        let cut = 300 * 512;
        // [noise][second piece][noise][first piece][noise]
        let mut disk = noise(8 * 512, 6);
        let second_at = disk.len() as u64;
        disk.extend(&p[cut..]);
        pad(&mut disk);
        disk.extend(noise(30 * 512, 7));
        let first_at = disk.len() as u64;
        disk.extend(&p[..cut]);
        disk.extend(noise(40 * 512, 8));
        let size = disk.len() as u64;
        let mut src = Mem(disk);
        assert!(matches!(measure(&mut src, first_at), Check::Bad));
        let runs = rebuild(&mut src, size, first_at, &Claims::default(), &|_| true, 8).unwrap();
        assert_eq!(runs, vec![(first_at, cut as u64), (second_at, (p.len() - cut) as u64)]);
    }

    #[test]
    fn another_picture_in_the_way_is_not_taken() {
        let p = picture(400, 300, 9, false);
        let q = picture(400, 300, 10, false);
        let cut = 211 * 512;
        let mut disk = p[..cut].to_vec();
        let q_at = disk.len() as u64;
        disk.extend(&q);
        pad(&mut disk);
        let second_at = disk.len() as u64;
        disk.extend(&p[cut..]);
        pad(&mut disk);
        disk.extend(noise(4096, 11));
        let size = disk.len() as u64;
        let mut src = Mem(disk);
        let mut claims = Claims::default();
        claims.add(q_at, q_at + 512);
        let runs = rebuild(&mut src, size, 0, &claims, &|_| true, 8).unwrap();
        assert_eq!(runs, vec![(0, cut as u64), (second_at, (p.len() - cut) as u64)]);
    }

    #[test]
    fn noise_with_a_header_is_not_a_picture() {
        let mut disk = picture(300, 200, 12, false)[..54].to_vec();
        disk.extend(noise(300 * 3 * 200 + 4096, 13));
        assert!(matches!(measure(&mut Mem(disk), 0), Check::Unsupported));
    }
}
