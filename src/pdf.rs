//! PDF files cut in pieces, put back together from their cross-reference table.
//!
//! A PDF ends with a table that gives the offset in the file of each of its objects, and a
//! `startxref` that gives the table's own. Wherever the file was cut, each object still reads
//! `12 0 obj` at the offset the table gives, in whichever piece holds it: the table found near the
//! cut places its piece, the objects then tell which piece each part of the file is in, and the
//! content between two pieces (the end of a stream, checked by inflating it when it is
//! compressed) tells where one stops and the next starts.

use crate::carve::{Kind, Source};
use crate::frag::{self, Claims, Feed, SECTOR};
use memchr::memmem;
use miniz_oxide::inflate::stream::{InflateState, inflate};
use miniz_oxide::{DataFormat, MZError, MZFlush, MZStatus};

/// How far the tables and pieces of a file are looked for.
const AHEAD: u64 = 64 << 20;
const BEHIND: u64 = 32 << 20;
/// Objects of a whole file checked, at most, spread through it.
const CHECKED: usize = 4096;

#[derive(Clone, Copy, PartialEq, Debug)]
enum What {
    /// `%PDF-` at the start of the file
    Head,
    /// a cross-reference table
    Table,
    /// an object, its number and generation
    Obj(u32, u16),
}

#[derive(Clone, Copy, Debug)]
struct Item {
    off: u64,
    what: What,
}

/// Reads bytes of the file from a position: how many it could.
type Get<'a> = dyn FnMut(u64, &mut [u8]) -> usize + 'a;

fn get_vec(get: &mut Get, pos: u64, n: usize) -> Vec<u8> {
    let mut v = vec![0u8; n];
    let k = get(pos, &mut v);
    v.truncate(k);
    v
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | 0)
}

fn skip_ws(b: &[u8], i: &mut usize) -> usize {
    let s = *i;
    while *i < b.len() && is_ws(b[*i]) {
        *i += 1;
    }
    *i - s
}

fn digits(b: &[u8], i: &mut usize) -> Option<u64> {
    let s = *i;
    let mut v = 0u64;
    while *i < b.len() && b[*i].is_ascii_digit() && *i - s < 19 {
        v = v * 10 + (b[*i] - b'0') as u64;
        *i += 1;
    }
    (*i > s).then_some(v)
}

/// `12 0 obj` at the start of `b`: the number, the generation, and where the body starts.
fn obj_header(b: &[u8]) -> Option<(u32, u16, usize)> {
    let mut i = 0;
    let n = digits(b, &mut i)?;
    if skip_ws(b, &mut i) == 0 {
        return None;
    }
    let g = digits(b, &mut i)?;
    if skip_ws(b, &mut i) == 0 || !b[i..].starts_with(b"obj") {
        return None;
    }
    Some((u32::try_from(n).ok()?, u16::try_from(g).ok()?, i + 3))
}

/// Whether the item reads as itself in `b`, the bytes found where it should be.
fn reads_as(b: &[u8], what: What) -> bool {
    match what {
        What::Head => b.starts_with(b"%PDF-"),
        What::Table => b.starts_with(b"xref") && b.get(4).is_some_and(|&c| is_ws(c)),
        What::Obj(n, g) => obj_header(b).is_some_and(|(a, b, _)| a == n && b == g),
    }
}

/// A cross-reference table at `off`: what it places, and the offset of the table before it.
struct Table {
    items: Vec<Item>,
    prev: Option<u64>,
}

