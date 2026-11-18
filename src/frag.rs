//! Fragmented files: the second pass of the carver.
//!
//! The first pass finds every header and keeps the files that read straight through. A file whose
//! own structure goes wrong before its end was most likely cut in pieces by the file system, the
//! rest written further on (or before) wherever there was room. Its format tells where the first
//! piece stops making sense; the next piece is then looked for among the sectors no other file
//! claimed, nearest first, and accepted only when the format reads on from it without a fault.

use crate::carve::{Kind, Source};

pub const SECTOR: u64 = 512;
/// How far from the cut a next piece is looked for.
const AHEAD: u64 = 64 << 20;
const BEHIND: u64 = 32 << 20;
/// Pieces in a file before giving up on it.
const MAX_PIECES: usize = 8;
/// Windows a file is read through.
const WINDOWS: usize = 4;

/// A file seen through its pieces on the disk, in file order: `(disk offset, length)`.
pub struct Feed<'a> {
    src: &'a mut dyn Source,
    runs: Vec<(u64, u64)>,
    /// windows on the disk, so going back and forth between pieces and candidates reads nothing
    /// again: where each starts, its bytes, when it was last used
    caches: [(u64, Vec<u8>, u64); WINDOWS],
    last: usize,
    clock: u64,
    warm: bool,
    /// run where the last byte was found, and the file offset that run starts at
    hint: usize,
    hint_base: u64,
    /// where the window last loaded was wanted: a walk back loads the next one behind it
    miss: u64,
}

impl<'a> Feed<'a> {
    pub fn new(src: &'a mut dyn Source, runs: Vec<(u64, u64)>) -> Self {
        Self { src, runs, caches: Default::default(), last: 0, clock: 0, warm: false, hint: 0, hint_base: 0, miss: 0 }
    }

    pub fn set_runs(&mut self, runs: Vec<(u64, u64)>) {
        self.runs = runs;
        self.hint = 0;
        self.hint_base = 0;
    }

    /// The pieces the feed shows.
    pub fn runs(&self) -> &[(u64, u64)] {
        &self.runs
    }

    /// How long the first piece is.
    pub fn first_len(&self) -> u64 {
        self.runs.first().map_or(0, |r| r.1)
    }

    /// Bytes of the disk from `at`, straight from the source: how many could be read.
    pub fn disk_read(&mut self, at: u64, buf: &mut [u8]) -> usize {
        self.src.read_at(at, buf)
    }

    /// Where byte `pos` of the file lies on the disk.
    pub fn locate(&mut self, pos: u64) -> Option<u64> {
        if pos < self.hint_base {
            self.hint = 0;
            self.hint_base = 0;
        }
        while let Some(&(at, len)) = self.runs.get(self.hint) {
            if pos - self.hint_base < len {
                return Some(at + pos - self.hint_base);
            }
            self.hint_base += len;
            self.hint += 1;
        }
        None
    }

    #[inline]
    pub fn byte(&mut self, pos: u64) -> Option<u8> {
        let at = self.locate(pos)?;
        self.disk(at)
    }

    /// Bytes of the disk from `at`, through the windows: how many could be read.
    pub fn disk_bytes(&mut self, at: u64, buf: &mut [u8]) -> usize {
        let mut done = 0;
        while done < buf.len() {
            let pos = at + done as u64;
            // loads the window that holds `pos`, then copies as much of it as is wanted
            let Some(b) = self.disk(pos) else { break };
            buf[done] = b;
            let (s, c, _) = &self.caches[self.last];
            let from = (pos - s) as usize + 1;
            let n = (c.len() - from).min(buf.len() - done - 1);
            buf[done + 1..done + 1 + n].copy_from_slice(&c[from..from + n]);
            done += 1 + n;
        }
        done
    }

