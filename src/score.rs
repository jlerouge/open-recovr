//! An honest yardstick for carving engines, ours or anyone's.
//!
//! `--carve <image> <outdir>` runs the built-in carver over a whole image and writes what it finds.
//! `--bench-score <truth.tsv> <recovered_dir> [image]` compares what any engine recovered with
//! the list of files that were really on the disk.
//!
//! The truth file is tab separated, one file per line, `#` for comments:
//!
//! ```text
//! sha256  head  size  state  pieces  extents  name
//! ```
//!
//! `head` is the sha256 of the first 4 KiB (of the whole file when shorter), to tell a file that
//! came back damaged from junk. `state` is `deleted` (a file we want back) or `live` (still listed by the file system: finding
//! it is neither a success nor junk). `pieces` is how many fragments the file had on disk, `extents`
//! where they lay, as `offset+length` byte ranges counted from the start of the image, separated by
//! commas (`-` when unknown). With the image given, the scorer reads those ranges back to tell the
//! files still intact on disk from those already overwritten: no engine can be blamed for those.

use crate::carve::{self, Source, Volume};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Runs the built-in carver over the whole image, the way the deep search does on a disk with no
/// file system it knows: every sector is looked at, and a file found is jumped over.
pub fn carve_image(image: &Path, out: &Path) {
    let Ok(file) = std::fs::File::open(image) else {
        eprintln!("cannot open {}", image.display());
        std::process::exit(2);
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    std::fs::create_dir_all(out).ok();
    let mut vol = Volume { inner: file, size };
    let t = Instant::now();
    const CHUNK: usize = 8 << 20;
    const STEP: usize = 512;
    let mut buf = vec![0u8; CHUNK];
    let (mut pos, mut found, mut bytes) = (0u64, 0usize, 0u64);
    let mut open = carve::Open::default();
    while pos < size {
        let span = CHUNK.min((size - pos) as usize);
        let got = vol.read_at(pos, &mut buf[..span]);
        if got == 0 {
            pos += span as u64;
            continue;
        }
        let mut off = 0usize;
        while off + 16 <= got {
            let abs = pos + off as u64;
            if let Some(kind) = carve::detect(&buf[off..got.min(off + 512)])
                && open.allows(abs, kind)
                && let Some((len, ext)) = carve::measure(&mut vol, abs, kind)
            {
                open.push(kind, abs + len);
                let path = out.join(format!("f{abs:012}.{ext}"));
                if copy_range(&mut vol, abs, len, &path) {
                    found += 1;
                    bytes += len;
                }
            }
            off += STEP;
        }
        pos += got as u64;
    }
    println!("carved {found} files, {bytes} bytes, in {:.2} s", t.elapsed().as_secs_f64());
}

fn copy_range(src: &mut dyn Source, start: u64, len: u64, path: &Path) -> bool {
    use std::io::Write;
    let Ok(f) = std::fs::File::create(path) else { return false };
    let mut w = std::io::BufWriter::new(f);
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    while done < len {
        let n = ((len - done) as usize).min(buf.len());
        let got = src.read_at(start + done, &mut buf[..n]);
        if got == 0 || w.write_all(&buf[..got]).is_err() {
            return false;
        }
        done += got as u64;
    }
    w.flush().is_ok()
}

struct Truth {
    sha: [u8; 32],
    head: [u8; 32],
    size: u64,
    deleted: bool,
    pieces: u32,
    extents: Vec<(u64, u64)>,
    name: String,
}

impl Truth {
    fn ext(&self) -> String {
        Path::new(&self.name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "?".into())
    }
}

/// How well one file of the truth came back, best first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Status {
    /// byte for byte
    Exact,
    /// the right bytes, followed by a few more: opens like the original
    Trimmable,
    /// starts right, then goes wrong: most often a fragmented file read straight through
    Damaged,
    Missing,
}

fn hex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