fn table(get: &mut Get, off: u64) -> Option<Table> {
    if !reads_as(&get_vec(get, off, 5), What::Table) {
        return None;
    }
    let mut pos = off + 4;
    let mut items = vec![Item { off, what: What::Table }];
    loop {
        let b = get_vec(get, pos, 64);
        let mut i = 0;
        skip_ws(&b, &mut i);
        if b[i..].starts_with(b"trailer") {
            pos += i as u64;
            break;
        }
        let first = digits(&b, &mut i)?;
        if skip_ws(&b, &mut i) == 0 {
            return None;
        }
        let count = digits(&b, &mut i)?;
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        let eol = i;
        while i < b.len() && (b[i] == b'\r' || b[i] == b'\n') && i - eol < 2 {
            i += 1;
        }
        if i == eol || count > 1 << 20 || first > 1 << 24 {
            return None;
        }
        pos += i as u64;
        let data = get_vec(get, pos, (count * 20) as usize);
        if data.len() as u64 != count * 20 {
            return None;
        }
        for (k, r) in data.chunks(20).enumerate() {
            let num = |s: &[u8]| s.iter().all(u8::is_ascii_digit);
            let end = |c: u8| matches!(c, b' ' | b'\r' | b'\n');
            if !(num(&r[..10]) && r[10] == b' ' && num(&r[11..16]) && r[16] == b' ' && end(r[18]) && end(r[19])) {
                return None;
            }
            let o = r[..10].iter().fold(0u64, |v, &c| v * 10 + (c - b'0') as u64);
            let g = r[11..16].iter().fold(0u32, |v, &c| v * 10 + (c - b'0') as u32);
            match r[17] {
                b'n' if o > 0 => items.push(Item { off: o, what: What::Obj((first + k as u64) as u32, g.min(65535) as u16) }),
                b'n' | b'f' => {}
                _ => return None,
            }
        }
        pos += count * 20;
    }
    let t = get_vec(get, pos, 4096);
    let t = &t[..memmem::find(&t, b"startxref").unwrap_or(t.len())];
    let prev = memmem::find(t, b"/Prev").and_then(|i| {
        let mut j = i + 5;
        skip_ws(t, &mut j);
        digits(t, &mut j)
    });
    Some(Table { items, prev })
}

/// The `startxref` offset of a file whose last bytes are `t`, when they end with
/// `startxref <offset> %%EOF`.
fn tail(t: &[u8]) -> Option<u64> {
    let i = memmem::rfind(t, b"startxref")?;
    let mut j = i + 9;
    skip_ws(t, &mut j);
    let s = digits(t, &mut j)?;
    skip_ws(t, &mut j);
    if !t[j..].starts_with(b"%%EOF") {
        return None;
    }
    j += 5;
    skip_ws(t, &mut j);
    (j == t.len()).then_some(s)
}

/// What the first pass makes of a PDF read straight through.
pub enum Check {
    /// its table places every object where it is: its length
    Good(u64),
    /// it has no table at its end, or the table places objects where they are not: cut in pieces
    Bad,
    /// a table this does not read (a compressed one)
    Unsupported,
}

/// Checks the PDF at `at` against its own cross-reference table.
pub fn check(src: &mut dyn Source, at: u64) -> Check {
    let Some((len, _)) = crate::carve::measure(src, at, Kind::Pdf) else { return Check::Unsupported };
    let mut f = Feed::new(src, vec![(at, len)]);
    let from = len.saturating_sub(1100);
    let Some(t) = f.bytes(from, (len - from) as usize) else { return Check::Bad };
    let Some(s) = tail(&t) else { return Check::Bad };
    if s >= len {
        return Check::Bad;
    }
    let mut get = |pos: u64, buf: &mut [u8]| f.read_some(pos, buf);
    let mut items = Vec::new();
    let mut next = Some(s);
    let mut seen = Vec::new();
    while let Some(off) = next.filter(|o| !seen.contains(o) && seen.len() < 64) {
        seen.push(off);
        match table(&mut get, off) {
            Some(t) => {
                items.extend(t.items);
                next = t.prev;
            }
            // a table kept in a compressed stream: not read here
            None if off == s && obj_header(&get_vec(&mut get, off, 32)).is_some() => return Check::Unsupported,
            None => return Check::Bad,
        }
    }
    let step = items.len().div_ceil(CHECKED).max(1);
    for it in items.iter().step_by(step) {
        if it.off >= len || !reads_as(&get_vec(&mut get, it.off, 32), it.what) {
            return Check::Bad;
        }
    }
    Check::Good(len)
}

/// Every place `pat` is found on the disk between `lo` and `hi`.
fn find_all(f: &mut Feed, lo: u64, hi: u64, pat: &[u8]) -> Vec<u64> {
    const STEP: u64 = 4 << 20;
    let mut out = Vec::new();
    let mut buf = vec![0u8; STEP as usize + pat.len()];
    let mut pos = lo;
    while pos < hi {
        let n = (hi - pos + pat.len() as u64 - 1).min(buf.len() as u64) as usize;
        let got = f.disk_read(pos, &mut buf[..n]);
        if got < pat.len() {
            break;
        }
        out.extend(memmem::find_iter(&buf[..got], pat).map(|i| pos + i as u64).filter(|&p| p < hi));
        pos += STEP;
    }
    out
}

