//! PCX files cut in pieces.
//!
//! A PCX holds its scan lines run-length encoded and nothing else to check them by. Decoded, the
//! rows of a photograph change little from one to the next, as in a bitmap; and the encoding has
//! rules of its own a stretch of some other file seldom keeps: no run is empty, and equal pixels
//! side by side are written as a run, not one by one. Where a piece ends, both break at once.
//! The next piece is the free sector that, decoded on from the state the decoder was in at the
//! cut, carries on the rows above as smoothly as the picture did, and keeps doing so for a while.

use std::collections::VecDeque;

use crate::carve::{Kind, Source};
use crate::frag::{self, Candidates, Claims, Feed, SECTOR};

const ROUGH_K: f32 = 3.0;
const ROUGH_C: f32 = 12.0;
const SMOOTH_K: f32 = 1.1;
const SMOOTH_C: f32 = 1.0;
const EXTRA: u32 = 1024;
const RECENT: usize = 16;
/// Encoded bytes a next piece must decode on as a picture before it is believed.
const TRIAL: u64 = 16 << 10;
const NOT_A_PHOTO: f32 = 40.0;
/// The same pixel written out this many times running, not as a run: no encoder does that.
const REPEATS: u8 = 4;
/// Searches for a next piece before a file is given up.
const SEARCHES: usize = 2;
// sectors tried at most for one join when none reads smooth
const TRIES: u32 = 20_000;

struct Pic {
    /// decoded bytes in the whole picture
    total: u64,
    /// decoded bytes in a scan line, all planes: a byte is compared with the one this far back
    row: usize,
    /// whether a 256-colour palette follows the pixels
    palette: bool,
}

fn pic(f: &mut Feed) -> Option<Pic> {
    let h = f.bytes(0, 128)?;
    let le16 = |i: usize| u16::from_le_bytes([h[i], h[i + 1]]) as u64;
    if h[0] != 0x0A || h[2] != 1 || h[3] != 8 || !matches!(h[65], 1 | 3 | 4) {
        return None;
    }
    let (x0, y0, x1, y1) = (le16(4), le16(6), le16(8), le16(10));
    if x1 < x0 || y1 < y0 {
        return None;
    }
    let (w, ht, bpl, planes) = (x1 - x0 + 1, y1 - y0 + 1, le16(66), h[65] as u64);
    if w < 16 || bpl < w || bpl > w + 64 {
        return None;
    }
    Some(Pic { total: planes * bpl * ht, row: (planes * bpl) as usize, palette: h[1] == 5 && planes == 1 })
}

/// Where the decoder stands: the next encoded byte, the pixels decoded so far, the last row of
/// them, and how the sectors just before read.
#[derive(Clone)]
struct Dec {
    pos: u64,
    done: u64,
    /// a run whose value is the next byte
    pend: u8,
    /// the last pixel written one by one, and how many times running
    last: u8,
    reps: u8,
    ring: Vec<u8>,
    recent: VecDeque<f32>,
}

impl Dec {
    fn new(p: &Pic) -> Self {
        Self { pos: 128, done: 0, pend: 0, last: 0, reps: 0, ring: vec![0; p.row], recent: VecDeque::new() }
    }

    fn put(&mut self, v: u8, n: u8, p: &Pic, acc: &mut Acc) {
        for _ in 0..n {
            if self.done >= p.total {
                return;
            }
            let i = (self.done % p.row as u64) as usize;
            if self.done >= p.row as u64 {
                let d = (v as i32 - self.ring[i] as i32).unsigned_abs() as u64;
                acc.sum += d;
                acc.n += 1;
                if acc.head_n < HEAD && self.done < acc.split {
                    acc.head += d;
                    acc.head_n += 1;
                }
            }
            self.ring[i] = v;
            self.done += 1;
        }
    }

