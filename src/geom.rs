//! Where the clusters of a disk lie: a file system hands out whole clusters, so a file, and each
//! piece of a file cut in pieces, starts on one. Knowing the grid, the search for the next piece
//! of a file looks at one sector in eight on a FAT32 or NTFS volume (clusters of 4 KiB), one in
//! sixty-four on an exFAT card (32 KiB), instead of every sector.
//!
//! The grid is read from the boot sector of each volume (behind an MBR or a GPT, or the disk
//! itself) and kept only where the files found whole agree with it: a disk formatted again with
//! other clusters still holds the files of the old one, on the old grid.

use crate::carve::Source;

/// The data area of a volume: `[start, end)` on the disk, clusters of `unit` bytes from `start`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub start: u64,
    pub end: u64,
    pub unit: u64,
}

#[derive(Default, Debug)]
pub struct Grid(Vec<Area>);

impl Grid {
    /// The volumes on a disk image, behind its partition table if it has one.
    pub fn read(src: &mut dyn Source, size: u64) -> Grid {
        Grid(volumes(src, size).into_iter().map(|v| v.1).collect())
    }

    /// Keeps the volumes where most of the files found whole start on the grid: at least 16 of
    /// them, half of them on it. By chance one in eight would be (clusters of 4 KiB) or fewer;
    /// the rest are files held in others (a cover in a song) and headers that only look right.
    pub fn confirm(&mut self, starts: &[u64]) {
        self.0.retain(|a| {
            let inside: Vec<u64> = starts.iter().copied().filter(|&s| s >= a.start && s < a.end).collect();
            let on = inside.iter().filter(|&&s| (s - a.start) % a.unit == 0).count();
            inside.len() >= 16 && on * 2 >= inside.len()
        });
    }

    /// Whether a piece of a file could start at `at`: on a cluster bound inside a known volume,
    /// anywhere elsewhere.
    pub fn allows(&self, at: u64) -> bool {
        self.0.iter().find(|a| at >= a.start && at < a.end).is_none_or(|a| (at - a.start).is_multiple_of(a.unit))
    }

    #[cfg(test)]
    pub fn areas(&self) -> &[Area] {
        &self.0
    }
}

/// The volumes on a disk image, behind its partition table if it has one: where each starts
/// (its boot sector) and its data area.
pub fn volumes(src: &mut dyn Source, size: u64) -> Vec<(u64, Area)> {
    let mut s0 = [0u8; 512];
    if src.read_at(0, &mut s0) < 512 {
        return Vec::new();
    }
    if let Some(a) = volume(src, 0, size) {
        return vec![(0, a)];
    }
    if s0[510..512] != [0x55, 0xAA] {
        return Vec::new();
    }
    let mut starts = Vec::new();
    for e in 0..4 {
        let p = &s0[446 + 16 * e..462 + 16 * e];
        let kind = p[4];
        let lba = u32::from_le_bytes(p[8..12].try_into().unwrap()) as u64;
        if kind == 0xEE {
            starts.extend(gpt(src));
        } else if kind != 0 && lba > 0 {
            starts.push(lba * 512);
        }
    }
    starts.into_iter().filter(|&at| at < size).filter_map(|at| volume(src, at, size).map(|a| (at, a))).collect()
}

fn le16(b: &[u8], at: usize) -> u64 {
    u16::from_le_bytes([b[at], b[at + 1]]) as u64
}

fn le32(b: &[u8], at: usize) -> u64 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as u64
}

fn le64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

/// Where the partitions of a GPT start.
fn gpt(src: &mut dyn Source) -> Vec<u64> {
    let mut h = [0u8; 512];
    if src.read_at(512, &mut h) < 512 || &h[..8] != b"EFI PART" {
        return Vec::new();
    }
    let (lba, count, esize) = (le64(&h, 72), le32(&h, 80).min(256), le32(&h, 84));
    if !(128..=4096).contains(&esize) {
        return Vec::new();
    }
    let mut t = vec![0u8; (count * esize) as usize];
    let got = src.read_at(lba * 512, &mut t);
    t.chunks_exact(esize as usize)
        .take(got / esize as usize)
        .filter(|e| e[..16].iter().any(|&x| x != 0))
        .map(|e| le64(e, 32) * 512)
        .filter(|&s| s > 0)
        .collect()
}