fn disk(f: &mut Feed, at: u64, n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    let k = f.disk_bytes(at, &mut b);
    b.truncate(k);
    b
}

/// Puts back together a PDF that does not read straight through: a first piece at `at`, the
/// last placed by its table, those between by the objects the table lists.
pub fn rebuild(
    src: &mut dyn Source,
    size: u64,
    at: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
    max_pieces: usize,
) -> Option<Vec<(u64, u64)>> {
    let first = frag::open_run(at, size, claims, Kind::Pdf.max_size());
    let mut f = Feed::new(src, vec![first]);
    let near = first.0 + first.1;
    let lo = at.saturating_sub(BEHIND);
    let hi = (near + AHEAD).min(size).min(at + 4 * AHEAD);
    // the ends of files: `startxref <offset> %%EOF`, nearest the end of the first piece first
    let mut tails = Vec::new();
    for p in find_all(&mut f, lo, hi, b"startxref") {
        let b = disk(&mut f, p, 48);
        let mut j = 9;
        skip_ws(&b, &mut j);
        let Some(s) = digits(&b, &mut j) else { continue };
        skip_ws(&b, &mut j);
        if !b[j..].starts_with(b"%%EOF") {
            continue;
        }
        j += 5;
        // the end of line after it, as the first pass counts it
        let eol = b[j..].iter().take(2).take_while(|&&c| c == b'\r' || c == b'\n').count();
        tails.push((p.abs_diff(near), p, s, p + (j + eol) as u64));
    }
    tails.sort_unstable();
    for &(_, p, s, e) in tails.iter().take(32) {
        if claims.covering(p).is_some() {
            continue;
        }
        if let Some(runs) = from_tail(&mut f, size, at, first, p, s, e, claims, free, max_pieces) {
            return Some(runs);
        }
    }
    None
}

/// The pieces of the file, given the `startxref` at `p` on the disk, its offset `s`, and where
/// the file ends `e`.
#[allow(clippy::too_many_arguments)]
fn from_tail(
    f: &mut Feed,
    size: u64,
    at: u64,
    first: (u64, u64),
    p: u64,
    s: u64,
    e: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
    max_pieces: usize,
) -> Option<Vec<(u64, u64)>> {
    // an update of the file follows: this is not its end
    let next = disk(f, e, 48);
    let k = next.iter().position(|&c| !is_ws(c)).unwrap_or(next.len());
    if crate::carve::pdf_goes_on(&next[k..]) {
        return None;
    }
    // the table just before it
    let back = p.saturating_sub(4 << 20);
    let b = disk(f, back, (p - back) as usize);
    let x = back + memmem::find_iter(&b, b"xref").filter(|&i| i == 0 || is_ws(b[i - 1])).last()? as u64;
    // which offset of the file it sits at: the one `startxref` gives, or, in a linearized file
    // whose `startxref` gives the table of its first page, the one that table chains to
    let mut offs = vec![s];
    if s < first.1
        && let Some(t) = table(&mut |pos, buf| f.disk_bytes(at + pos, buf), s)
        && let Some(pv) = t.prev
    {
        offs.push(pv);
    }
    for off in offs {
        if off > x || !(x - off).is_multiple_of(SECTOR) || e - (x - off) > Kind::Pdf.max_size() {
            continue;
        }
        if let Some(runs) = place(f, size, at, x - off, off, s, e - (x - off), claims, free, max_pieces) {
            return Some(runs);
        }
    }
    None
}

/// Whether `it` reads as itself with the piece that puts byte 0 of the file at `d` on the disk.
fn fits(f: &mut Feed, it: Item, d: u64, at: u64, claims: &Claims) -> bool {
    let pos = d + it.off;
    if pos >= at + SECTOR && claims.covering(pos).is_some() {
        return false;
    }
    reads_as(&disk(f, pos, 32), it.what)
}

