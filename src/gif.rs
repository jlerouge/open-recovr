//! GIF files cut in pieces.
//!
//! A GIF holds its picture as one LZW stream, chopped into sub-blocks of up to 255 bytes, with
//! nothing to check it by. Decoded, the rows of a photograph change little from one to the next;
//! and the stream has rules a stretch of some other file seldom keeps for long: every code names
//! an entry the dictionary already holds, no colour lies past the palette, and the stream does not
//! end before the last pixel. Where a piece ends, the codes that follow, read on with the
//! dictionary built so far, still give strings of the picture's own pixels, but in the wrong
//! places: the rows stop carrying on from the ones above. The next piece is the free sector that,
//! decoded on from the state the decoder was in at the cut, carries the rows on as smoothly as
//! the picture did, and keeps doing so for a while.
//!
//! An interlaced picture is decoded in its own order, each row against the one decoded before
//! it: rows a few lines apart in the picture, still close.

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
/// Searches for a next piece before a file is given up.
const SEARCHES: usize = 2;
/// Sectors tried at most for one join when none reads smooth.
const TRIES: u32 = 40_000;
/// Pixels at the start of a sector that tell whether a rough stretch starts with it.
const HEAD: u64 = 32;
const MAX_CODES: usize = 4096;

struct Pic {
    /// pixels in the picture
    total: u64,
    /// pixels in a row: a pixel is compared with the one this far back
    row: usize,
    /// where the LZW stream starts: its first sub-block length
    data: u64,
    /// the LZW minimum code size
    min: u32,
    /// colours in the palette
    colours: usize,
    /// a brightness for each colour
    lut: [u8; 256],
    /// rows, in decoding order, that start a pass of an interlaced picture: nothing above them
    passes: [u64; 4],
}

fn pic(f: &mut Feed) -> Option<Pic> {
    let h = f.bytes(0, 13)?;
    if !(h.starts_with(b"GIF87a") || h.starts_with(b"GIF89a")) {
        return None;
    }
    let mut pos = 13u64;
    let mut palette = None;
    if h[10] & 0x80 != 0 {
        let n = 3u64 << ((h[10] & 7) + 1);
        palette = Some(f.bytes(pos, n as usize)?);
        pos += n;
    }
    let (w, ht, interlaced) = loop {
        match f.byte(pos)? {
            0x21 => {
                pos += 2;
                loop {
                    let n = f.byte(pos)? as u64;
                    pos += 1 + n;
                    if n == 0 {
                        break;
                    }
                    if pos > 1 << 20 {
                        return None;
                    }
                }
            }
            0x2C => {
                let d = f.bytes(pos, 10)?;
                let le16 = |i: usize| u16::from_le_bytes([d[i], d[i + 1]]) as u64;
                pos += 10;
                if d[9] & 0x80 != 0 {
                    let n = 3u64 << ((d[9] & 7) + 1);
                    palette = Some(f.bytes(pos, n as usize)?);
                    pos += n;
                }
                break (le16(5), le16(7), d[9] & 0x40 != 0);
            }
            _ => return None,
        }
    };
    let palette = palette?;
    let min = f.byte(pos)? as u32;
    if !(2..=8).contains(&min) || w < 16 || ht < 2 {
        return None;
    }
    let mut lut = [0u8; 256];
    for (i, c) in palette.chunks(3).enumerate() {
        lut[i] = ((299 * c[0] as u32 + 587 * c[1] as u32 + 114 * c[2] as u32) / 1000) as u8;
    }
    let passes = if interlaced {
        let (a, b, c) = (ht.div_ceil(8), (ht + 3) / 8, (ht + 1) / 4);
        [0, a, a + b, a + b + c]
    } else {
        [0; 4]
    };
    Some(Pic { total: w * ht, row: w as usize, data: pos + 1, min, colours: palette.len() / 3, lut, passes })
}