    /// Decodes one byte; false when no encoder would have written it there.
    fn eat(&mut self, b: u8, p: &Pic, acc: &mut Acc) -> bool {
        self.pos += 1;
        if self.pend > 0 {
            let n = self.pend;
            self.pend = 0;
            self.reps = 0;
            self.put(b, n, p, acc);
        } else if b >= 0xC0 {
            self.pend = b & 0x3F;
            if self.pend == 0 {
                return false;
            }
        } else {
            if b == self.last && self.reps > 0 {
                self.reps = self.reps.saturating_add(1);
                if self.reps >= REPEATS {
                    return false;
                }
            } else {
                (self.last, self.reps) = (b, 1);
            }
            self.put(b, 1, p, acc);
        }
        true
    }

    fn base(&self) -> Option<f32> {
        let mut s: Vec<f32> = self.recent.iter().copied().collect();
        s.sort_by(f32::total_cmp);
        s.get(s.len() / 2).copied()
    }
}

/// Gaps over a sector: all of it, and its first pixels up to the next row.
#[derive(Default)]
struct Acc {
    sum: u64,
    n: u64,
    head: u64,
    head_n: u64,
    split: u64,
}

/// Pixels at the start of a sector that tell whether a rough stretch starts with it.
const HEAD: u64 = 32;

fn rough(g: f32, base: f32) -> bool {
    g > ROUGH_K * base + ROUGH_C
}

enum Scan {
    /// every pixel decoded: where the file ends, and the mean gap
    Done(u64, f32),
    /// decoded up to the end asked for
    Clean,
    /// the sector the decoder now stands at breaks off from the picture
    Fault,
    /// the pieces end before the sector the decoder now stands at does
    Short,
}

/// Decodes on from `st` up to encoded byte `until`, a sector at a time, each against the ones
/// before it, from encoded byte `judge` on. On a fault or a short read, `st` is left at the start
/// of the sector at fault.
fn scan(f: &mut Feed, p: &Pic, st: &mut Dec, until: u64, judge: u64) -> Scan {
    let (mut sum, mut n) = (0f64, 0u64);
    let mean = |sum: f64, n: u64| if n == 0 { 0.0 } else { (sum / n as f64) as f32 };
    let mut buf = Vec::new();
    loop {
        if st.done >= p.total {
            let mut end = st.pos;
            if p.palette && f.bytes(end, 1).as_deref() == Some(&[0x0C]) && f.bytes(end + 768, 1).is_some() {
                end += 769;
            }
            return Scan::Done(end, mean(sum, n));
        }
        if st.pos >= until {
            return Scan::Clean;
        }
        let mut end = (st.pos + (64 << 10)).min(until);
        if end < until {
            end -= end % SECTOR;
        }
        buf.resize((end - st.pos) as usize, 0);
        let got = f.read_some(st.pos, &mut buf);
        let mut i = 0;
        while i < buf.len() && st.done < p.total {
            let sec = (SECTOR - st.pos % SECTOR) as usize;
            let take = sec.min(buf.len() - i);
            let save = st.clone();
            let have = take.min(got.saturating_sub(i));
            let mut acc = Acc { split: save.done.next_multiple_of(p.row as u64).max(save.done + 1), ..Default::default() };
            let mut valid = true;
            for &b in &buf[i..i + have] {
                // a byte no encoder writes is let through where nothing is judged
                valid &= st.eat(b, p, &mut acc);
                if st.done >= p.total {
                    break;
                }
            }
            let used = (st.pos - save.pos) as usize;
            if used < take && st.done < p.total {
                *st = save;
                return Scan::Short;
            }
            let judged = save.pos >= judge;
            if !valid && judged {
                *st = save;
                return Scan::Fault;
            }
            if acc.n > 0 {
                let g = acc.sum as f32 / acc.n as f32;
                if let Some(base) = st.base().filter(|_| judged && st.done < p.total)
                    && rough(g, base)
                    && !(starts_smooth(&acc, base) && edge(f, p, st, base))
                {
                    *st = save;
                    return Scan::Fault;
                }
                st.recent.push_back(g);
                if st.recent.len() > RECENT {
                    st.recent.pop_front();
                }
                sum += acc.sum as f64;
                n += acc.n;
            }
            i += used;
        }
    }
}