/// The pieces of the file whose last one puts byte 0 at `dz`, its table at `off_x`.
#[allow(clippy::too_many_arguments)]
fn place(
    f: &mut Feed,
    size: u64,
    at: u64,
    dz: u64,
    off_x: u64,
    s: u64,
    len: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
    max_pieces: usize,
) -> Option<Vec<(u64, u64)>> {
    if len <= off_x {
        return None;
    }
    let mut deltas = vec![at, dz];
    let mut tables: Vec<u64> = vec![off_x, s];
    let mut read: Vec<u64> = Vec::new();
    let mut items = vec![Item { off: 0, what: What::Head }];
    let mut pieces: Vec<(u64, usize)> = Vec::new();
    // tables chained to are read where a piece puts them; a piece found may put more of them
    for _round in 0..3 {
        let mut more = false;
        let mut i = 0;
        while i < tables.len() {
            let off = tables[i];
            i += 1;
            if read.contains(&off) || read.len() >= 64 {
                continue;
            }
            for &d in &deltas {
                if let Some(t) = table(&mut |pos, buf| f.disk_bytes(d + pos, buf), off) {
                    items.extend(t.items);
                    tables.extend(t.prev);
                    read.push(off);
                    more = true;
                    break;
                }
            }
        }
        if !read.contains(&off_x) {
            return None;
        }
        if !more && !pieces.is_empty() {
            break;
        }
        items.sort_by_key(|it| it.off);
        items.dedup_by_key(|it| it.off);
        if items.last().is_some_and(|it| it.off >= len) {
            return None;
        }
        // which piece each item is in, in file order
        pieces = vec![(at, 0)];
        let mut i = 0;
        while i < items.len() {
            let cur = pieces.last().unwrap().0;
            let it = items[i];
            if fits(f, it, cur, at, claims) {
                i += 1;
                continue;
            }
            if pieces.len() >= max_pieces {
                return None;
            }
            let used = |d: u64| pieces.iter().any(|p| p.0 == d);
            if !used(dz) && fits(f, it, dz, at, claims) {
                pieces.push((dz, i));
                i += 1;
                continue;
            }
            let near = pieces.last().unwrap().0 + items[i - 1].off;
            let d = middle(f, size, &items[i..], near, dz, at, claims, &pieces)?;
            pieces.push((d, i));
            if !deltas.contains(&d) {
                deltas.push(d);
            }
            i += 1;
        }
        if pieces.last().unwrap().0 != dz {
            return None;
        }
    }
    // the first piece holds more than the header
    if pieces.len() > 1 && pieces[1].1 < 2 {
        return None;
    }
    // where each piece stops and the next starts
    let lengths = Lengths { items: &items, pieces: &pieces };
    let mut cuts = vec![0u64];
    for j in 1..pieces.len() {
        let ic = pieces[j].1;
        let b = boundary(f, &items, ic - 1, ic, pieces[j - 1].0, pieces[j].0, claims, free, &lengths)?;
        if b <= *cuts.last().unwrap() {
            return None;
        }
        cuts.push(b);
    }
    cuts.push(len);
    let runs: Vec<(u64, u64)> = pieces.iter().enumerate().map(|(j, &(d, _))| (d + cuts[j], cuts[j + 1] - cuts[j])).collect();
    // no two pieces on the same sectors
    for (i, a) in runs.iter().enumerate() {
        for b in &runs[i + 1..] {
            if a.0 < b.0 + b.1 && b.0 < a.0 + a.1 {
                return None;
            }
        }
    }
    Some(runs)
}

/// A next piece that starts with the first of `rest`: found on the disk near `near`, its
/// position agreeing with the offset the table gives, the items after it agreeing too.
#[allow(clippy::too_many_arguments)]
fn middle(f: &mut Feed, size: u64, rest: &[Item], near: u64, dz: u64, at: u64, claims: &Claims, pieces: &[(u64, usize)]) -> Option<u64> {
    let it = rest[0];
    let What::Obj(n, g) = it.what else { return None };
    let pat = format!("{n} {g} obj");
    let mut hits = find_all(f, near.saturating_sub(BEHIND), (near + AHEAD).min(size), pat.as_bytes());
    hits.sort_unstable_by_key(|&h| h.abs_diff(near));
    // the items after it that the last piece does not hold
    let follow: Vec<Item> = rest[1..].iter().copied().take(4).collect();
    let follow: Vec<Item> = follow.into_iter().take_while(|&x| !fits(f, x, dz, at, claims)).take(2).collect();
    for h in hits {
        if h < it.off {
            continue;
        }
        let d = h - it.off;
        if !d.is_multiple_of(SECTOR) || d == dz || pieces.iter().any(|p| p.0 == d) || claims.covering(h).is_some() {
            continue;
        }
        if h > 0 && !disk(f, h - 1, 1).first().is_some_and(|&c| is_ws(c)) {
            continue;
        }
        if follow.iter().all(|&x| fits(f, x, d, at, claims)) {
            return Some(d);
        }
    }
    None
}

