//! MP3 files (MPEG audio layer III): frames one after the other, each with a header that gives
//! its length. Nothing in the file says how long it is, except the Xing or Info frame most
//! encoders write first, which counts the bytes of the frames.
//!
//! A next piece of a fragmented MP3 is checked by its frames: the header of the next frame must
//! lie exactly where the frame cut by the join ends, in the same format. The bit reservoir makes
//! this much stronger. Each frame says how many bytes back, in the frames before it, its data
//! starts; that is where the data of the frame before ended, and most encoders keep it exact
//! from frame to frame. A frame of another song lands there by chance about once in four hundred.
//! A file whose encoder does not keep it exact is not put back together.

use crate::carve::{Kind, Source};
use crate::frag::{self, Candidates, Claims, Feed, SECTOR};

/// Frames a next piece must carry on with, unless the song ends before.
const NEED: u32 = 8;
/// Free sectors a next piece is looked for among, at most, for each cut.
const BUDGET: u32 = 1 << 22;

/// The length of the MPEG audio layer III frame whose header starts `f`, if it is one.
pub fn frame_len(f: &[u8]) -> Option<u64> {
    const BR1: [u64; 15] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
    const BR2: [u64; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
    if f.len() < 4 || f[0] != 0xFF || f[1] & 0xE0 != 0xE0 {
        return None;
    }
    let version = (f[1] >> 3) & 3; // 3 = MPEG1, 2 = MPEG2, 0 = MPEG2.5
    let layer = (f[1] >> 1) & 3; // 1 = layer III
    let bri = (f[2] >> 4) as usize;
    let sri = ((f[2] >> 2) & 3) as usize;
    if version == 1 || layer != 1 || bri == 0 || bri == 15 || sri == 3 || f[3] & 3 == 2 {
        return None;
    }
    let pad = ((f[2] >> 1) & 1) as u64;
    let base = [44100u64, 48000, 32000][sri];
    let (sr, br, k) = match version {
        3 => (base, BR1[bri], 144),
        2 => (base / 2, BR2[bri], 72),
        _ => (base / 4, BR2[bri], 72),
    };
    Some(k * br * 1000 / sr + pad)
}

/// What stays the same from frame to frame of a song: version, layer and protection, sample
/// rate, mono or not. The bit rate may change.
fn id(f: &[u8]) -> [u8; 3] {
    [f[1], f[2] & 0x0C, (f[3] >> 6 == 3) as u8]
}

/// `n` bits of `b` from bit `p`.
fn bits(b: &[u8], p: usize, n: usize) -> i32 {
    (p..p + n).fold(0, |v, i| (v << 1) | ((b[i >> 3] >> (7 - (i & 7))) & 1) as i32)
}

/// A frame: its length, and from its side information, how far back its data starts and where
/// the data of the next frame should start (`None` when the side information could not be read).
struct Frame {
    len: u64,
    begin: Option<(i32, i32)>,
}

/// The frame whose header starts `h` (up to 40 bytes of it).
fn frame(h: &[u8]) -> Option<Frame> {
    let len = frame_len(h)?;
    let mpeg1 = (h[1] >> 3) & 3 == 3;
    let mono = h[3] >> 6 == 3;
    let crc = if h[1] & 1 == 0 { 2 } else { 0 };
    let side = match (mpeg1, mono) {
        (true, false) => 32,
        (true, true) => 17,
        (false, false) => 17,
        (false, true) => 9,
    };
    let room = len as i32 - 4 - crc as i32 - side as i32;
    let Some(s) = h.get(4 + crc..4 + crc + side) else { return Some(Frame { len, begin: None }) };
    let channels = if mono { 1 } else { 2 };
    // main_data_begin, then per granule and channel a part2_3_length in bits
    let (begin, used, most) = if mpeg1 {
        let mut p = 9 + if mono { 5 + 4 } else { 3 + 8 };
        let mut used = 0;
        for _ in 0..2 * channels {
            used += bits(s, p, 12);
            p += 59;
        }
        (bits(s, 0, 9), used, 511)
    } else {
        let mut p = 8 + channels;
        let mut used = 0;
        for _ in 0..channels {
            used += bits(s, p, 12);
            p += 63;
        }
        (bits(s, 0, 8), used, 255)
    };
    // what is left in the reservoir after this frame, as far as the next frame can reach back
    let left = (begin + room - (used + 7) / 8).min(most);
    Some(Frame { len, begin: Some((begin, left)) })
}

/// A song's frames read through: where they stop, the last frames before it (their place, their
/// length and the reservoir the frame before left), and how they kept to the reservoir.
#[derive(Clone)]
struct Walk {
    stop: u64,
    last: Vec<(u64, u64, Option<i32>)>,
    checked: u32,
    exact: u32,
    /// frames from the offset asked for on, and whether the first two of them kept to it
    after: u32,
    joined: [bool; 2],
}

/// Reads frames from `pos` while they keep the format `id`, `left` the reservoir the frame
/// before left, up to `max`.
fn walk(f: &mut Feed, mut pos: u64, id_: [u8; 3], mut left: Option<i32>, max: u64, from: u64) -> Walk {
    let mut w = Walk { stop: pos, last: Vec::new(), checked: 0, exact: 0, after: 0, joined: [false; 2] };
    let mut h = [0u8; 40];
    while pos < max {
        let n = f.read_some(pos, &mut h);
        if n < 4 || id(&h) != id_ {
            break;
        }
        let Some(fr) = frame(&h[..n]) else { break };
        let kept = match (left, fr.begin) {
            (Some(l), Some((b, _))) => {
                w.checked += 1;
                w.exact += (l == b) as u32;
                l == b
            }
            _ => false,
        };
        if pos >= from {
            if (w.after as usize) < 2 {
                w.joined[w.after as usize] = kept;
            }
            w.after += 1;
        }
        if w.last.len() == 6 {
            w.last.remove(0);
        }
        w.last.push((pos, fr.len, left));
        left = fr.begin.map(|b| b.1);
        pos += fr.len;
        w.stop = pos;
    }
    w
}

/// Where a song's tags end, read from `pos` where its frames stop: an APE tag, then an ID3v1 tag.
fn tags(f: &mut Feed, mut pos: u64) -> u64 {
    let mut b = [0u8; 32];
    for _ in 0..3 {
        if !f.read(pos, &mut b) {
            break;
        }
        if b.starts_with(b"APETAGEX") {
            let size = u32::from_le_bytes(b[12..16].try_into().unwrap()) as u64;
            let flags = u32::from_le_bytes(b[20..24].try_into().unwrap());
            if size > 16 << 20 {
                break;
            }
            // read from its header: the size counts what follows the header
            pos += if flags & (1 << 29) != 0 { 32 + size } else { 32 };
        } else if b.starts_with(b"TAG") && f.read(pos + 127, &mut b[..1]) {
            pos += 128;
        } else {
            break;
        }
    }
    pos
}

/// Whether the song may end where its frames stop: a tag or blank space follows, or nothing.
fn ends(f: &mut Feed, pos: u64) -> bool {
    let mut b = [0u8; 16];
    let n = f.read_some(pos, &mut b);
    n == 0 || b.starts_with(b"TAG") || b.starts_with(b"APETAGEX") || b[..n].iter().all(|&x| x == 0)
}

/// The start of a song: where its frames start, their format, and how many bytes they make if
/// its Xing or Info frame says.
struct Song {
    audio: u64,
    id: [u8; 3],
    bytes: Option<u64>,
}

fn song(f: &mut Feed) -> Option<Song> {
    let h = f.bytes(0, 10)?;
    let audio = if h.starts_with(b"ID3") {
        let tag = h[6..10].iter().fold(0u64, |a, &b| (a << 7) | b as u64);
        10 + tag + if h[5] & 0x10 != 0 { 10 } else { 0 }
    } else {
        0
    };
    let mut first = [0u8; 192];
    let n = f.read_some(audio, &mut first);
    frame_len(&first[..n])?;
    let first = &first[..n];
    let bytes = [&b"Xing"[..], b"Info"].iter().find_map(|t| {
        let i = first.windows(4).take(48).position(|w| w == *t)?;
        let flags = first.get(i + 4..i + 8)?;
        if flags[3] & 3 != 3 {
            return None;
        }
        let b = first.get(i + 12..i + 16)?;
        Some(u32::from_be_bytes(b.try_into().unwrap()) as u64).filter(|&b| b > 0)
    });
    Some(Song { audio, id: id(first), bytes })
}

/// Whether the encoder kept the reservoir exact: then a join can be judged by it.
fn exact(w: &Walk) -> bool {
    w.checked >= 32 && w.exact as u64 * 10 >= w.checked as u64 * 9
}

pub enum Check {
    /// whole, this long, its byte count told by its Info frame
    Good(u64),
    /// its frames stop before the end
    Bad,
    Unsupported,
}

/// Whether the MP3 at `offset` reads through to its end.
pub fn check(src: &mut dyn Source, offset: u64) -> Check {
    let max = Kind::Mp3.max_size();
    let mut f = Feed::new(src, vec![(offset, max)]);
    let Some(s) = song(&mut f) else { return Check::Unsupported };
    let w = walk(&mut f, s.audio, s.id, None, max, u64::MAX);
    match s.bytes.map(|b| s.audio + b) {
        Some(e) if w.stop == e => Check::Good(tags(&mut f, w.stop)),
        Some(e) if w.stop < e && w.last.len() > 1 => Check::Bad,
        // frames read from the middle of a song do not say where it starts
        None if s.audio > 0 && exact(&w) && !ends(&mut f, w.stop) => Check::Bad,
        _ => Check::Unsupported,
    }
}

/// The free sectors where frames of a song start, noted as the disk is read through: where a
/// next piece may lie. Given up past a bound, as `frag::Coded`.
#[derive(Default)]
pub struct Frames {
    list: Vec<u64>,
    full: bool,
}

impl Frames {
    const MAX: usize = 4 << 20;

    /// Looks at the sector at `at`, `b` the bytes from it on (a few kB), unless a file found holds it.
    pub fn see(&mut self, at: u64, b: &[u8], claims: &Claims) {
        if self.full || claims.covering(at).is_some() || !starts_frames(b) {
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

/// Whether three frames of one format follow each other from somewhere in the first frame's
/// length of `b`.
fn starts_frames(b: &[u8]) -> bool {
    let reach = b.len().min(1441);
    let mut i = 0;
    while let Some(j) = b[i..reach].iter().position(|&x| x == 0xFF) {
        let p = i + j;
        let mut q = p;
        let mut n = 0;
        while n < 3 {
            let Some(h) = b.get(q..q + 4) else { break };
            if frame_len(h).is_none() || id(h) != id(&b[p..p + 4]) {
                break;
            }
            q += frame_len(h).unwrap() as usize;
            n += 1;
        }
        if n == 3 {
            return true;
        }
        i = p + 1;
    }
    false
}

/// Puts back together an MP3 whose frames stop before its end, from the free sectors where its
/// frames go on, `list` those of the whole disk when known.
pub fn rebuild(
    src: &mut dyn Source,
    size: u64,
    at: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
    max_pieces: usize,
    list: Option<&[u64]>,
) -> Option<Vec<(u64, u64)>> {
    let max = Kind::Mp3.max_size();
    let first = frag::open_run(at, size, claims, max);
    let mut f = Feed::new(src, vec![first]);
    let s = song(&mut f)?;
    let end = s.bytes.map(|b| s.audio + b);
    let mut runs = vec![first];
    let mut w = walk(&mut f, s.audio, s.id, None, end.unwrap_or(max), u64::MAX);
    if !exact(&w) {
        return None;
    }
    loop {
        if end == Some(w.stop) || (end.is_none() && ends(&mut f, w.stop)) {
            let e = tags(&mut f, w.stop);
            return Some(frag::cut(&runs, e));
        }
        if end.is_some_and(|e| w.stop > e) || runs.len() >= max_pieces {
            break;
        }
        match join(&mut f, size, claims, free, list, &s, &w, &runs, max) {
            Some((r, nw)) => (runs, w) = (r, nw),
            None => break,
        }
    }
    // the pieces put together so far, up to where the frames stop
    (runs.len() > 1).then(|| frag::cut(&runs, end.map_or(w.stop, |e| w.stop.min(e))))
}

/// The next piece of a song whose frames stop at `w.stop`: the pieces with it, and the frames
/// read on through it.
#[allow(clippy::too_many_arguments)]
fn join(
    f: &mut Feed,
    size: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
    list: Option<&[u64]>,
    s: &Song,
    w: &Walk,
    runs: &[(u64, u64)],
    max: u64,
) -> Option<(Vec<(u64, u64)>, Walk)> {
    let piece: u64 = runs[..runs.len() - 1].iter().map(|r| r.1).sum();
    // the cut lies on a sector bound after the start of the last frames read
    let (lo, hi) = (w.last.first()?.0 + 1, w.stop);
    // (cut, frame read from, reservoir before it, where the next header lies)
    let mut cuts = Vec::new();
    for k in (lo.div_ceil(SECTOR)..=hi / SECTOR).rev() {
        let cut = k * SECTOR;
        if cut <= piece {
            continue;
        }
        // read on from the last frame whose side information lies before the cut
        let Some(&(from, _, left)) = w.last.iter().rev().find(|l| l.0 + 40 <= cut).or(w.last.first()) else { continue };
        // the header the frame across the cut leads to
        let next = w.last.iter().rev().find(|l| l.0 + 4 <= cut).map(|l| l.0 + l.1).filter(|&n| n >= cut);
        cuts.push((cut, from, left, next));
    }
    let low = cuts.last()?.0;
    let kept_low = frag::cut(runs, low);
    f.set_runs(kept_low.clone());
    let disk_low = f.locate(low - 1)? + 1;
    let mut cands = match list {
        Some(l) => Candidates::among(disk_low, l),
        None => Candidates::new(disk_low, size),
    };
    let mut h = [0u8; 4];
    let mut tries = 0;
    while let Some(c) = cands.next(claims, &kept_low, free) {
        tries += 1;
        if tries > BUDGET {
            break;
        }
        for &(cut, from, left, next) in &cuts {
            // the header after the cut, where it must lie in this sector's piece
            if let Some(n) = next
                && (f.disk_bytes(c + n - cut, &mut h) < 4 || frame_len(&h).is_none() || id(&h) != s.id)
            {
                continue;
            }
            let mut tried = frag::cut(runs, cut);
            if tried.iter().any(|&(a, l)| c >= a && c < a + l) {
                continue;
            }
            tried.push(frag::open_run(c, size, claims, max));
            f.set_runs(tried.clone());
            let nw = walk(f, from, s.id, left, s.bytes.map_or(max, |b| s.audio + b), cut);
            let done = s.bytes.map_or_else(|| ends(f, nw.stop), |b| nw.stop == s.audio + b);
            if nw.joined == [true; 2] && (nw.after >= NEED || done) {
                return Some((tried, nw));
            }
        }
    }
    f.set_runs(runs.to_vec());
    None
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

    /// An MPEG1 stereo frame at 128 kbit/s, 44.1 kHz, of `len` (417 or 418) bytes, whose data
    /// starts `begin` bytes back and takes `used` bytes; the rest filled with `fill`.
    fn frame_bytes(begin: i32, used: i32, fill: u8) -> Vec<u8> {
        let pad = 1;
        let mut f = vec![0xFF, 0xFB, 0x90 | (pad << 1), 0x44];
        let mut side = vec![0u8; 32];
        let mut put = |p: usize, n: usize, v: i32| {
            for i in 0..n {
                if (v >> (n - 1 - i)) & 1 == 1 {
                    side[(p + i) >> 3] |= 0x80 >> ((p + i) & 7);
                }
            }
        };
        put(0, 9, begin);
        // all of the data in the first granule's first channel
        put(9 + 3 + 8, 12, used * 8);
        f.extend(side);
        f.resize(418, fill);
        f
    }

    /// A song of `n` frames that keep the reservoir exact, each using 370 bytes of data.
    fn song_bytes(n: usize, fill: u8) -> Vec<u8> {
        let mut v = Vec::new();
        let mut left = 0;
        for _ in 0..n {
            v.extend(frame_bytes(left, 370, fill));
            left = (left + 418 - 36 - 370).min(511);
        }
        v
    }

    #[test]
    fn frames_keep_the_reservoir() {
        let s = song_bytes(100, 0x55);
        let mut m = Mem(s.clone());
        let mut f = Feed::new(&mut m, vec![(0, s.len() as u64)]);
        let w = walk(&mut f, 0, id(&s), None, s.len() as u64, u64::MAX);
        assert_eq!(w.stop, s.len() as u64);
        assert_eq!((w.checked, w.exact), (99, 99));
        assert!(exact(&w));
    }

    #[test]
    fn a_song_cut_in_two_is_put_back() {
        // the song's first 40 sectors at 0, the rest at 512 kB, something else between
        let s = [&b"ID3\x04\0\0\0\0\0\0"[..], &song_bytes(400, 0x55)].concat();
        let cut = 40 * 512;
        let mut disk = vec![0x11u8; 1 << 20];
        disk[..cut].copy_from_slice(&s[..cut]);
        disk[512 << 10..(512 << 10) + s.len() - cut].copy_from_slice(&s[cut..]);
        let mut m = Mem(disk);
        assert!(matches!(check(&mut m, 0), Check::Bad));
        let claims = Claims::default();
        let runs = rebuild(&mut m, 1 << 20, 0, &claims, &|_| true, 8, None).unwrap();
        assert_eq!(runs, vec![(0, cut as u64), (512 << 10, (s.len() - cut) as u64)]);
    }

    #[test]
    fn another_song_is_not_taken_for_the_rest() {
        // the same format, but its reservoir does not follow on from the first song's
        let s = song_bytes(400, 0x55);
        let mut other = vec![0u8; 0];
        for i in 0..400 {
            other.extend(frame_bytes(200 + (i % 7), 370, 0x66));
        }
        let cut = 40 * 512;
        let mut disk = vec![0x11u8; 1 << 20];
        disk[..cut].copy_from_slice(&s[..cut]);
        // its frames placed so that one starts where the first song's next header would
        let at = (512 << 10) + 100;
        disk[at..at + 100_000].copy_from_slice(&other[..100_000]);
        let mut m = Mem(disk);
        let claims = Claims::default();
        assert!(rebuild(&mut m, 1 << 20, 0, &claims, &|_| true, 8, None).is_none());
    }

    #[test]
    fn tags_after_the_frames() {
        let mut v = vec![0u8; 0];
        v.extend(b"APETAGEX");
        v.extend(2000u32.to_le_bytes());
        v.extend(40u32.to_le_bytes());
        v.extend(1u32.to_le_bytes());
        v.extend((1u32 << 29 | 1 << 31).to_le_bytes());
        v.resize(32 + 40, 0x20);
        v.extend(b"TAG");
        v.resize(32 + 40 + 128, 0x20);
        v.extend([0x77; 64]);
        let mut m = Mem(v);
        let mut f = Feed::new(&mut m, vec![(0, 1000)]);
        assert_eq!(tags(&mut f, 0), 32 + 40 + 128);
    }
}
