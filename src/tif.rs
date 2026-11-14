//! TIFF files cut in pieces.
//!
//! A TIFF of uncompressed strips is rows of pixels like a bitmap, so its pieces are found the same
//! way: where the rows stop reading on from the ones above them, the next piece is the free sector
//! that carries them on. What a bitmap does not have is a header at the end: many writers put the
//! directory (the IFD) and the strip tables after the pixels, at an offset the file header gives.
//! When the file is cut, that offset points into some other file. The directory is then looked for
//! on its own: the free sector, near the start of the file, that holds at that place in the sector
//! a directory whose strips fit the file. Where it lies says where the last piece is, and the cut
//! before the last piece is tried there first.

use crate::bmp::{self, Fit, Pic};
use crate::carve::{Kind, Source};
use crate::frag::{self, Candidates, Claims, Feed, SECTOR};

/// Strips a directory may list.
const MAX_STRIPS: u64 = 1 << 17;

/// What the directory tells: the rows of pixels, and where the file ends.
struct Layout {
    pic: Pic,
    end: u64,
}

enum Lay {
    /// a directory that holds together, for a file whose rows can be compared
    Rows(Box<Layout>),
    /// a directory that holds together, for some other kind of picture
    Other,
    /// no directory here
    Bad,
}

fn rd16(le: bool, b: &[u8]) -> u64 {
    let v = [b[0], b[1]];
    (if le { u16::from_le_bytes(v) } else { u16::from_be_bytes(v) }) as u64
}
fn rd32(le: bool, b: &[u8]) -> u64 {
    let v = [b[0], b[1], b[2], b[3]];
    (if le { u32::from_le_bytes(v) } else { u32::from_be_bytes(v) }) as u64
}

/// Byte order and where the directory is, for a plain TIFF.
fn header(h: &[u8]) -> Option<(bool, u64)> {
    let le = match &h[..4] {
        b"II*\0" => true,
        b"MM\0*" => false,
        _ => return None,
    };
    let ifd = rd32(le, &h[4..]);
    (ifd >= 8).then_some((le, ifd))
}

/// Reads the directory at file offset `ifd`, `read` giving bytes of the file.
fn layout(read: &mut dyn FnMut(u64, usize) -> Option<Vec<u8>>, le: bool, ifd: u64, max: u64) -> Lay {
    let Some(n) = read(ifd, 2).map(|b| rd16(le, &b)) else { return Lay::Bad };
    if !(6..=64).contains(&n) {
        return Lay::Bad;
    }
    let Some(t) = read(ifd + 2, n as usize * 12 + 4) else { return Lay::Bad };
    let mut end = ifd + 2 + n * 12 + 4;
    let (mut w, mut h, mut comp, mut photo, mut spp, mut planar) = (0, 0, 1, u64::MAX, 1, 1);
    let mut bps: Vec<u64> = vec![1];
    let (mut offs, mut lens): (Vec<u64>, Vec<u64>) = Default::default();
    let mut other = rd32(le, &t[n as usize * 12..]) != 0;
    let mut last = 0;
    for e in t[..n as usize * 12].chunks(12) {
        let (tag, typ, count) = (rd16(le, e), rd16(le, &e[2..]), rd32(le, &e[4..]));
        if tag <= last {
            return Lay::Bad;
        }
        last = tag;
        let unit = match typ {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 | 13 => 4,
            5 | 10 | 12 => 8,
            _ => return Lay::Bad,
        };
        let total = unit * count;
        let at = rd32(le, &e[8..]);
        if total > 4 {
            if at < 8 || at + total > max {
                return Lay::Bad;
            }
            end = end.max(at + total);
        }
        // the values of a SHORT or LONG entry
        let mut values = |want: u64| -> Option<Vec<u64>> {
            if !matches!(typ, 3 | 4) || count == 0 || count > want {
                return None;
            }
            let raw = if total <= 4 { e[8..12].to_vec() } else { read(at, total as usize)? };
            Some(raw.chunks(unit as usize).take(count as usize).map(|c| if typ == 3 { rd16(le, c) } else { rd32(le, c) }).collect())
        };
        let one = |v: Option<Vec<u64>>| v.and_then(|v| v.first().copied());
        match tag {
            256 => w = one(values(1)).unwrap_or(0),
            257 => h = one(values(1)).unwrap_or(0),
            258 => bps = values(8).unwrap_or_default(),
            259 => comp = one(values(1)).unwrap_or(0),
            262 => photo = one(values(1)).unwrap_or(u64::MAX),
            273 => offs = values(MAX_STRIPS).unwrap_or_default(),
            277 => spp = one(values(1)).unwrap_or(0),
            279 => lens = values(MAX_STRIPS).unwrap_or_default(),
            284 => planar = one(values(1)).unwrap_or(0),
            // more pictures, raw photo data, sub-directories whose extent is not told here
            330 | 34665 | 34853 | 40965 | 50706 => other = true,
            _ => {}
        }
    }
    if w == 0 || h == 0 || offs.is_empty() || offs.len() != lens.len() {
        return Lay::Bad;
    }
    let fits = offs.windows(2).zip(&lens).all(|(o, l)| o[1] == o[0] + l);
    let (data, len) = (offs[0], offs[offs.len() - 1] + lens[lens.len() - 1]);
    if !fits || data < 8 || len > max {
        return Lay::Other;
    }
    end = end.max(len);
    let stride = w * spp;
    let rows = comp == 1
        && !other
        && planar == 1
        && matches!(spp, 1 | 3 | 4)
        && bps.len() as u64 == spp
        && bps.iter().all(|&b| b == 8)
        && matches!(photo, 0 | 1 | 2 | 5 | 6)
        && stride >= 48
        && len - data == stride * h
        && len <= ifd;
    if !rows {
        return Lay::Other;
    }
    Lay::Rows(Box::new(Layout { pic: Pic::raw(data, stride, len), end }))
}