/// The objects of the file and the piece each is in: the stream lengths kept in objects of their
/// own are read from there.
struct Lengths<'a> {
    items: &'a [Item],
    pieces: &'a [(u64, usize)],
}

impl Lengths<'_> {
    /// The integer that object `n` holds.
    fn value(&self, f: &mut Feed, n: u32) -> Option<u64> {
        let i = self.items.iter().position(|it| matches!(it.what, What::Obj(m, _) if m == n))?;
        let j = self.pieces.partition_point(|p| p.1 <= i) - 1;
        let b = disk(f, self.pieces[j].0 + self.items[i].off, 64);
        let (_, _, mut k) = obj_header(&b)?;
        skip_ws(&b, &mut k);
        digits(&b, &mut k)
    }
}

/// Where the piece holding item `ia` stops and the one holding `ic` starts.
#[allow(clippy::too_many_arguments)]
fn boundary(
    f: &mut Feed,
    items: &[Item],
    ia: usize,
    ic: usize,
    dj: u64,
    dk: u64,
    claims: &Claims,
    free: &dyn Fn(u64) -> bool,
    lengths: &Lengths,
) -> Option<u64> {
    let (ao, co) = (items[ia].off, items[ic].off);
    let lo = (ao + 1).next_multiple_of(SECTOR);
    let mut hi = co / SECTOR * SECTOR;
    // a piece stops before the next file found
    if let Some(n) = claims.next(dj + ao + 1) {
        hi = hi.min(n.saturating_sub(dj) / SECTOR * SECTOR);
    }
    let fine = |b: u64| free(dk + b) && claims.next(dk + b).is_none_or(|n| n > dk + co);
    // a stream length kept in an object of its own, read where its piece puts it
    let head = disk(f, dj + ao, 4096);
    let dict = &head[..memmem::find(&head, b"stream").unwrap_or(head.len())];
    let reflen = match stream_len(dict) {
        Some(Len::Ref(n)) => lengths.value(f, n),
        _ => None,
    };
    // the one right where the next file found starts first, then from the top down
    let mut order: Vec<u64> = Vec::new();
    if let Some(n) = claims.next(dj + ao + 1) {
        let b = n.saturating_sub(dj);
        if b.is_multiple_of(SECTOR) && b >= lo && b <= hi {
            order.push(b);
        }
    }
    let mut b = hi;
    let mut tries = 0;
    for &k in &order {
        if fine(k) && span(f, items[ia], co, k, dj, dk, reflen).is_ok() {
            return Some(k);
        }
    }
    while b >= lo && tries < 4096 {
        tries += 1;
        if !fine(b) {
            b -= SECTOR;
            continue;
        }
        match span(f, items[ia], co, b, dj, dk, reflen) {
            Ok(()) => return Some(b),
            // wrong already before the cut: the cut is no later than that
            Err(p) if p < b => b = p / SECTOR * SECTOR,
            Err(_) => b -= SECTOR,
        }
        if b < SECTOR {
            break;
        }
    }
    None
}