    #[inline]
    fn disk(&mut self, at: u64) -> Option<u8> {
        let inside = |(s, c, _): &(u64, Vec<u8>, u64)| at >= *s && at < *s + c.len() as u64;
        if !inside(&self.caches[self.last]) {
            self.clock += 1;
            if let Some(i) = self.caches.iter().position(inside) {
                self.last = i;
            } else {
                // short at first: most headers that look right are not files at all
                let want = if self.warm { 256 << 10 } else { 16 << 10 };
                let back = self.warm && at < self.miss;
                self.warm = true;
                self.miss = at;
                let start = if back { (at + SECTOR).saturating_sub(want as u64) } else { at };
                let mut buf = vec![0u8; want];
                let got = self.src.read_at(start, &mut buf);
                if got == 0 {
                    return None;
                }
                buf.truncate(got);
                // in place of the window used longest ago
                self.last = (0..WINDOWS).min_by_key(|&i| self.caches[i].2).unwrap_or(0);
                self.caches[self.last] = (start, buf, 0);
            }
            self.caches[self.last].2 = self.clock;
        }
        let (s, c, _) = &self.caches[self.last];
        Some(c[(at - s) as usize])
    }

    /// Bytes `pos..pos + buf.len()` of the file; false when the pieces do not reach that far.
    pub fn read(&mut self, pos: u64, buf: &mut [u8]) -> bool {
        let mut done = 0;
        while done < buf.len() {
            let Some(at) = self.locate(pos + done as u64) else { return false };
            // as far as this piece goes
            let (start, len) = self.runs[self.hint];
            let room = (start + len - at) as usize;
            let n = room.min(buf.len() - done);
            if self.disk_bytes(at, &mut buf[done..done + n]) < n {
                return false;
            }
            done += n;
        }
        true
    }

    /// Like `read`, but as many bytes as the pieces hold: how many.
    pub fn read_some(&mut self, pos: u64, buf: &mut [u8]) -> usize {
        let mut done = 0;
        while done < buf.len() {
            let Some(at) = self.locate(pos + done as u64) else { break };
            let (start, len) = self.runs[self.hint];
            let n = ((start + len - at) as usize).min(buf.len() - done);
            let got = self.disk_bytes(at, &mut buf[done..done + n]);
            done += got;
            if got < n {
                break;
            }
        }
        done
    }
    pub fn bytes(&mut self, pos: u64, n: usize) -> Option<Vec<u8>> {
        let mut v = vec![0u8; n];
        self.read(pos, &mut v).then_some(v)
    }
}

/// The sectors that read like the coded data of a JPEG and that no file found whole holds,
/// gathered as the disk is read through. Given up past a bound, to keep memory small on a large
/// disk full of photos: the search near each cut is then used.
#[derive(Default)]
pub struct Coded {
    list: Vec<u64>,
    full: bool,
}

impl Coded {
    const MAX: usize = 4 << 20;

    /// Looks at the sector at `at` (its first bytes in `b`), unless a file found holds it.
    pub fn see(&mut self, at: u64, b: &[u8], claims: &Claims) {
        if self.full || b.len() < SECTOR as usize || claims.covering(at).is_some() || !crate::jpeg::looks_coded(&b[..SECTOR as usize]) {
            return;
        }
        if self.list.len() == Self::MAX {
            self.full = true;
            self.list = Vec::new();
        } else {
            self.list.push(at);
        }
    }

    pub fn list(&self) -> Option<&[u64]> {
        (!self.full).then_some(&self.list[..])
    }
}

/// Byte ranges of the disk that belong to files already found, sorted and disjoint.
#[derive(Default)]
pub struct Claims(Vec<(u64, u64)>);

impl Claims {
    pub fn add(&mut self, start: u64, end: u64) {
        if end <= start {
            return;
        }
        let i = self.0.partition_point(|&(s, _)| s < start);
        self.0.insert(i, (start, end));
        // merge with the neighbours it touches
        let mut j = i.saturating_sub(1);
        while j + 1 < self.0.len() {
            if self.0[j].1 >= self.0[j + 1].0 {
                self.0[j].1 = self.0[j].1.max(self.0[j + 1].1);
                self.0.remove(j + 1);
            } else if j > i {
                break;
            } else {
                j += 1;
            }
        }
    }