/// Whether a rough stretch just decoded is only a hard line across the picture: a row further
/// on, the rows read smoothly again against each other.
fn edge(f: &mut Feed, p: &Pic, st: &Dec, base: f32) -> bool {
    let mut d = st.clone();
    let from = d.done + p.row as u64;
    let want = (p.row as u64 / 2).max(256);
    if from + want > p.total {
        return false;
    }
    let (mut sum, mut n) = (0u64, 0u64);
    let mut buf = [0u8; 1024];
    while d.done < from + want {
        let got = f.read_some(d.pos, &mut buf);
        if got == 0 {
            return false;
        }
        for &b in &buf[..got] {
            let mut a = Acc::default();
            if !d.eat(b, p, &mut a) {
                return false;
            }
            if d.done > from {
                sum += a.sum;
                n += a.n;
            }
            if d.done >= from + want {
                break;
            }
        }
    }
    n > 0 && !rough(sum as f32 / n as f32, base)
}

/// Whether the sector starts smoothly, a rough stretch in it starting only with a row: a hard
/// line across the picture does that, a piece of some other file put after the cut does not.
fn starts_smooth(acc: &Acc, base: f32) -> bool {
    acc.head_n < 8 || !rough(acc.head as f32 / acc.head_n as f32, base)
}

/// The gap over the row of pixels decoded on from `d`: each compared with the one above it,
/// from before the cut. `None` when the pieces end first or, if `strict`, a byte no encoder
/// writes turns up.
fn cross(f: &mut Feed, p: &Pic, d: &mut Dec, strict: bool) -> Option<f32> {
    let to = (d.done + p.row as u64).min(p.total);
    let mut a = Acc::default();
    let mut buf = [0u8; 1024];
    while d.done < to {
        let got = f.read_some(d.pos, &mut buf);
        if got == 0 {
            return None;
        }
        for &b in &buf[..got] {
            if !d.eat(b, p, &mut a) && strict {
                return None;
            }
            if d.done >= to {
                break;
            }
        }
    }
    (a.n > 0).then(|| a.sum as f32 / a.n as f32)
}

pub enum Check {
    Good(u64),
    Bad,
    Unsupported,
}

pub fn measure(src: &mut dyn Source, at: u64) -> Check {
    let max = Kind::Pcx.max_size();
    let mut f = Feed::new(src, vec![(at, max)]);
    let Some(p) = pic(&mut f) else { return Check::Unsupported };
    let mut st = Dec::new(&p);
    match scan(&mut f, &p, &mut st, max, 0) {
        Scan::Done(len, g) if g <= NOT_A_PHOTO => Check::Good(len),
        Scan::Done(..) | Scan::Clean => Check::Unsupported,
        Scan::Fault | Scan::Short => Check::Bad,
    }
}