/// The data area of the NTFS, exFAT or FAT volume that starts at `at`, if its boot sector makes
/// sense.
fn volume(src: &mut dyn Source, at: u64, size: u64) -> Option<Area> {
    let mut b = [0u8; 512];
    if src.read_at(at, &mut b) < 512 || b[510..512] != [0x55, 0xAA] {
        return None;
    }
    let area = if &b[3..11] == b"NTFS    " {
        let bps = le16(&b, 11);
        let spc = match b[13] {
            0 => return None,
            x if x > 0x80 => 1u64 << (256 - x as u32).min(31),
            x => x as u64,
        };
        Area { start: at, end: at + le64(&b, 40).saturating_mul(bps), unit: bps * spc }
    } else if &b[3..11] == b"EXFAT   " {
        let (bps_shift, spc_shift) = (b[108] as u32, b[109] as u32);
        if !(9..=12).contains(&bps_shift) || bps_shift + spc_shift > 25 {
            return None;
        }
        let bps = 1u64 << bps_shift;
        let unit = bps << spc_shift;
        let start = at + le32(&b, 88) * bps;
        Area { start, end: start + le32(&b, 92) * unit, unit }
    } else {
        // FAT12, FAT16, FAT32: the data area after the reserved sectors, the FATs and, before
        // FAT32, the root folder
        let fat32 = &b[82..87] == b"FAT32";
        if !fat32 && &b[54..57] != b"FAT" {
            return None;
        }
        let (bps, spc, reserved, fats) = (le16(&b, 11), b[13] as u64, le16(&b, 14), b[16] as u64);
        let fat_size = if fat32 { le32(&b, 36) } else { le16(&b, 22) };
        let root = (le16(&b, 17) * 32).div_ceil(bps.max(1));
        let total = if le16(&b, 19) != 0 { le16(&b, 19) } else { le32(&b, 32) };
        if bps == 0 || spc == 0 || fats == 0 {
            return None;
        }
        let start = at + (reserved + fats * fat_size + root) * bps;
        Area { start, end: at + total * bps, unit: bps * spc }
    };
    let sane = area.unit.is_power_of_two()
        && (512..=32 << 20).contains(&area.unit)
        && area.start % 512 == 0
        && area.start < area.end
        && area.start < size;
    sane.then_some(Area { end: area.end.min(size), ..area })
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

    #[test]
    fn fat32_behind_an_mbr() {
        let mut d = vec![0u8; 1 << 20];
        // MBR: one FAT32 partition at sector 128
        d[446 + 4] = 0x0C;
        d[446 + 8..446 + 12].copy_from_slice(&128u32.to_le_bytes());
        d[510] = 0x55;
        d[511] = 0xAA;
        let p = 128 * 512;
        let b = &mut d[p..p + 512];
        b[11..13].copy_from_slice(&512u16.to_le_bytes());
        b[13] = 8;
        b[14..16].copy_from_slice(&32u16.to_le_bytes());
        b[16] = 2;
        b[32..36].copy_from_slice(&1800u32.to_le_bytes());
        b[36..40].copy_from_slice(&15u32.to_le_bytes());
        b[82..87].copy_from_slice(b"FAT32");
        b[510] = 0x55;
        b[511] = 0xAA;
        let size = d.len() as u64;
        let mut g = Grid::read(&mut Mem(d), size);
        let start = (p + (32 + 2 * 15) * 512) as u64;
        assert_eq!(g.areas(), &[Area { start, end: (p + 1800 * 512) as u64, unit: 4096 }]);
        assert!(g.allows(start + 8192) && !g.allows(start + 512) && g.allows(512));
        // the files found agree: kept; they don't: dropped
        let on: Vec<u64> = (0..20).map(|k| start + k * 4096).collect();
        g.confirm(&on);
        assert_eq!(g.areas().len(), 1);
        let off: Vec<u64> = (0..20).map(|k| start + k * 4096 + 1024).collect();
        g.confirm(&off);
        assert!(g.areas().is_empty() && g.allows(start + 512));
    }
}