/// How a TIFF that starts at `at` reads straight through.
pub enum Check {
    /// as one picture to its end: its length
    Good(u64),
    /// its directory or its rows are not where they should be
    Bad,
    /// nothing can be told from its pixels
    Unsupported,
}

pub fn measure(src: &mut dyn Source, at: u64) -> Check {
    let max = Kind::Tiff.max_size();
    let mut f = Feed::new(src, vec![(at, max)]);
    let Some((le, ifd)) = f.bytes(0, 8).and_then(|h| header(&h)) else { return Check::Unsupported };
    let lay = layout(&mut |o, n| f.bytes(o, n), le, ifd, max);
    match lay {
        Lay::Bad => Check::Bad,
        Lay::Other => Check::Unsupported,
        Lay::Rows(l) => match bmp::check(&mut f, &l.pic) {
            bmp::Check::Good(_) => Check::Good(l.end),
            bmp::Check::Bad => Check::Bad,
            bmp::Check::Unsupported => Check::Unsupported,
        },
    }
}

/// Looks for the directory near `at`: the free sector that holds, at the place in the sector the
/// header gives, a directory for rows of pixels. Its layout and where it lies on the disk.
fn find(f: &mut Feed, at: u64, size: u64, le: bool, ifd: u64, claims: &Claims, free: &dyn Fn(u64) -> bool) -> Option<(Box<Layout>, u64)> {
    let max = Kind::Tiff.max_size();
    let o = ifd % SECTOR;
    let mut cands = Candidates::new((at + ifd - o).min(size.saturating_sub(SECTOR)) & !(SECTOR - 1), size);
    let mut head = [0u8; 4];
    while let Some(s) = cands.next(claims, &[], free) {
        let q = s + o;
        if f.disk_bytes(q, &mut head) < 4 || !(6..=64).contains(&rd16(le, &head)) || !matches!(rd16(le, &head[2..]), 254 | 256) {
            continue;
        }
        let mut read = |p: u64, n: usize| -> Option<Vec<u8>> {
            let d = (q + p).checked_sub(ifd)?;
            let mut b = vec![0u8; n];
            (f.disk_bytes(d, &mut b) == n).then_some(b)
        };
        if let Lay::Rows(l) = layout(&mut read, le, ifd, max) {
            return Some((l, q));
        }
    }
    None
}