    /// The end of the claimed range `at` lies in.
    pub fn covering(&self, at: u64) -> Option<u64> {
        let i = self.0.partition_point(|&(s, _)| s <= at);
        (i > 0 && self.0[i - 1].1 > at).then(|| self.0[i - 1].1)
    }

    /// The start of the first claimed range from `at` on.
    pub fn next(&self, at: u64) -> Option<u64> {
        if self.covering(at).is_some() {
            return Some(at);
        }
        let i = self.0.partition_point(|&(s, _)| s < at);
        self.0.get(i).map(|&(s, _)| s)
    }

    /// The start of the claimed range `at` lies in.
    fn start_of(&self, at: u64) -> Option<u64> {
        let i = self.0.partition_point(|&(s, _)| s <= at);
        (i > 0 && self.0[i - 1].1 > at).then(|| self.0[i - 1].0)
    }
}

/// What the second pass made of a file that did not read straight through.
pub enum Rebuilt {
    /// Its pieces, in file order.
    Pieces(Vec<(u64, u64)>, &'static str),
}

/// The sectors where a next piece may start, nearest the cut first, on either side of it.
pub struct Candidates<'a> {
    ahead: u64,
    behind: u64,
    lo: u64,
    hi: u64,
    forward: bool,
    origin: u64,
    /// when the sectors worth trying are known in advance (sorted): only those, over the whole
    /// disk, by index on each side of the cut
    among: Option<(&'a [u64], usize, usize)>,
}

impl<'a> Candidates<'a> {
    pub fn new(cut_at: u64, size: u64) -> Self {
        Self {
            ahead: cut_at + SECTOR,
            behind: cut_at,
            lo: cut_at.saturating_sub(BEHIND),
            hi: (cut_at + AHEAD).min(size),
            forward: true,
            origin: cut_at,
            among: None,
        }
    }

    /// Only the sectors of `list` (sorted), however far from the cut.
    pub fn among(cut_at: u64, list: &'a [u64]) -> Self {
        let i = list.partition_point(|&s| s <= cut_at);
        Self { among: Some((list, i, i)), ..Self::new(cut_at, 0) }
    }

