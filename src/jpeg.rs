//! Baseline JPEG, decoded only as far as needed to tell where its coded data stops making sense.
//!
//! No picture is made (no inverse DCT): each Huffman code, coefficient position, coefficient
//! range and restart marker is checked, which random bytes, text or the coded data of another
//! picture fail within a few hundred bytes. Where it fails, the file was most likely cut by the
//! file system; the decoder state kept every sector or so lets it start again from just before
//! the cut on each sector that might hold the rest, and the DC values of the luma blocks tell a
//! piece of this picture from a piece of another one coded the same way: the row of blocks after
//! the join must look like the row above it.
//!
//! Written from ITU-T T.81 (the JPEG standard), sections F.1.2 and F.2.2.

use crate::carve::Source;
use crate::frag::{self, Candidates, Claims, Feed, SECTOR};

/// Bytes a piece must decode past the join before it is believed.
const TRIAL: u64 = 8 << 10;
/// Pieces tried at each cut before settling for the best one seen.
const BUDGET: u32 = 6_000;
/// Cuts tried at each fault, from the sector bound just above it down.
const CUTS: usize = 4;
/// A join that jumps less than this, against the rows before it, is taken unless a better one
/// turns up within `EXTRA` more tries.
const SMOOTH: f64 = 1.0;
const EXTRA: u32 = 256;
/// A join that jumps more than this, against the rows before it, is never taken.
const ROUGH: f64 = 4.0;
/// Bytes decoded, at one fault, to see whether joins that are not the smoothest end the file.
const WHOLE: u64 = 2 << 20;

#[derive(Clone)]
struct Huff {
    /// first 8 bits of a code -> (length << 8) | value, 0 when the code is longer
    look: [u16; 256],
    maxcode: [i32; 17],
    valptr: [i32; 17],
    mincode: [i32; 17],
    vals: [u8; 256],
}

impl Huff {
    fn new(counts: &[u8], vals: &[u8]) -> Option<Huff> {
        let mut h = Huff { look: [0; 256], maxcode: [-1; 17], valptr: [0; 17], mincode: [0; 17], vals: [0; 256] };
        h.vals[..vals.len()].copy_from_slice(vals);
        let (mut code, mut k) = (0i32, 0usize);
        for l in 1..=16 {
            let n = counts[l - 1] as usize;
            h.valptr[l] = k as i32;
            h.mincode[l] = code;
            for _ in 0..n {
                if l <= 8 {
                    let shift = 8 - l;
                    for f in 0..1usize << shift {
                        h.look[((code as usize) << shift) | f] = ((l as u16) << 8) | vals[k] as u16;
                    }
                }
                code += 1;
                k += 1;
            }
            if n > 0 {
                h.maxcode[l] = code - 1;
            }
            if code > 1 << l {
                return None;
            }
            code <<= 1;
        }
        Some(h)
    }
}

#[derive(Clone)]
struct Tables {
    dc: [Option<Box<Huff>>; 4],
    ac: [Option<Box<Huff>>; 4],
    q: [[u16; 64]; 4],
}

impl Default for Tables {
    fn default() -> Self {
        Tables { dc: Default::default(), ac: Default::default(), q: [[1; 64]; 4] }
    }
}

struct Comp {
    id: u8,
    h: usize,
    v: usize,
    tq: usize,
}

#[derive(Default)]
struct Frame {
    w: u64,
    h: u64,
    comps: Vec<Comp>,
    hmax: usize,
    vmax: usize,
    /// largest dequantized DC and AC values for the sample precision
    dc_max: i32,
    ac_max: i32,
}

#[derive(Clone, Copy, Default)]
struct ScanComp {
    comp: usize,
    dc: usize,
    ac: usize,
    /// blocks per MCU, across and down
    bh: usize,
    bv: usize,
}

#[derive(Clone, Copy, Default)]
struct Scan {
    comps: [ScanComp; 4],
    ns: usize,
    mcus_x: u64,
    mcus: u64,
}

/// The bit reader: `n` bits in `acc`, the last `fake` of them zeros made up past a marker.
#[derive(Clone, Copy, Default)]
struct Bits {
    pos: u64,
    acc: u64,
    n: u32,
    fake: u32,
    /// a marker (or the end of the data) was met at `pos`; `mark` is its second byte, 0 at the end
    stop: bool,
    mark: u8,
}

/// Everything the decoder needs to start again from the start of an MCU.
#[derive(Clone, Copy, Default)]
struct Point {
    bits: Bits,
    preds: [i32; 4],
    mcu: u64,
    rst: u8,
    left: u32,
    tables: usize,
    scan: Scan,
    /// scans finished before this one
    scans: u32,
}

pub enum Outcome {
    /// the file ends at this offset
    Done(u64),
    /// the data stops making sense at about this offset
    Broken(u64),
    /// not a baseline JPEG: progressive, arithmetic coded, lossless...
    Unsupported,
}