/// A dictionary entry: the code it extends, its last pixel, its first, and its length.
#[derive(Clone, Copy)]
struct Entry {
    prefix: u16,
    last: u8,
    first: u8,
    len: u16,
}

/// Where the decoder stands: the next encoded byte, the bits and dictionary so far, the pixels
/// decoded so far, the last row of them, and how the sectors just before read.
#[derive(Clone)]
struct Dec {
    pos: u64,
    /// bytes left in the sub-block: at 0, the next byte is a length
    left: u8,
    bits: u32,
    nbits: u32,
    size: u32,
    prev: Option<u16>,
    table: Vec<Entry>,
    done: u64,
    ring: Vec<u8>,
    recent: VecDeque<f32>,
    /// a string being written out, kept empty in between so that copies stay cheap
    out: Vec<u8>,
}

impl Dec {
    fn new(p: &Pic) -> Self {
        let mut d = Self {
            pos: p.data,
            left: 0,
            bits: 0,
            nbits: 0,
            size: 0,
            prev: None,
            table: Vec::with_capacity(MAX_CODES),
            done: 0,
            ring: vec![0; p.row],
            recent: VecDeque::new(),
            out: Vec::new(),
        };
        d.reset(p);
        d
    }

    fn reset(&mut self, p: &Pic) {
        let clear = 1usize << p.min;
        self.table.clear();
        self.table.extend((0..clear + 2).map(|i| Entry { prefix: 0, last: i as u8, first: i as u8, len: 1 }));
        self.size = p.min + 1;
        self.prev = None;
    }