    /// The next free sector, not in `own`, not claimed by another file.
    pub fn next(&mut self, claims: &Claims, own: &[(u64, u64)], free: &dyn Fn(u64) -> bool) -> Option<u64> {
        let mine = |s: u64| own.iter().any(|&(a, l)| s >= a && s < a + l);
        if let Some((list, ahead, behind)) = &mut self.among {
            loop {
                // as below: behind weighed double
                let a = list.get(*ahead).copied();
                let b = behind.checked_sub(1).map(|i| list[i]);
                let s = match (a, b) {
                    (None, None) => return None,
                    (Some(a), Some(b)) if a - self.origin > 2 * (self.origin - b) => {
                        *behind -= 1;
                        b
                    }
                    (Some(a), _) => {
                        *ahead += 1;
                        a
                    }
                    (None, Some(b)) => {
                        *behind -= 1;
                        b
                    }
                };
                if claims.covering(s).is_none() && !mine(s) && free(s) {
                    return Some(s);
                }
            }
        }
        loop {
            // whichever side is nearer the cut, behind weighed double: file systems mostly
            // write on forward
            let can_ahead = self.ahead < self.hi;
            let can_behind = self.behind > self.lo && self.behind >= SECTOR;
            self.forward = match (can_ahead, can_behind) {
                (false, false) => return None,
                (true, false) => true,
                (false, true) => false,
                (true, true) => self.ahead - self.origin <= 2 * (self.origin - self.behind),
            };
            if self.forward {
                let s = self.ahead;
                if let Some(end) = claims.covering(s) {
                    self.ahead = end.next_multiple_of(SECTOR);
                    continue;
                }
                self.ahead += SECTOR;
                if !mine(s) && free(s) {
                    return Some(s);
                }
            } else {
                if self.behind <= self.lo || self.behind < SECTOR {
                    return None;
                }
                let s = self.behind - SECTOR;
                if let Some(start) = claims.start_of(s) {
                    self.behind = start - start % SECTOR;
                    continue;
                }
                self.behind = s;
                if !mine(s) && free(s) {
                    return Some(s);
                }
            }
        }
    }
}

/// Pieces cut to the first `len` bytes of the file they make.
pub fn cut(runs: &[(u64, u64)], len: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    let mut left = len;
    for &(at, n) in runs {
        if left == 0 {
            break;
        }
        out.push((at, n.min(left)));
        left -= n.min(left);
    }
    out
}

/// The piece that starts at `at` and runs until the next file found, the end of the disk, or
/// the most a file of this kind can hold.
pub fn open_run(at: u64, size: u64, claims: &Claims, max: u64) -> (u64, u64) {
    let end = claims.next(at + SECTOR).unwrap_or(size).min(size).min(at.saturating_add(max));
    (at, end.saturating_sub(at).max(SECTOR.min(size.saturating_sub(at))))
}

/// Puts a file that did not read straight through back together, if its format allows it.
/// The pieces found are claimed, so no other file takes them.
///
/// `coded` lists, when known, every sector of the disk that no file found whole holds and that
/// reads like the coded data of a JPEG: the next piece of a photo is looked for among them,
/// across the whole disk, in place of the sectors near the cut. `frames` lists in the same way
/// the free sectors where the frames of an MP3 start.
#[allow(clippy::too_many_arguments)]
pub fn rebuild(
    src: &mut dyn Source,
    size: u64,
    at: u64,
    kind: Kind,
    claims: &mut Claims,
    free: &dyn Fn(u64) -> bool,
    coded: Option<&[u64]>,
    frames: Option<&[u64]>,
) -> Option<Rebuilt> {
    if kind == Kind::Ole {
        let (runs, ext) = crate::ole::rebuild(src, size, at, claims, free)?;
        for &(a, l) in &runs {
            claims.add(a, a + l);
        }
        return Some(Rebuilt::Pieces(runs, ext));
    }
    let runs = match kind {
        Kind::Jpeg => crate::jpeg::rebuild(src, size, at, claims, free, MAX_PIECES, coded)?,
        Kind::Png => crate::png::rebuild(src, size, at, claims, free, MAX_PIECES)?,
        Kind::Bmp => crate::bmp::rebuild(src, size, at, claims, free, MAX_PIECES)?,
        Kind::Pcx => crate::pcx::rebuild(src, size, at, claims, free, MAX_PIECES)?,
        Kind::Gif => crate::gif::rebuild(src, size, at, claims, free, MAX_PIECES)?,
        Kind::Tiff => crate::tif::rebuild(src, size, at, claims, free, MAX_PIECES)?,
        Kind::Zip => crate::pkzip::rebuild(src, size, at, claims)?,
        Kind::Pdf => crate::pdf::rebuild(src, size, at, claims, free, MAX_PIECES)?,
        // its frames tell a join well enough to follow a song through many pieces
        Kind::Mp3 => crate::mp3::rebuild(src, size, at, claims, free, 4 * MAX_PIECES, frames)?,
        _ => return None,
    };
    for &(a, l) in &runs {
        claims.add(a, a + l);
    }
    let ext = match kind {
        Kind::Jpeg => "jpg",
        Kind::Png => "png",
        Kind::Bmp => "bmp",
        Kind::Pcx => "pcx",
        Kind::Gif => "gif",
        Kind::Tiff => "tif",
        Kind::Pdf => "pdf",
        Kind::Mp3 => "mp3",
        Kind::Zip => crate::carve::measure(src, at, kind).map_or("zip", |m| m.1),
        _ => "",
    };
    Some(Rebuilt::Pieces(runs, ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claims_merge_and_answer() {
        let mut c = Claims::default();
        c.add(100, 200);
        c.add(300, 400);
        c.add(150, 320);
        c.add(1000, 1100);
        assert_eq!(c.0, vec![(100, 400), (1000, 1100)]);
        assert_eq!(c.covering(399), Some(400));
        assert_eq!(c.covering(400), None);
        assert_eq!(c.next(500), Some(1000));
        assert_eq!(c.next(150), Some(150));
        assert_eq!(c.next(2000), None);
    }
}