// short-lived, one at a time: not worth a box
#[allow(clippy::large_enum_variant)]
enum Next {
    Scan(Point),
    End(Outcome),
}

/// How a try from a point went.
enum Run {
    Out(Outcome),
    /// decoded cleanly up to the limit
    Far,
    /// met the end marker before the last block: the data read is the end of some picture
    Short(u64),
}

pub struct Jpeg {
    frame: Frame,
    versions: Vec<Tables>,
    restart: u32,
    max: u64,
    points: Vec<Point>,
    /// the pixels along the top and the bottom of every luma block of the scan, per MCU
    grid: Vec<[f32; 16]>,
    /// the join being tried: cut offset, MCU where the new piece starts, and the gaps in the
    /// values across it and in the rows before it
    join: Option<Join>,
}

#[derive(Clone, Copy, Default)]
struct Join {
    cut: u64,
    mcu: Option<u64>,
    across: (f64, u32),
    before: (f64, u32),
}

impl Bits {
    #[inline]
    fn fill(&mut self, f: &mut Feed) {
        while self.n <= 56 {
            if self.stop {
                self.acc <<= 8;
                self.n += 8;
                self.fake += 8;
                continue;
            }
            let Some(b) = f.byte(self.pos) else {
                self.stop = true;
                self.mark = 0;
                continue;
            };
            if b == 0xFF {
                match f.byte(self.pos + 1) {
                    Some(0x00) => self.pos += 2,
                    // fill bytes before a marker
                    Some(0xFF) => {
                        self.pos += 1;
                        continue;
                    }
                    Some(m) => {
                        self.stop = true;
                        self.mark = m;
                        continue;
                    }
                    None => {
                        self.stop = true;
                        self.mark = 0;
                        continue;
                    }
                }
            } else {
                self.pos += 1;
            }
            self.acc = (self.acc << 8) | b as u64;
            self.n += 8;
        }
    }

    #[inline]
    fn real(&self) -> u32 {
        self.n - self.fake
    }

    #[inline]
    fn peek16(&mut self, f: &mut Feed) -> u32 {
        if self.n < 16 {
            self.fill(f);
        }
        ((self.acc >> (self.n - 16)) & 0xFFFF) as u32
    }

    /// Uses up `k` bits; None when they run past a marker.
    #[inline]
    fn take(&mut self, f: &mut Feed, k: u32) -> Option<u32> {
        if k == 0 {
            return Some(0);
        }
        if self.n < k {
            self.fill(f);
        }
        if k > self.real() {
            return None;
        }
        self.n -= k;
        Some(((self.acc >> self.n) & ((1u64 << k) - 1)) as u32)
    }

    #[inline]
    fn huff(&mut self, f: &mut Feed, h: &Huff) -> Option<u8> {
        let p = self.peek16(f);
        let e = h.look[(p >> 8) as usize];
        if e != 0 {
            self.take(f, (e >> 8) as u32)?;
            return Some(e as u8);
        }
        for l in 9..=16 {
            let code = (p >> (16 - l)) as i32;
            if code <= h.maxcode[l] {
                self.take(f, l as u32)?;
                return Some(h.vals[(h.valptr[l] + code - h.mincode[l]) as usize]);
            }
        }
        None
    }

    /// Where the decoding stands, to the byte: the bits read ahead are given back.
    fn at(&self) -> u64 {
        self.pos.saturating_sub((self.real() / 8) as u64)
    }
}

#[inline]
fn extend(v: u32, s: u32) -> i32 {
    if s == 0 {
        0
    } else if v < 1 << (s - 1) {
        v as i32 - (1 << s) + 1
    } else {
        v as i32
    }
}

fn be16(b: &[u8]) -> u64 {
    u16::from_be_bytes([b[0], b[1]]) as u64
}

impl Jpeg {
    /// Decodes the file the feed shows from its start.
    pub fn start(f: &mut Feed, max: u64) -> (Option<Jpeg>, Outcome) {
        let mut j = Jpeg {
            frame: Frame::default(),
            versions: vec![Tables::default()],
            restart: 0,
            max,
            points: Vec::new(),
            grid: Vec::new(),
            join: None,
        };
        match j.markers(f, 2, 0) {
            Next::End(o) => (None, o),
            Next::Scan(p) => {
                let o = j.go(f, p, None);
                match o {
                    Run::Out(Outcome::Unsupported) => (None, Outcome::Unsupported),
                    Run::Out(o) => (Some(j), o),
                    Run::Short(e) => (Some(j), Outcome::Broken(e)),
                    Run::Far => unreachable!(),
                }
            }
        }
    }