    fn put(&mut self, v: u8, p: &Pic, acc: &mut Acc) {
        if self.done >= p.total {
            return;
        }
        let i = (self.done % p.row as u64) as usize;
        let r = self.done / p.row as u64;
        let v = p.lut[v as usize];
        if r > 0 && !p.passes.contains(&r) {
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

    /// Writes out the string of `c`, then `tail` if given; its first pixel.
    fn string(&mut self, c: u16, tail: Option<u8>, p: &Pic, acc: &mut Acc) -> u8 {
        let mut out = std::mem::take(&mut self.out);
        let mut k = c as usize;
        loop {
            let e = self.table[k];
            out.push(e.last);
            if e.len == 1 {
                break;
            }
            k = e.prefix as usize;
        }
        let first = *out.last().unwrap_or(&0);
        for &v in out.iter().rev() {
            self.put(v, p, acc);
        }
        if let Some(t) = tail {
            self.put(t, p, acc);
        }
        out.clear();
        self.out = out;
        first
    }

    /// Decodes one code; false when no encoder would have written it there.
    fn code(&mut self, c: u16, p: &Pic, acc: &mut Acc) -> bool {
        let clear = 1u16 << p.min;
        if c == clear {
            self.reset(p);
            return true;
        }
        if c == clear + 1 {
            // the end of the stream, before the last pixel
            return false;
        }
        let n = self.table.len();
        let first = if (c as usize) < n {
            if c < clear && c as usize >= p.colours {
                return false;
            }
            self.string(c, None, p, acc)
        } else if c as usize == n
            && let Some(q) = self.prev
        {
            let f = self.table[q as usize].first;
            self.string(q, Some(f), p, acc);
            f
        } else {
            return false;
        };
        if let Some(q) = self.prev
            && n < MAX_CODES
        {
            let e = self.table[q as usize];
            self.table.push(Entry { prefix: q, last: first, first: e.first, len: e.len.saturating_add(1) });
            if self.table.len() >= 1 << self.size && self.size < 12 {
                self.size += 1;
            }
        }
        self.prev = Some(c);
        true
    }

    /// Decodes one byte of the stream; false when no encoder would have written it there.
    fn eat(&mut self, b: u8, p: &Pic, acc: &mut Acc) -> bool {
        self.pos += 1;
        if self.left == 0 {
            // a sub-block length; the stream's end before the last pixel is none
            self.left = b;
            return b != 0;
        }
        self.left -= 1;
        self.bits |= (b as u32) << self.nbits;
        self.nbits += 8;
        while self.nbits >= self.size && self.done < p.total {
            let c = (self.bits & ((1 << self.size) - 1)) as u16;
            self.bits >>= self.size;
            self.nbits -= self.size;
            if !self.code(c, p, acc) {
                return false;
            }
        }
        true
    }

    fn base(&self) -> Option<f32> {
        let mut s: Vec<f32> = self.recent.iter().copied().collect();
        s.sort_by(f32::total_cmp);
        s.get(s.len() / 2).copied()
    }
}

/// Where the file ends once every pixel is decoded: the rest of the stream, then extensions up to
/// the trailer. `None` when that does not read as a GIF's end, or another frame follows.
fn tail(f: &mut Feed, st: &Dec) -> Option<u64> {
    let mut pos = st.pos + st.left as u64;
    let walk = |f: &mut Feed, mut pos: u64| -> Option<u64> {
        for _ in 0..4096 {
            let n = f.byte(pos)? as u64;
            pos += 1 + n;
            if n == 0 {
                return Some(pos);
            }
        }
        None
    };
    pos = walk(f, pos)?;
    for _ in 0..64 {
        match f.byte(pos)? {
            0x3B => return Some(pos + 1),
            0x21 => pos = walk(f, pos + 2)?,
            _ => return None,
        }
    }
    None
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

fn rough(g: f32, base: f32) -> bool {
    g > ROUGH_K * base + ROUGH_C
}

enum Scan {
    /// every pixel decoded: where the file ends, if it reads as a single picture's end, and the
    /// mean gap
    Done(Option<u64>, f32),
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
            return Scan::Done(tail(f, st), mean(sum, n));
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
            let before = d.done;
            if !d.eat(b, p, &mut a) {
                return false;
            }
            if before >= from {
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
    let max = Kind::Gif.max_size();
    let mut f = Feed::new(src, vec![(at, max)]);
    let Some(p) = pic(&mut f) else { return Check::Unsupported };
    let mut st = Dec::new(&p);
    match scan(&mut f, &p, &mut st, max, 0) {
        Scan::Done(Some(len), g) if g <= NOT_A_PHOTO => Check::Good(len),
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
    let max = Kind::Gif.max_size();
    let first = frag::open_run(at, size, claims, max);
    let mut f = Feed::new(src, vec![first]);
    let p = pic(&mut f)?;
    let mut runs = vec![first];
    let mut st = Dec::new(&p);
    let mut judge = 0;
    let mut searches = 0;
    loop {
        match scan(&mut f, &p, &mut st, max, judge) {
            Scan::Done(Some(len), g) if g <= NOT_A_PHOTO => return Some(frag::cut(&runs, len)),
            Scan::Done(..) | Scan::Clean => return None,
            Scan::Fault | Scan::Short => {}
        }
        let k = st.pos;
        searches += 1;
        if searches > SEARCHES || runs.len() >= max_pieces || k < SECTOR || !k.is_multiple_of(SECTOR) {
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
    use std::collections::HashMap;

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

    /// LZW codes as a GIF encoder writes them, packed and cut into sub-blocks.
    fn lzw(px: &[u8]) -> Vec<u8> {
        let (clear, eoi) = (256u32, 257u32);
        let mut bytes = Vec::new();
        let (mut acc, mut nacc) = (0u64, 0u32);
        let mut emit = |c: u32, size: u32, bytes: &mut Vec<u8>| {
            acc |= (c as u64) << nacc;
            nacc += size;
            while nacc >= 8 {
                bytes.push(acc as u8);
                acc >>= 8;
                nacc -= 8;
            }
        };
        let mut dict: HashMap<(u32, u8), u32> = HashMap::new();
        let (mut next, mut size) = (258u32, 9u32);
        emit(clear, size, &mut bytes);
        let mut cur = px[0] as u32;
        for &b in &px[1..] {
            if let Some(&c) = dict.get(&(cur, b)) {
                cur = c;
                continue;
            }
            emit(cur, size, &mut bytes);
            if next >= 1 << size && size < 12 {
                size += 1;
            }
            if next < 4096 {
                dict.insert((cur, b), next);
                next += 1;
            } else {
                emit(clear, size, &mut bytes);
                dict.clear();
                (next, size) = (258, 9);
            }
            cur = b as u32;
        }
        emit(cur, size, &mut bytes);
        emit(eoi, size, &mut bytes);
        emit(0, 7, &mut bytes);
        let mut out = vec![8u8];
        for chunk in bytes.chunks(255) {
            out.push(chunk.len() as u8);
            out.extend(chunk);
        }
        out.push(0);
        out
    }

    /// A grey GIF of soft shapes with a little grain; with `edge`, a hard line across it halfway
    /// down.
    fn picture(w: u16, h: u16, seed: u32, edge: bool, interlaced: bool) -> Vec<u8> {
        let mut out = b"GIF89a".to_vec();
        out.extend(w.to_le_bytes());
        out.extend(h.to_le_bytes());
        out.extend([0xF7, 0, 0]);
        for i in 0..=255u8 {
            out.extend([i, i, i]);
        }
        out.push(0x2C);
        out.extend([0, 0, 0, 0]);
        out.extend(w.to_le_bytes());
        out.extend(h.to_le_bytes());
        out.push(if interlaced { 0x40 } else { 0 });
        let grain = noise(w as usize * h as usize, seed);
        let s = seed as f32;
        let row = |y: usize| -> Vec<u8> {
            (0..w as usize)
                .map(|x| {
                    let px = x as f32;
                    let mut v = 128.0 + 60.0 * ((px * 0.031 + s).sin() * (y as f32 * 0.023).cos()) + 30.0 * ((px + y as f32) * 0.011).sin();
                    if edge && y >= h as usize / 2 {
                        v = 255.0 - v;
                    }
                    (v + (grain[y * w as usize + x] % 5) as f32 - 2.0).clamp(0.0, 255.0) as u8
                })
                .collect()
        };
        let order: Vec<usize> = if interlaced {
            let h = h as usize;
            (0..h).step_by(8).chain((4..h).step_by(8)).chain((2..h).step_by(4)).chain((1..h).step_by(2)).collect()
        } else {
            (0..h as usize).collect()
        };
        let px: Vec<u8> = order.into_iter().flat_map(row).collect();
        out.extend(lzw(&px));
        out.push(0x3B);
        out
    }

    fn pad(v: &mut Vec<u8>) {
        v.resize(v.len().next_multiple_of(512), 0);
    }

    #[test]
    fn whole_picture_reads_to_its_end() {
        for interlaced in [false, true] {
            let p = picture(300, 200, 1, false, interlaced);
            let mut disk = p.clone();
            pad(&mut disk);
            disk.extend(noise(8192, 2));
            assert!(matches!(measure(&mut Mem(disk), 0), Check::Good(n) if n == p.len() as u64));
        }
    }

    #[test]
    fn a_hard_line_across_the_picture_is_not_a_cut() {
        let p = picture(300, 200, 3, true, false);
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
        for interlaced in [false, true] {
            let p = picture(400, 300, 5, false, interlaced);
            let cut = 100 * 512;
            assert!(cut < p.len());
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
    }

    #[test]
    fn another_picture_in_the_way_is_not_taken() {
        let p = picture(400, 300, 9, false, false);
        let q = picture(400, 300, 10, false, false);
        let cut = 71 * 512;
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
        let p = picture(300, 200, 12, false, false);
        let mut disk = p[..13 + 768 + 11].to_vec();
        disk.extend(noise(300 * 200, 13));
        assert!(!matches!(measure(&mut Mem(disk), 0), Check::Good(_)));
    }
}