/// Checks the bytes of the file from item `a` to offset `co`, with byte `b` on read from the next
/// piece: where they went wrong if they did.
fn span(f: &mut Feed, a: Item, co: u64, b: u64, dj: u64, dk: u64, reflen: Option<u64>) -> Result<(), u64> {
    let mut get = |pos: u64, buf: &mut [u8]| -> usize {
        let mut done = 0;
        while done < buf.len() {
            let p = pos + done as u64;
            let (at, room) = if p < b { (dj + p, (b - p) as usize) } else { (dk + p, buf.len()) };
            let n = room.min(buf.len() - done);
            let got = f.disk_bytes(at, &mut buf[done..done + n]);
            done += got;
            if got < n {
                break;
            }
        }
        done
    };
    let ao = a.off;
    match a.what {
        What::Head => Ok(()),
        What::Table => {
            let mut g = |pos: u64, buf: &mut [u8]| get(pos, buf);
            table(&mut g, ao).map(|_| ()).ok_or(ao)
        }
        What::Obj(..) => {
            // the object's own end: `endobj`, or a stream first
            let mut pos = ao;
            let (kw, stream) = loop {
                let c = get_vec(&mut get, pos, 4096);
                if c.len() < 16 {
                    return Err(pos);
                }
                let e = memmem::find(&c, b"endobj");
                let s = memmem::find_iter(&c, b"stream").find(|&i| i < 3 || &c[i - 3..i] != b"end");
                match (e, s) {
                    (Some(e), Some(s)) if s < e => break (pos + s as u64, true),
                    (Some(e), _) => break (pos + e as u64, false),
                    (None, Some(s)) => break (pos + s as u64, true),
                    (None, None) => {}
                }
                pos += c.len() as u64 - 8;
                if pos > co {
                    return Err(pos);
                }
            };
            let end = if stream {
                let dict = get_vec(&mut get, ao, (kw - ao).min(64 << 10) as usize);
                let after = get_vec(&mut get, kw + 6, 2);
                let data = kw
                    + 6
                    + match after.as_slice() {
                        [b'\r', b'\n'] => 2,
                        [b'\n', _] | [b'\r', _] => 1,
                        _ => return Err(kw),
                    };
                let len = stream_len(&dict).and_then(|l| match l {
                    Len::Direct(n) => Some(n),
                    Len::Ref(_) => reflen,
                });
                let Some(len) = len else { return Err(kw) };
                let t = get_vec(&mut get, data + len, 64);
                let mut i = 0;
                skip_ws(&t, &mut i);
                if !t[i..].starts_with(b"endstream") {
                    return Err(data + len);
                }
                i += 9;
                skip_ws(&t, &mut i);
                if !t[i..].starts_with(b"endobj") {
                    return Err(data + len);
                }
                if flate(&dict) {
                    inflates(&mut get, data, len)?;
                }
                data + len + i as u64 + 6
            } else {
                kw + 6
            };
            if end > co {
                return Err(end);
            }
            gap(&mut get, end, co)
        }
    }
}

enum Len {
    Direct(u64),
    Ref(u32),
}

/// The `/Length` of a stream's dictionary.
fn stream_len(dict: &[u8]) -> Option<Len> {
    let i = memmem::find_iter(dict, b"/Length").find(|&i| !dict.get(i + 7).is_some_and(|c| c.is_ascii_alphanumeric()))?;
    let mut j = i + 7;
    skip_ws(dict, &mut j);
    let n = digits(dict, &mut j)?;
    if skip_ws(dict, &mut j) > 0
        && digits(dict, &mut j).is_some()
        && skip_ws(dict, &mut j) > 0
        && dict.get(j) == Some(&b'R')
        && !dict.get(j + 1).is_some_and(|c| c.is_ascii_alphanumeric())
    {
        return Some(Len::Ref(u32::try_from(n).ok()?));
    }
    Some(Len::Direct(n))
}

/// Whether the stream is compressed with deflate first.
fn flate(dict: &[u8]) -> bool {
    let Some(i) = memmem::find(dict, b"/Filter") else { return false };
    let mut j = i + 7;
    skip_ws(dict, &mut j);
    if dict.get(j) == Some(&b'[') {
        j += 1;
        skip_ws(dict, &mut j);
    }
    dict[j..].starts_with(b"/FlateDecode") || (dict[j..].starts_with(b"/Fl") && !dict.get(j + 3).is_some_and(|c| c.is_ascii_alphanumeric()))
}