    /// Reads the marker segments from `pos` up to the next scan or the end of the file.
    fn markers(&mut self, f: &mut Feed, mut pos: u64, scans: u32) -> Next {
        let broken = |p: u64| Next::End(Outcome::Broken(p));
        let mut dirty = false;
        loop {
            if pos > self.max {
                return broken(pos);
            }
            let (Some(b0), Some(m)) = (f.byte(pos), f.byte(pos + 1)) else { return broken(pos) };
            if b0 != 0xFF {
                return broken(pos);
            }
            match m {
                0xFF => {
                    pos += 1;
                    continue;
                }
                0xD8 if pos == 0 => {
                    pos += 2;
                    continue;
                }
                0xD0..=0xD7 | 0x01 => {
                    pos += 2;
                    continue;
                }
                0xD9 => return if scans > 0 { Next::End(Outcome::Done(pos + 2)) } else { broken(pos) },
                0xD8 | 0x00 => return broken(pos),
                _ => {}
            }
            let Some(l) = f.bytes(pos + 2, 2) else { return broken(pos) };
            let len = be16(&l);
            if len < 2 {
                return broken(pos);
            }
            let body_at = pos + 4;
            let body_len = len as usize - 2;
            let needs_body = matches!(m, 0xC0..=0xCF | 0xDA | 0xDB | 0xDD);
            let body = if needs_body {
                match f.bytes(body_at, body_len) {
                    Some(b) => b,
                    None => return broken(pos),
                }
            } else {
                Vec::new()
            };
            match m {
                0xC0 | 0xC1 => {
                    if body.len() < 6 || !self.frame.comps.is_empty() {
                        return broken(pos);
                    }
                    let p = body[0];
                    let (h, w, nf) = (be16(&body[1..]), be16(&body[3..]), body[5] as usize);
                    if p != 8 && p != 12 {
                        return Next::End(Outcome::Unsupported);
                    }
                    if h == 0 || w == 0 || nf == 0 || nf > 4 || body.len() < 6 + 3 * nf {
                        return if h == 0 { Next::End(Outcome::Unsupported) } else { broken(pos) };
                    }
                    let mut comps = Vec::new();
                    for c in body[6..6 + 3 * nf].chunks(3) {
                        let (hs, vs, tq) = ((c[1] >> 4) as usize, (c[1] & 15) as usize, c[2] as usize);
                        if !(1..=4).contains(&hs) || !(1..=4).contains(&vs) || tq > 3 {
                            return broken(pos);
                        }
                        comps.push(Comp { id: c[0], h: hs, v: vs, tq });
                    }
                    let scale = if p == 12 { 16 } else { 1 };
                    self.frame = Frame {
                        w,
                        h,
                        hmax: comps.iter().map(|c| c.h).max().unwrap_or(1),
                        vmax: comps.iter().map(|c| c.v).max().unwrap_or(1),
                        comps,
                        dc_max: 1024 * scale,
                        ac_max: 2048 * scale,
                    };
                }
                0xC2 | 0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return Next::End(Outcome::Unsupported),
                0xC4 => {
                    let Some(body) = f.bytes(body_at, body_len) else { return broken(pos) };
                    let mut i = 0;
                    while i < body.len() {
                        if i + 17 > body.len() {
                            return broken(pos);
                        }
                        let (tc, th) = (body[i] >> 4, (body[i] & 15) as usize);
                        let counts = &body[i + 1..i + 17];
                        let total: usize = counts.iter().map(|&c| c as usize).sum();
                        if tc > 1 || th > 3 || total > 256 || i + 17 + total > body.len() {
                            return broken(pos);
                        }
                        let Some(h) = Huff::new(counts, &body[i + 17..i + 17 + total]) else { return broken(pos) };
                        let t = self.tables_to_change(&mut dirty);
                        if tc == 0 {
                            t.dc[th] = Some(Box::new(h));
                        } else {
                            t.ac[th] = Some(Box::new(h));
                        }
                        i += 17 + total;
                    }
                }
                0xDB => {
                    let mut i = 0;
                    while i < body.len() {
                        let (pq, tq) = (body[i] >> 4, (body[i] & 15) as usize);
                        let n = if pq == 0 { 64 } else { 128 };
                        if pq > 1 || tq > 3 || i + 1 + n > body.len() {
                            return broken(pos);
                        }
                        let mut q = [0u16; 64];
                        for (k, v) in q.iter_mut().enumerate() {
                            *v = if pq == 0 { body[i + 1 + k] as u16 } else { be16(&body[i + 1 + 2 * k..]) as u16 };
                        }
                        self.tables_to_change(&mut dirty).q[tq] = q;
                        i += 1 + n;
                    }
                }
                0xDD => {
                    if body.len() < 2 {
                        return broken(pos);
                    }
                    self.restart = be16(&body) as u32;
                }
                0xDA => return self.scan_header(pos, &body, body_at + body_len as u64, scans),
                _ => {}
            }
            pos = body_at + body_len as u64;
        }
    }

    /// The tables being defined: a new version once a scan used the current one.
    fn tables_to_change(&mut self, dirty: &mut bool) -> &mut Tables {
        if !*dirty && !self.points.is_empty() {
            let t = self.versions.last().unwrap().clone();
            self.versions.push(t);
        }
        *dirty = true;
        self.versions.last_mut().unwrap()
    }