/// Puts back together a TIFF whose directory or rows are not where they should be.
pub fn rebuild(
    src: &mut dyn Source,
    size: u64,
    at: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
    max_pieces: usize,
) -> Option<Vec<(u64, u64)>> {
    let max = Kind::Tiff.max_size();
    let first = frag::open_run(at, size, claims, max);
    let mut f = Feed::new(src, vec![first]);
    let (le, ifd) = header(&f.bytes(0, 8)?)?;
    let straight = if ifd + 2 <= first.1 { layout(&mut |o, n| f.bytes(o, n), le, ifd, max) } else { Lay::Bad };
    let (l, anchor) = match straight {
        Lay::Rows(l) => (l, None),
        Lay::Other => return None,
        Lay::Bad => {
            let (l, q) = find(&mut f, at, size, le, ifd, claims, free)?;
            (l, Some((ifd, q)))
        }
    };
    let end = l.end;
    let runs = bmp::rows(f, &l.pic, Fit { size, claims, free, max_pieces, max, end, anchor })?;
    // the directory found must be where the pieces put it
    if let Some((ifd, q)) = anchor {
        let mut g = Feed::new(src, runs.clone());
        if g.locate(ifd) != Some(q) {
            return None;
        }
    }
    Some(runs)
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

    /// An RGB TIFF of soft shapes and a little grain, one row a strip, the directory and the strip
    /// tables after the pixels.
    fn picture(w: u32, h: u32, seed: u32) -> Vec<u8> {
        let stride = w * 3;
        let mut out = b"II*\0".to_vec();
        out.extend(0u32.to_le_bytes());
        let grain = noise((stride * h) as usize, seed);
        let s = seed as f32;
        for y in 0..h {
            for x in 0..stride {
                let (px, c) = ((x / 3) as f32, (x % 3) as f32);
                let v = 128.0
                    + 60.0 * ((px * 0.031 + s).sin() * (y as f32 * 0.023 + c).cos())
                    + 30.0 * ((px + y as f32) * 0.011 * (1.0 + c)).sin();
                out.push((v + (grain[(y * stride + x) as usize] % 7) as f32 - 3.0).clamp(0.0, 255.0) as u8);
            }
        }
        let offs_at = out.len() as u32;
        for y in 0..h {
            out.extend((8 + y * stride).to_le_bytes());
        }
        let lens_at = out.len() as u32;
        for _ in 0..h {
            out.extend(stride.to_le_bytes());
        }
        let bps_at = out.len() as u32;
        out.extend([8u8, 0, 8, 0, 8, 0]);
        let ifd = out.len() as u32;
        out[4..8].copy_from_slice(&ifd.to_le_bytes());
        let entries: [(u16, u16, u32, u32); 9] = [
            (256, 4, 1, w),
            (257, 4, 1, h),
            (258, 3, 3, bps_at),
            (259, 3, 1, 1),
            (262, 3, 1, 2),
            (273, 4, h, offs_at),
            (277, 3, 1, 3),
            (278, 4, 1, 1),
            (279, 4, h, lens_at),
        ];
        out.extend((entries.len() as u16).to_le_bytes());
        for (tag, typ, count, val) in entries {
            out.extend(tag.to_le_bytes());
            out.extend(typ.to_le_bytes());
            out.extend(count.to_le_bytes());
            out.extend(val.to_le_bytes());
        }
        out.extend(0u32.to_le_bytes());
        out
    }

    fn pad(v: &mut Vec<u8>) {
        v.resize(v.len().next_multiple_of(512), 0);
    }

    #[test]
    fn whole_tiff_reads_to_its_directory() {
        let p = picture(300, 200, 1);
        let mut disk = p.clone();
        pad(&mut disk);
        disk.extend(noise(8192, 2));
        assert!(matches!(measure(&mut Mem(disk), 0), Check::Good(n) if n == p.len() as u64));
    }

    #[test]
    fn tiff_in_three_pieces_comes_back() {
        let p = picture(400, 300, 3);
        let (a, b) = (200 * 512, 450 * 512);
        // [first][noise][third, with the directory][noise][second][noise]
        let mut disk = p[..a].to_vec();
        disk.extend(noise(20 * 512, 4));
        let third_at = disk.len() as u64;
        disk.extend(&p[b..]);
        pad(&mut disk);
        disk.extend(noise(30 * 512, 5));
        let second_at = disk.len() as u64;
        disk.extend(&p[a..b]);
        disk.extend(noise(40 * 512, 6));
        let size = disk.len() as u64;
        let mut src = Mem(disk);
        assert!(matches!(measure(&mut src, 0), Check::Bad));
        let runs = rebuild(&mut src, size, 0, &Claims::default(), &|_| true, 8).unwrap();
        assert_eq!(runs, vec![(0, a as u64), (second_at, (b - a) as u64), (third_at, (p.len() - b) as u64)]);
    }

    #[test]
    fn compressed_tiff_is_left_to_the_structure() {
        let mut p = picture(300, 200, 7);
        // compression 5: the entry for tag 259 is the fourth
        let ifd = u32::from_le_bytes([p[4], p[5], p[6], p[7]]) as usize;
        p[ifd + 2 + 3 * 12 + 8] = 5;
        assert!(matches!(measure(&mut Mem(p), 0), Check::Unsupported));
    }
}