pub fn rebuild(
    src: &mut dyn Source,
    size: u64,
    at: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
    max_pieces: usize,
) -> Option<Vec<(u64, u64)>> {
    let max = Kind::Pcx.max_size();
    let first = frag::open_run(at, size, claims, max);
    let mut f = Feed::new(src, vec![first]);
    let p = pic(&mut f)?;
    let mut runs = vec![first];
    let mut st = Dec::new(&p);
    let mut judge = 0;
    // searches through the free sectors, each a few seconds on a big disk: past these, the
    // file is given up
    let mut searches = 0;
    loop {
        match scan(&mut f, &p, &mut st, max, judge) {
            Scan::Done(len, g) if g <= NOT_A_PHOTO => return Some(frag::cut(&runs, len)),
            Scan::Done(..) | Scan::Clean => return None,
            Scan::Fault | Scan::Short => {}
        }
        let k = st.pos;
        searches += 1;
        if searches > SEARCHES {
            return None;
        }
        if runs.len() >= max_pieces || k < SECTOR || !k.is_multiple_of(SECTOR) {
            return None;
        }
        let base = st.base().unwrap_or(20.0);
        // the data straight on, judged on nothing
        let straight = cross(&mut f, &p, &mut st.clone(), false);
        let kept = frag::cut(&runs, k);
        let origin = f.locate(k - 1)? + 1;
        let mut cands = Candidates::new(origin, size);
        let mut best: Option<(f32, u64)> = None;
        let (mut tries, mut limit) = (0u32, u32::MAX);
        while let Some(s) = cands.next(claims, &kept, free) {
            tries += 1;
            if tries >= limit || (limit == u32::MAX && tries >= TRIES) {
                break;
            }
            let mut hyp = kept.clone();
            hyp.push(frag::open_run(s, size, claims, max));
            f.set_runs(hyp);
            // a sector first, then the row across the join, then a good stretch on
            if matches!(scan(&mut f, &p, &mut st.clone(), k + SECTOR, 0), Scan::Fault | Scan::Short) {
                continue;
            }
            let mut d = st.clone();
            let Some(j) = cross(&mut f, &p, &mut d, true) else { continue };
            if best.is_some_and(|(b, _)| j >= b) {
                continue;
            }
            if d.done < p.total && matches!(scan(&mut f, &p, &mut d, k + TRIAL, 0), Scan::Fault | Scan::Short) {
                continue;
            }
            best = Some((j, s));
            if j <= SMOOTH_K * base + SMOOTH_C && limit == u32::MAX {
                limit = tries + EXTRA;
            }
        }
        let (j, s) = best?;
        if straight.is_some_and(|t| j >= 0.5 * t) {
            f.set_runs(runs.clone());
            judge = k + SECTOR;
            continue;
        }
        runs = kept;
        runs.push(frag::open_run(s, size, claims, max));
        f.set_runs(runs.clone());
        judge = 0;
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

    /// A 3-plane, 8-bit PCX of soft shapes with a little grain; with `edge`, a hard line across
    /// it halfway down.
    fn picture(w: u16, h: u16, seed: u32, edge: bool) -> Vec<u8> {
        let mut out = vec![0u8; 128];
        out[0] = 0x0A;
        out[1] = 5;
        out[2] = 1;
        out[3] = 8;
        out[8..10].copy_from_slice(&(w - 1).to_le_bytes());
        out[10..12].copy_from_slice(&(h - 1).to_le_bytes());
        out[65] = 3;
        out[66..68].copy_from_slice(&w.to_le_bytes());
        let grain = noise(w as usize * h as usize * 3, seed);
        let s = seed as f32;
        for y in 0..h as usize {
            let mut line = Vec::new();
            for c in 0..3usize {
                for x in 0..w as usize {
                    let (px, cf) = (x as f32, c as f32);
                    let mut v = 128.0
                        + 60.0 * ((px * 0.031 + s).sin() * (y as f32 * 0.023 + cf).cos())
                        + 30.0 * ((px + y as f32) * 0.011 * (1.0 + cf)).sin();
                    if edge && y >= h as usize / 2 {
                        v = 255.0 - v;
                    }
                    let g = grain[(y * 3 + c) * w as usize + x];
                    line.push((v + (g % 7) as f32 - 3.0).clamp(0.0, 255.0) as u8);
                }
            }
            let mut i = 0;
            while i < line.len() {
                let mut n = 1;
                while i + n < line.len() && line[i + n] == line[i] && n < 63 {
                    n += 1;
                }
                if n > 1 || line[i] >= 0xC0 {
                    out.push(0xC0 | n as u8);
                }
                out.push(line[i]);
                i += n;
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
        let mut disk = noise(8 * 512, 6);
        let second_at = disk.len() as u64;
        disk.extend(&p[cut..]);
        pad(&mut disk);
        disk.extend(vec![0u8; 30 * 512]);
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
        let mut disk = picture(300, 200, 12, false)[..128].to_vec();
        disk.extend(noise(300 * 3 * 200 * 2, 13));
        assert!(!matches!(measure(&mut Mem(disk), 0), Check::Good(_)));
    }
}