    fn scan_header(&mut self, pos: u64, body: &[u8], data: u64, scans: u32) -> Next {
        let broken = Next::End(Outcome::Broken(pos));
        let fr = &self.frame;
        if fr.comps.is_empty() || body.is_empty() {
            return broken;
        }
        let ns = body[0] as usize;
        if ns == 0 || ns > 4 || body.len() < 1 + 2 * ns + 3 {
            return broken;
        }
        let tail = &body[1 + 2 * ns..];
        if tail[0] != 0 || tail[1] != 63 || tail[2] != 0 {
            return Next::End(Outcome::Unsupported);
        }
        let t = self.versions.last().unwrap();
        let mut scan = Scan { ns, ..Default::default() };
        for (k, c) in body[1..1 + 2 * ns].chunks(2).enumerate() {
            let Some(comp) = fr.comps.iter().position(|x| x.id == c[0]) else { return broken };
            let (dc, ac) = ((c[1] >> 4) as usize, (c[1] & 15) as usize);
            if dc > 3 || ac > 3 || t.dc[dc].is_none() || t.ac[ac].is_none() {
                return broken;
            }
            let (bh, bv) = if ns == 1 { (1, 1) } else { (fr.comps[comp].h, fr.comps[comp].v) };
            scan.comps[k] = ScanComp { comp, dc, ac, bh, bv };
        }
        let (mx, my) = if ns == 1 {
            let c = &fr.comps[scan.comps[0].comp];
            let cw = (fr.w * c.h as u64).div_ceil(fr.hmax as u64);
            let ch = (fr.h * c.v as u64).div_ceil(fr.vmax as u64);
            (cw.div_ceil(8), ch.div_ceil(8))
        } else {
            (fr.w.div_ceil(8 * fr.hmax as u64), fr.h.div_ceil(8 * fr.vmax as u64))
        };
        scan.mcus_x = mx;
        scan.mcus = mx * my;
        let luma = scan.comps[..ns].iter().find(|c| c.comp == 0);
        let blocks = luma.map_or(0, |c| c.bh * c.bv) as u64;
        self.grid = vec![[0.0; 16]; (scan.mcus * blocks).min(8 << 20) as usize];
        Next::Scan(Point {
            bits: Bits { pos: data, ..Default::default() },
            preds: [0; 4],
            mcu: 0,
            rst: 0,
            left: self.restart,
            tables: self.versions.len() - 1,
            scan,
            scans,
        })
    }

    /// Decodes from `p` to the end of the file, keeping a point every sector or so; with a
    /// limit, stops there and keeps nothing.
    fn go(&mut self, f: &mut Feed, mut p: Point, limit: Option<u64>) -> Run {
        loop {
            match self.scan(f, p, limit) {
                Ok(end) => match self.markers(f, end, p.scans + 1) {
                    Next::Scan(q) => p = q,
                    Next::End(o) => return Run::Out(o),
                },
                Err(r) => return r,
            }
        }
    }