fn load_truth(path: &Path) -> Vec<Truth> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| {
        eprintln!("cannot read {}", path.display());
        std::process::exit(2);
    });
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        // Windows PowerShell writes a byte order mark at the start of its files
        let line = line.trim_end_matches('\r').trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let c: Vec<&str> = line.split('\t').collect();
        let parsed = (|| {
            let extents = if c.get(5).is_none_or(|e| *e == "-" || e.is_empty()) {
                Vec::new()
            } else {
                c[5].split(',')
                    .map(|r| {
                        let (o, l) = r.split_once('+')?;
                        Some((o.trim().parse().ok()?, l.trim().parse().ok()?))
                    })
                    .collect::<Option<Vec<_>>>()?
            };
            Some(Truth {
                sha: hex(c.first()?)?,
                head: hex(c.get(1)?)?,
                size: c.get(2)?.parse().ok()?,
                deleted: *c.get(3)? != "live",
                pieces: c.get(4).and_then(|p| p.parse().ok()).unwrap_or(1),
                extents,
                name: c.get(6).unwrap_or(&"").to_string(),
            })
        })();
        match parsed {
            Some(t) if t.size > 0 => out.push(t),
            Some(_) => {}
            None => eprintln!("{}:{}: line skipped", path.display(), n + 1),
        }
    }
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.is_file() {
            // the engines' own logs and reports are not recovered files
            let n = e.file_name().to_string_lossy().to_lowercase();
            if n.ends_with(".log") || n == "report.xml" {
                continue;
            }
            out.push(p);
        }
    }
}

/// Hashes of a recovered file: of all of it, and of its first `n` bytes for each `n` asked.
fn hash_file(path: &Path, cuts: &[u64]) -> Option<(u64, [u8; 32], HashMap<u64, [u8; 32]>)> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut h = Sha256::new();
    let mut at_cut = HashMap::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    let mut next = cuts.iter().copied().peekable();
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        let mut chunk = &buf[..n];
        // stop at each cut on the way to take the hash of what came before it
        while let Some(&c) = next.peek() {
            if c > done + chunk.len() as u64 {
                break;
            }
            let k = (c - done) as usize;
            h.update(&chunk[..k]);
            at_cut.insert(c, h.clone().finalize().into());
            chunk = &chunk[k..];
            done = c;
            next.next();
        }
        h.update(chunk);
        done += chunk.len() as u64;
    }
    Some((done, h.finalize().into(), at_cut))
}

/// Whether a file's bytes still lie intact where the file system had put them.
fn intact(img: &mut dyn Source, t: &Truth) -> Option<bool> {
    if t.extents.is_empty() {
        return None;
    }
    let mut h = Sha256::new();
    let mut left = t.size;
    let mut buf = vec![0u8; 1 << 20];
    for &(off, len) in &t.extents {
        let mut done = 0u64;
        let len = len.min(left);
        while done < len {
            let n = ((len - done) as usize).min(buf.len());
            let got = img.read_at(off + done, &mut buf[..n]);
            if got == 0 {
                return Some(false);
            }
            h.update(&buf[..got]);
            done += got as u64;
        }
        left -= len;
    }
    Some(left == 0 && <[u8; 32]>::from(h.finalize()) == t.sha)
}

const HEAD: u64 = 4096;

pub fn score(truth_path: &Path, recovered: &Path, image: Option<&Path>, label: &str) {
    let truth = load_truth(truth_path);
    if truth.is_empty() {
        eprintln!("no file in {}", truth_path.display());
        std::process::exit(2);
    }
    let t0 = Instant::now();
    // a recovered file is compared on its full hash, on the hash of its first `size` bytes for
    // each size in the truth (a file followed by extra bytes), and on the hash of its first 4 KiB
    let mut by_sha: HashMap<[u8; 32], Vec<usize>> = HashMap::new();
    let mut by_head: HashMap<[u8; 32], Vec<usize>> = HashMap::new();
    let mut cuts: Vec<u64> = vec![HEAD];
    for (i, t) in truth.iter().enumerate() {
        by_sha.entry(t.sha).or_default().push(i);
        by_head.entry(t.head).or_default().push(i);
        cuts.push(t.size);
    }
    cuts.sort_unstable();
    cuts.dedup();

    let ceiling: Vec<Option<bool>> = match image.and_then(|p| std::fs::File::open(p).ok()) {
        Some(f) => {
            let size = f.metadata().map(|m| m.len()).unwrap_or(0);
            let mut vol = Volume { inner: f, size };
            truth.iter().map(|t| if t.deleted { intact(&mut vol, t) } else { None }).collect()
        }
        None => vec![None; truth.len()],
    };

    let mut files = Vec::new();
    walk(recovered, &mut files);
    let mut status = vec![Status::Missing; truth.len()];
    let mut copies = vec![0u32; truth.len()];
    let mut m = Totals::default();
    for p in &files {
        let Some((len, sha, at)) = hash_file(p, &cuts) else { continue };
        m.files += 1;
        m.bytes += len;
        let mut hits: Vec<(Status, usize)> = Vec::new();
        if let Some(v) = by_sha.get(&sha) {
            hits.extend(v.iter().map(|&i| (Status::Exact, i)));
        }
        if hits.is_empty() {
            for (&cut, h) in &at {
                if let Some(v) = by_sha.get(h) {
                    hits.extend(v.iter().filter(|&&i| truth[i].size == cut).map(|&i| (Status::Trimmable, i)));
                }
            }
        }
        if hits.is_empty() {
            // files shorter than 4 KiB have their whole content as head
            let head = at.get(&HEAD).copied().unwrap_or(sha);
            if let Some(v) = by_head.get(&head) {
                hits.extend(v.iter().map(|&i| (Status::Damaged, i)));
            }
        }
        if hits.is_empty() {
            m.junk += 1;
            m.junk_bytes += len;
            continue;
        }
        if hits.iter().all(|&(_, i)| !truth[i].deleted) {
            m.live += 1;
        }
        // one recovered file is one file back: when the same content was on the disk twice, it
        // goes to the copy that gains the most from it
        let (s, i) = hits.into_iter().min_by_key(|&(s, i)| (s, std::cmp::Reverse(status[i]))).unwrap();
        status[i] = status[i].min(s);
        copies[i] += 1;
    }
    report(&truth, &status, &copies, &ceiling, &m, label);
    println!("(scored in {:.1} s)", t0.elapsed().as_secs_f64());
}