/// Inflates `len` bytes from `at` as a zlib stream, its checksum checked: where it went wrong.
fn inflates(get: &mut Get, at: u64, len: u64) -> Result<(), u64> {
    let mut st = InflateState::new_boxed(DataFormat::Zlib);
    let mut out = vec![0u8; 64 << 10];
    let mut inbuf = vec![0u8; 16 << 10];
    let mut pos = at;
    loop {
        let want = (at + len - pos).min(inbuf.len() as u64) as usize;
        if want == 0 {
            return Err(pos);
        }
        let n = get(pos, &mut inbuf[..want]);
        if n == 0 {
            return Err(pos);
        }
        let mut inp = &inbuf[..n];
        loop {
            let r = inflate(&mut st, inp, &mut out, MZFlush::None);
            inp = &inp[r.bytes_consumed..];
            pos += r.bytes_consumed as u64;
            match r.status {
                Ok(MZStatus::StreamEnd) => return Ok(()),
                Ok(_) | Err(MZError::Buf) => {}
                Err(_) => return Err(pos),
            }
            if (inp.is_empty() && r.bytes_written < out.len()) || (r.bytes_consumed == 0 && r.bytes_written == 0) {
                break;
            }
        }
    }
}

/// Between two objects: blank, comments, or something the table does not list but that reads
/// like a PDF (an object no longer used, the end of an earlier version).
fn gap(get: &mut Get, from: u64, to: u64) -> Result<(), u64> {
    let n = (to - from).min(4096) as usize;
    let b = get_vec(get, from, n);
    if b.len() < n {
        return Err(from + b.len() as u64);
    }
    let mut i = 0;
    loop {
        skip_ws(&b, &mut i);
        if i >= b.len() {
            return Ok(());
        }
        if b[i] == b'%' {
            // a comment, to the end of its line
            match b[i..].iter().position(|&c| c == b'\r' || c == b'\n') {
                Some(k) => i += k,
                None => return Ok(()),
            }
            continue;
        }
        let r = &b[i..];
        let known = obj_header(r).is_some() || r.starts_with(b"xref") || r.starts_with(b"trailer") || r.starts_with(b"startxref");
        return if known { Ok(()) } else { Err(from + i as u64) };
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

    /// A PDF of a few objects, compressed streams among them, and its table.
    fn document(seed: u32) -> Vec<u8> {
        let mut d = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offs = Vec::new();
        for n in 1..=12u32 {
            offs.push(d.len());
            if n % 3 == 0 {
                // text that compresses some, long enough to span sectors
                let words: Vec<String> = noise(3000, seed + n).iter().map(|b| format!("w{} ", b % 61)).collect();
                let z = miniz_oxide::deflate::compress_to_vec_zlib(words.concat().as_bytes(), 6);
                d.extend(format!("{n} 0 obj\n<< /Length {} /Filter /FlateDecode >>\nstream\n", z.len()).as_bytes());
                d.extend(&z);
                d.extend(b"\nendstream\nendobj\n");
            } else {
                d.extend(format!("{n} 0 obj\n<< /Type /Thing /N {n} >>\nendobj\n").as_bytes());
            }
        }
        let x = d.len();
        d.extend(b"xref\n0 13\n0000000000 65535 f \n");
        for o in &offs {
            d.extend(format!("{o:010} 00000 n \n").as_bytes());
        }
        d.extend(format!("trailer\n<< /Size 13 /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n").as_bytes());
        d
    }

    #[test]
    fn whole_pdf_checks_against_its_table() {
        let a = document(1);
        let mut disk = a.clone();
        disk.extend(noise(3000, 2));
        assert!(matches!(check(&mut Mem(disk), 0), Check::Good(n) if n == a.len() as u64));
    }

    #[test]
    fn pdf_in_three_pieces_comes_back() {
        let a = document(3);
        let c1 = (a.len() / 3 / 512) * 512;
        let c2 = (a.len() * 2 / 3 / 512) * 512;
        let mut disk = a[..c1].to_vec();
        let other = disk.len() as u64;
        disk.extend(noise(40 * 512, 4));
        let third = disk.len() as u64;
        disk.extend(&a[c2..]);
        disk.extend(noise(30 * 512 - (a.len() - c2) % 512, 5));
        let second = disk.len() as u64;
        disk.extend(&a[c1..c2]);
        disk.extend(noise(5000, 6));
        let size = disk.len() as u64;
        let mut claims = Claims::default();
        claims.add(other, other + 512);
        let mut src = Mem(disk);
        assert!(matches!(check(&mut src, 0), Check::Bad));
        let runs = rebuild(&mut src, size, 0, &claims, &|_| true, 8).unwrap();
        assert_eq!(runs, vec![(0, c1 as u64), (second, (c2 - c1) as u64), (third, (a.len() - c2) as u64)]);
    }
}