    /// One scan from `p`: where its data ends, or how it stopped.
    fn scan(&mut self, f: &mut Feed, mut p: Point, limit: Option<u64>) -> Result<u64, Run> {
        let Jpeg { frame: fr, versions, restart, points, grid, join, .. } = self;
        let restart = *restart;
        let t = &versions[p.tables];
        let sc = p.scan;
        let luma_blocks = sc.comps[..sc.ns].iter().find(|c| c.comp == 0).map_or(0, |c| c.bh * c.bv);
        let luma_w = sc.comps[..sc.ns].iter().find(|c| c.comp == 0).map_or(1, |c| c.bh);
        let mut last_point = points.last().map_or(0, |q| q.bits.pos);
        let fail = |b: &Bits| Err(if b.stop && b.mark == 0xD9 { Run::Short(b.at()) } else { Run::Out(Outcome::Broken(b.at())) });
        while p.mcu < sc.mcus {
            if restart > 0 && p.mcu > 0 && p.left == 0 {
                if p.bits.real() >= 8 {
                    return fail(&p.bits);
                }
                if !p.bits.stop {
                    p.bits.fill(f);
                }
                if p.bits.real() >= 8 || p.bits.mark != 0xD0 + p.rst {
                    return fail(&p.bits);
                }
                p.bits = Bits { pos: p.bits.pos + 2, ..Default::default() };
                p.preds = [0; 4];
                p.rst = (p.rst + 1) & 7;
                p.left = restart;
            }
            match limit {
                None => {
                    if p.bits.pos >= last_point + SECTOR {
                        points.push(p);
                        last_point = p.bits.pos;
                    }
                }
                Some(lim) => {
                    if let Some(j) = join
                        && j.mcu.is_none()
                        && p.bits.pos > j.cut
                    {
                        j.mcu = Some(p.mcu);
                        // the gaps between the rows just before the join, for comparison
                        if luma_blocks > 0 && p.mcu >= 2 * sc.mcus_x {
                            for m in p.mcu - sc.mcus_x..p.mcu {
                                j.before = add_gaps(grid, j.before, m, sc.mcus_x, luma_w, luma_blocks);
                            }
                        }
                    }
                    // far enough, once the row after the join is scored
                    if p.bits.pos >= lim && join.as_ref().is_none_or(|j| j.mcu.is_some_and(|m0| p.mcu >= m0 + sc.mcus_x)) {
                        return Err(Run::Far);
                    }
                }
            }
            for c in &sc.comps[..sc.ns] {
                let (Some(dc), Some(ac)) = (&t.dc[c.dc], &t.ac[c.ac]) else { return fail(&p.bits) };
                let q = &t.q[fr.comps[c.comp].tq];
                for b in 0..c.bh * c.bv {
                    let Some(s) = p.bits.huff(f, dc) else { return fail(&p.bits) };
                    if s > 11 {
                        return fail(&p.bits);
                    }
                    let Some(v) = p.bits.take(f, s as u32) else { return fail(&p.bits) };
                    p.preds[c.comp] += extend(v, s as u32);
                    let dcv = p.preds[c.comp] * q[0] as i32;
                    if dcv.abs() > fr.dc_max + q[0] as i32 {
                        return fail(&p.bits);
                    }
                    let luma = c.comp == 0 && luma_blocks > 0;
                    let mut blk = [0f32; 64];
                    blk[0] = dcv as f32;
                    let mut k = 1usize;
                    while k < 64 {
                        let Some(rs) = p.bits.huff(f, ac) else { return fail(&p.bits) };
                        let (r, s) = ((rs >> 4) as usize, (rs & 15) as u32);
                        if s == 0 {
                            if r == 15 {
                                k += 16;
                                continue;
                            }
                            break;
                        }
                        k += r;
                        if k > 63 || s > 10 {
                            return fail(&p.bits);
                        }
                        let Some(v) = p.bits.take(f, s) else { return fail(&p.bits) };
                        let a = extend(v, s) * q[k] as i32;
                        if a.abs() > fr.ac_max + q[k] as i32 {
                            return fail(&p.bits);
                        }
                        if luma {
                            blk[ZIGZAG[k] as usize] = a as f32;
                        }
                        k += 1;
                    }
                    if k > 64 {
                        return fail(&p.bits);
                    }
                    if luma && let Some(g) = grid.get_mut(p.mcu as usize * luma_blocks + b) {
                        *g = edges(&blk);
                    }
                }
            }
            if let Some(j) = join
                && let Some(m0) = j.mcu
                && luma_blocks > 0
                && p.mcu >= sc.mcus_x
                && p.mcu < m0 + sc.mcus_x
            {
                j.across = add_gaps(grid, j.across, p.mcu, sc.mcus_x, luma_w, luma_blocks);
            }
            p.mcu += 1;
            p.left = p.left.wrapping_sub(1);
        }
        // the scan is over: what is left are the padding bits, then a marker
        if p.bits.real() >= 8 {
            return fail(&p.bits);
        }
        if !p.bits.stop {
            p.bits.fill(f);
        }
        if p.bits.real() >= 8 || p.bits.mark == 0 {
            return fail(&p.bits);
        }
        Ok(p.bits.pos)
    }

    /// The last point kept at or before offset `cut`.
    fn point_before(&self, cut: u64) -> Option<usize> {
        let i = self.points.partition_point(|p| p.bits.pos <= cut);
        i.checked_sub(1)
    }

    /// Tries the pieces the feed now shows from point `i`: Some(how far the DC values jump at
    /// the join, against the rows before it) when the data reads on cleanly well past `cut`.
    /// Decodes to the end of the file only when `whole`, else stops a little past the join.
    fn trial(&mut self, f: &mut Feed, i: usize, cut: u64, whole: bool) -> Option<(f64, Outcome)> {
        let p = self.points[i];
        self.join = Some(Join { cut, ..Default::default() });
        let far = if whole { u64::MAX } else { cut + TRIAL };
        let r = self.go(f, p, Some(far));
        let j = self.join.take().unwrap_or_default();
        let out = match r {
            Run::Out(Outcome::Done(e)) => Outcome::Done(e),
            Run::Out(Outcome::Broken(e)) if e >= cut + TRIAL => Outcome::Broken(e),
            Run::Far => Outcome::Broken(far),
            _ => return None,
        };
        let mean = |(s, n): (f64, u32)| if n == 0 { None } else { Some(s / n as f64) };
        let score = match (mean(j.across), mean(j.before)) {
            (Some(a), Some(b)) => a / (b + 4.0),
            (Some(a), None) => a / 64.0,
            _ => 1.0,
        };
        Some((score, out))
    }