#[derive(Default)]
struct Totals {
    files: usize,
    bytes: u64,
    junk: usize,
    junk_bytes: u64,
    live: usize,
}

#[derive(Default, Clone, Copy)]
struct Row {
    total: usize,
    reachable: usize,
    exact: usize,
    trim: usize,
    damaged: usize,
}

impl Row {
    fn add(&mut self, s: Status, reachable: bool) {
        self.total += 1;
        self.reachable += reachable as usize;
        match s {
            Status::Exact => self.exact += 1,
            Status::Trimmable => self.trim += 1,
            Status::Damaged => self.damaged += 1,
            Status::Missing => {}
        }
    }
    fn good(&self) -> usize {
        self.exact + self.trim
    }
    fn line(&self, name: &str) -> String {
        let pct = |a: usize, b: usize| if b == 0 { "-".to_string() } else { format!("{:.1}%", 100.0 * a as f64 / b as f64) };
        format!(
            "{name:<16} {:>6} {:>6} {:>6} {:>6} {:>6}   {:>7} {:>7}",
            self.total,
            self.reachable,
            self.exact,
            self.trim,
            self.damaged,
            pct(self.good(), self.total),
            pct(self.good(), self.reachable)
        )
    }
}

fn report(truth: &[Truth], status: &[Status], copies: &[u32], ceiling: &[Option<bool>], m: &Totals, label: &str) {
    let mut all = Row::default();
    let mut by_ext: std::collections::BTreeMap<String, Row> = Default::default();
    let (mut whole, mut split) = (Row::default(), Row::default());
    let mut dup = 0u32;
    for (i, t) in truth.iter().enumerate().filter(|(_, t)| t.deleted) {
        // a file with no extents known is taken as still on the disk
        let reach = ceiling[i].unwrap_or(true);
        all.add(status[i], reach);
        by_ext.entry(t.ext()).or_default().add(status[i], reach);
        if t.pieces > 1 {
            split.add(status[i], reach)
        } else {
            whole.add(status[i], reach)
        }
        dup += copies[i].saturating_sub(1);
    }
    println!("== {label} ==");
    println!(
        "{:<16} {:>6} {:>6} {:>6} {:>6} {:>6}   {:>7} {:>7}",
        "deleted files", "total", "intact", "exact", "trim", "damag.", "recall", "of int."
    );
    println!("{}", all.line("all"));
    println!("{}", whole.line("  contiguous"));
    println!("{}", split.line("  fragmented"));
    for (e, r) in &by_ext {
        println!("{}", r.line(&format!("  .{e}")));
    }
    let precision = if m.files == 0 { 0.0 } else { 100.0 * (m.files - m.junk) as f64 / m.files as f64 };
    println!(
        "recovered: {} files ({} MB), {} junk ({} MB), {} live files, {} extra copies, precision {:.1}%",
        m.files,
        m.bytes >> 20,
        m.junk,
        m.junk_bytes >> 20,
        m.live,
        dup,
        precision
    );
    // one line for scripts to collect
    println!(
        "RESULT\t{label}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        all.total, all.reachable, all.exact, all.trim, all.damaged, m.files, m.junk, m.junk_bytes
    );
    for (i, t) in truth.iter().enumerate() {
        if t.deleted && status[i] != Status::Exact && std::env::var_os("SCORE_DETAIL").is_some() {
            println!("  {:?}\t{}\t{}", status[i], t.size, t.name);
        }
    }
}