    /// Whether the pieces the feed now shows read cleanly from point `i` up to `cut`.
    fn reaches(&mut self, f: &mut Feed, i: usize, cut: u64) -> bool {
        let p = self.points[i];
        match self.go(f, p, Some(cut)) {
            Run::Far | Run::Out(Outcome::Done(_)) => true,
            // the data runs out at the cut, a few bytes early since the reader looks ahead
            Run::Out(Outcome::Broken(e)) | Run::Short(e) => e + 16 >= cut,
            Run::Out(Outcome::Unsupported) => false,
        }
    }

    /// Decodes on from point `i` with the pieces now chosen, keeping new points.
    fn resume(&mut self, f: &mut Feed, i: usize) -> Outcome {
        let p = self.points[i];
        self.points.truncate(i + 1);
        match self.go(f, p, None) {
            Run::Out(o) => o,
            Run::Short(e) => Outcome::Broken(e),
            Run::Far => Outcome::Broken(0),
        }
    }
}

/// Adds the gaps between the pixels along the top of the top luma blocks of MCU `m` and those
/// along the bottom of the blocks just above.
fn add_gaps(grid: &[[f32; 16]], (mut s, mut n): (f64, u32), m: u64, mcus_x: u64, w: usize, blocks: usize) -> (f64, u32) {
    let bv = blocks / w;
    let (cur, up) = (m as usize * blocks, (m - mcus_x) as usize * blocks + (bv - 1) * w);
    for h in 0..w {
        if let (Some(a), Some(b)) = (grid.get(cur + h), grid.get(up + h)) {
            s += (0..8).map(|x| (a[x] - b[8 + x]).abs() as f64).sum::<f64>() / 8.0;
            n += 1;
        }
    }
    (s, n)
}

/// Where coefficient `k` of a block, in the order coded, lies in the 8x8 grid (row by row).
const ZIGZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56,
    57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// The pixels along the top (first 8) and the bottom (last 8) of a block, from its dequantized
/// coefficients: the inverse DCT worked out for those two rows only.
fn edges(blk: &[f32; 64]) -> [f32; 16] {
    static COS: std::sync::OnceLock<[[f32; 8]; 8]> = std::sync::OnceLock::new();
    // COS[x][u] = C(u) cos((2x+1)u pi/16)
    let cos = COS.get_or_init(|| {
        let mut t = [[0f32; 8]; 8];
        for (x, row) in t.iter_mut().enumerate() {
            for (u, v) in row.iter_mut().enumerate() {
                let c = if u == 0 { std::f32::consts::FRAC_1_SQRT_2 } else { 1.0 };
                *v = c * ((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI / 16.0).cos();
            }
        }
        t
    });
    // the sums down each column of frequencies, for row 0 and row 7 (where the odd ones flip)
    let (mut top, mut bottom) = ([0f32; 8], [0f32; 8]);
    for v in 0..8 {
        let c = cos[0][v];
        for u in 0..8 {
            let f = blk[v * 8 + u];
            if f != 0.0 {
                top[u] += c * f;
                bottom[u] += if v % 2 == 0 { c * f } else { -c * f };
            }
        }
    }
    let mut out = [0f32; 16];
    for x in 0..8 {
        let (mut a, mut b) = (0f32, 0f32);
        for u in 0..8 {
            a += cos[x][u] * top[u];
            b += cos[x][u] * bottom[u];
        }
        out[x] = a / 4.0;
        out[8 + x] = b / 4.0;
    }
    out
}

/// Whether a sector could be part of the coded data: a 0xFF byte there is only ever followed
/// by a zero, a restart marker or the end marker, and coded data is far from uniform.
pub fn looks_coded(s: &[u8]) -> bool {
    let mut seen = [false; 256];
    let mut distinct = 0;
    for i in 0..s.len() {
        let b = s[i];
        if b == 0xFF && i + 1 < s.len() {
            match s[i + 1] {
                0x00 | 0xD0..=0xD7 => {}
                0xD9 => return true,
                _ => return false,
            }
        }
        if !seen[b as usize] {
            seen[b as usize] = true;
            distinct += 1;
        }
    }
    distinct >= 32
}

/// A place where the first pieces may stop: the offset in the file, the point to decode from,
/// the pieces kept, and where on the disk the cut falls.
struct Cut {
    cut: u64,
    point: usize,
    kept: Vec<(u64, u64)>,
    at: u64,
}

/// Decodes a JPEG that starts at `at`, read straight through.
pub fn measure(src: &mut dyn Source, at: u64, max: u64) -> Outcome {
    let mut f = Feed::new(src, vec![(at, max)]);
    Jpeg::start(&mut f, max).1
}

/// Puts back together a JPEG whose coded data goes wrong before its end.
pub fn rebuild(
    src: &mut dyn Source,
    size: u64,
    at: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
    max_pieces: usize,
    coded: Option<&[u64]>,
) -> Option<Vec<(u64, u64)>> {
    let max = crate::carve::Kind::Jpeg.max_size();
    let first = frag::open_run(at, size, claims, max);
    let mut f = Feed::new(src, vec![first]);
    let (j, mut out) = Jpeg::start(&mut f, max);
    let mut j = j?;
    let mut runs = vec![first];
    let mut sector = vec![0u8; SECTOR as usize];
    loop {
        let err = match out {
            Outcome::Done(end) => return Some(frag::cut(&runs, end)),
            Outcome::Unsupported => return None,
            Outcome::Broken(e) => e,
        };
        if runs.len() >= max_pieces {
            return None;
        }
        let last_start: u64 = runs[..runs.len() - 1].iter().map(|r| r.1).sum();
        let total: u64 = runs.iter().map(|r| r.1).sum();
        // the fault shows a little after the cut: the sector bounds just above it and down,
        // as long as the data reads cleanly up to them
        let top = err.next_multiple_of(SECTOR).min(total);
        let mut cuts: Vec<Cut> = Vec::new();
        for back in 0..8u64 {
            let Some(cut) = top.checked_sub(back * SECTOR) else { break };
            if cut <= last_start || cuts.len() == CUTS {
                break;
            }
            let Some(point) = j.point_before(cut) else { break };
            let kept = frag::cut(&runs, cut);
            f.set_runs(kept.clone());
            if !j.reaches(&mut f, point, cut) {
                continue;
            }
            let Some(at) = f.locate(cut - 1).map(|x| x + 1) else { break };
            cuts.push(Cut { cut, point, kept, at });
        }
        let low = cuts.last()?;
        let mut cands = match coded {
            Some(list) => Candidates::among(low.at, list),
            None => Candidates::new(low.at, size),
        };
        // (score, cut, sector)
        let mut best: Option<(f64, usize, u64)> = None;
        let (mut done, mut smooth) = (None, None);
        let mut tries = 0u32;
        let mut limit = BUDGET;
        let mut spent = 0u64;
        'search: while let Some(s) = cands.next(claims, &low.kept, free) {
            if f.disk_bytes(s, &mut sector) < sector.len() || !looks_coded(&sector) {
                continue;
            }
            tries += 1;
            for (k, c) in cuts.iter().enumerate() {
                if s == c.at || c.kept.iter().any(|&(a, l)| s >= a && s < a + l) {
                    continue;
                }
                let mut tried = c.kept.clone();
                tried.push(frag::open_run(s, size, claims, max));
                f.set_runs(tried);
                let Some((mut score, mut end)) = j.trial(&mut f, c.point, c.cut, false) else { continue };
                // a join worth taking is read on to the end, to see whether it ends the file:
                // always when smoother than any seen yet, the others while the reading spent on
                // them stays small
                if score < ROUGH && (best.is_none_or(|(b, _, _)| score < b) || spent < WHOLE) {
                    let Some(r) = j.trial(&mut f, c.point, c.cut, true) else { continue };
                    (score, end) = r;
                    if let Outcome::Broken(e) = end {
                        spent += e.saturating_sub(c.cut);
                    }
                }
                // the right count of blocks, then the end marker: nothing else does that
                if matches!(end, Outcome::Done(_)) {
                    done = Some((0.0, k, s));
                    break 'search;
                }
                if best.is_none_or(|(b, _, _)| score < b) {
                    best = Some((score, k, s));
                }
                // as smooth across the join as within the picture
                if score < SMOOTH && smooth.is_none() {
                    smooth = Some((score, k, s));
                    limit = limit.min(tries + EXTRA);
                }
            }
            if tries >= limit {
                break;
            }
        }
        // an exact end first, then the nearest smooth join, then the smoothest of the rest
        let (score, k, s) = done.or(smooth).or(best)?;
        if score >= ROUGH {
            return None;
        }
        let Cut { point: i, kept, .. } = cuts.swap_remove(k);
        runs = kept;
        runs.push(frag::open_run(s, size, claims, max));
        f.set_runs(runs.clone());
        out = j.resume(&mut f, i);
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

    fn noise(n: usize, seed: &mut u64) -> Vec<u8> {
        (0..n)
            .map(|_| {
                *seed ^= *seed << 13;
                *seed ^= *seed >> 7;
                *seed ^= *seed << 17;
                (*seed >> 24) as u8
            })
            .collect()
    }

    /// A photo-like picture: smooth shapes and some grain, so its coded data is not trivial.
    fn picture(w: u32, h: u32, seed: u64) -> Vec<u8> {
        let mut s = seed | 1;
        let grain = noise((w * h) as usize, &mut s);
        let img = image::RgbImage::from_fn(w, h, |x, y| {
            let g = grain[(y * w + x) as usize] as u32 / 8;
            let k = seed as f32;
            let fx = (x as f32 / (13.0 + k) + k).sin() * 60.0 + 120.0;
            let fy = (y as f32 / (9.0 + 2.0 * k) + x as f32 / (31.0 + k)).cos() * 50.0 + 100.0;
            image::Rgb([(fx as u32 + g).min(255) as u8, (fy as u32 + g).min(255) as u8, ((x * seed as u32 + y) / 4 % 256) as u8])
        });
        let mut out = Vec::new();
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85);
        enc.encode_image(&img).unwrap();
        out
    }

    fn pad(v: &mut Vec<u8>) {
        while !v.len().is_multiple_of(SECTOR as usize) {
            v.push(0);
        }
    }

    #[test]
    fn whole_picture_ends_at_its_end_marker() {
        let pic = picture(300, 200, 3);
        let mut disk = pic.clone();
        disk.extend(noise(4096, &mut 7));
        match measure(&mut Mem(disk), 0, 1 << 30) {
            Outcome::Done(end) => assert_eq!(end, pic.len() as u64),
            _ => panic!("not read to its end"),
        }
    }

    #[test]
    fn picture_cut_by_random_data_comes_back() {
        let pic = picture(400, 300, 5);
        assert!(pic.len() > 20_000, "{}", pic.len());
        let cut = 12 * SECTOR as usize;
        let mut disk = pic[..cut].to_vec();
        disk.extend(noise(7 * SECTOR as usize, &mut 11));
        let second = disk.len() as u64;
        disk.extend_from_slice(&pic[cut..]);
        pad(&mut disk);
        disk.extend(noise(8192, &mut 13));
        let size = disk.len() as u64;
        let mut m = Mem(disk);
        assert!(matches!(measure(&mut m, 0, 1 << 30), Outcome::Broken(_)));
        let runs = rebuild(&mut m, size, 0, &Claims::default(), &|_| true, 8, None).expect("rebuilt");
        assert_eq!(runs, vec![(0, cut as u64), (second, (pic.len() - cut) as u64)]);
    }

    #[test]
    fn two_pictures_woven_together_come_back() {
        let a = picture(400, 300, 5);
        let b = picture(400, 300, 9);
        let (ca, cb) = (10 * SECTOR as usize, 14 * SECTOR as usize);
        // a1 b1 a2 b2, the way two files written at once end up
        let mut disk = a[..ca].to_vec();
        let b_at = disk.len() as u64;
        disk.extend_from_slice(&b[..cb]);
        let a2 = disk.len() as u64;
        disk.extend_from_slice(&a[ca..]);
        pad(&mut disk);
        let b2 = disk.len() as u64;
        disk.extend_from_slice(&b[cb..]);
        pad(&mut disk);
        disk.extend(noise(8192, &mut 17));
        let size = disk.len() as u64;
        let mut m = Mem(disk);
        let mut claims = Claims::default();
        claims.add(b_at, b_at + SECTOR);
        let ra = rebuild(&mut m, size, 0, &claims, &|_| true, 8, None).expect("a rebuilt");
        assert_eq!(ra, vec![(0, ca as u64), (a2, (a.len() - ca) as u64)]);
        for &(x, l) in &ra {
            claims.add(x, x + l);
        }
        let rb = rebuild(&mut m, size, b_at, &claims, &|_| true, 8, None).expect("b rebuilt");
        assert_eq!(rb, vec![(b_at, cb as u64), (b2, (b.len() - cb) as u64)]);
    }

    #[test]
    fn three_pieces_one_written_before_come_back() {
        let pic = picture(500, 400, 21);
        let (c1, c2) = (8 * SECTOR as usize, 30 * SECTOR as usize);
        assert!(pic.len() > c2 + 4096, "{}", pic.len());
        // the end of the file lies before its start, the middle after it, random data around
        let mut s = 99;
        let mut disk = noise(3 * SECTOR as usize, &mut s);
        let third = disk.len() as u64;
        disk.extend_from_slice(&pic[c2..]);
        pad(&mut disk);
        disk.extend(noise(5 * SECTOR as usize, &mut s));
        let first = disk.len() as u64;
        disk.extend_from_slice(&pic[..c1]);
        disk.extend(noise(9 * SECTOR as usize, &mut s));
        let second = disk.len() as u64;
        disk.extend_from_slice(&pic[c1..c2]);
        disk.extend(noise(4 * SECTOR as usize, &mut s));
        let size = disk.len() as u64;
        let runs = rebuild(&mut Mem(disk), size, first, &Claims::default(), &|_| true, 8, None).expect("rebuilt");
        assert_eq!(runs, vec![(first, c1 as u64), (second, (c2 - c1) as u64), (third, (pic.len() - c2) as u64)]);
    }

    #[test]
    fn no_rest_found_gives_nothing() {
        let pic = picture(400, 300, 5);
        let mut disk = pic[..10 * SECTOR as usize].to_vec();
        disk.extend(noise(64 * SECTOR as usize, &mut 3));
        let size = disk.len() as u64;
        assert!(rebuild(&mut Mem(disk), size, 0, &Claims::default(), &|_| true, 8, None).is_none());
    }
}
