//! `--bench <dossier>`: builds a disk image whose content is known file by file, deletes
//! everything on it, then lets each way of searching try to bring the files back.
//! A file counts as found only when the copy is identical, byte for byte, to the original.

use crate::carve::Volume;
use crate::engine::{Found, Origin, ScanShared, Wanted, read_found};
use crate::ntfs::make::{REC, name, outside, record, resident, run_list};
use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

const CL: u64 = 4096;
const CLUSTERS: u64 = 65_536; // 256 Mo
const MFT_AT: u64 = 4;
const RECORDS: u64 = 1024;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn fill(&mut self, b: &mut [u8]) {
        for c in b.chunks_mut(8) {
            let v = self.next().to_le_bytes();
            c.copy_from_slice(&v[..c.len()]);
        }
    }
    fn words(&mut self, bytes: usize) -> String {
        const W: &[&str] = &[
            "facture",
            "vacances",
            "maison",
            "jardin",
            "rapport",
            "budget",
            "photo",
            "lettre",
            "contrat",
            "réunion",
            "famille",
            "projet",
            "voiture",
            "banque",
            "école",
            "recette",
            "voyage",
            "assurance",
            "impôts",
            "santé",
        ];
        let mut s = String::with_capacity(bytes + 16);
        while s.len() < bytes {
            s.push_str(W[self.below(W.len() as u64) as usize]);
            s.push(if self.below(12) == 0 { '\n' } else { ' ' });
        }
        s
    }
}

fn picture(r: &mut Rng, w: u32, h: u32) -> image::RgbImage {
    let (a, b) = (r.below(255) as u32, r.below(255) as u32);
    image::RgbImage::from_fn(w, h, |x, y| {
        let n = (x * 7 + y * 13) % 31;
        image::Rgb([((x * 255 / w + a) % 256) as u8, ((y * 255 / h + b) % 256) as u8, (n * 8) as u8])
    })
}

fn jpeg(r: &mut Rng) -> Vec<u8> {
    let (w, h) = (400 + r.below(1600) as u32, 300 + r.below(1200) as u32);
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(picture(r, w, h)).write_to(&mut out, image::ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

fn png(r: &mut Rng) -> Vec<u8> {
    let (w, h) = (200 + r.below(600) as u32, 200 + r.below(500) as u32);
    let mut img = picture(r, w, h);
    for p in img.pixels_mut() {
        p.0[2] = r.next() as u8; // noise, so the file has a realistic size
    }
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn zip_file(r: &mut Rng) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..2 + r.below(4) {
        z.start_file(format!("document{i}.txt"), zip::write::SimpleFileOptions::default()).unwrap();
        let len = 20_000 + r.below(300_000) as usize;
        let text = r.words(len);
        z.write_all(text.as_bytes()).unwrap();
        // some stored, already compressed data too
        let mut noise = vec![0u8; r.below(200_000) as usize];
        r.fill(&mut noise);
        z.start_file(format!("image{i}.bin"), zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))
            .unwrap();
        z.write_all(&noise).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn pdf(r: &mut Rng) -> Vec<u8> {
    let mut text = String::new();
    let len = 10_000 + r.below(300_000) as usize;
    for (i, line) in r.words(len).lines().enumerate() {
        text.push_str(&format!("BT /F1 11 Tf 50 {} Td ({}) Tj ET\n", 800 - (i % 70) * 11, line.replace(['(', ')'], "")));
    }
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut at = Vec::new();
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R >>".to_string(),
        format!("<< /Length {} >>\nstream\n{text}endstream", text.len()),
    ];
    for (i, o) in objs.iter().enumerate() {
        at.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for a in at {
        out.extend_from_slice(format!("{a:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
}

struct Original {
    name: String,
    data: Vec<u8>,
    pieces: usize,
    /// Its entry in the file table was taken by a newer file.
    entry_lost: bool,
}

/// The image, and the deleted files written on it.
fn build(r: &mut Rng) -> (Vec<u8>, Vec<Original>) {
    let mut img = vec![0u8; (CLUSTERS * CL) as usize];
    r.fill(&mut img[(300 * CL) as usize..]); // old data everywhere
    img[3..11].copy_from_slice(b"NTFS    ");
    img[0x0B..0x0D].copy_from_slice(&512u16.to_le_bytes());
    img[0x0D] = 8;
    img[0x28..0x30].copy_from_slice(&(CLUSTERS * 8).to_le_bytes());
    img[0x30..0x38].copy_from_slice(&MFT_AT.to_le_bytes());
    img[0x40] = (-10i8) as u8;
    let table = (MFT_AT * CL) as usize;
    let put = |img: &mut Vec<u8>, n: u64, rec: Vec<u8>| {
        let o = table + n as usize * REC;
        img[o..o + REC].copy_from_slice(&rec);
    };
    let mft_clusters = RECORDS * REC as u64 / CL;
    put(&mut img, 0, record(1, &[name(5, "$MFT"), outside(RECORDS * REC as u64, &run_list(&[(MFT_AT, mft_clusters)]))]));
    put(&mut img, 5, record(3, &[name(5, ".")]));
    let folders = [(16u64, "Photos"), (17, "Documents"), (18, "Archives")];
    for (n, f) in folders {
        put(&mut img, n, record(3, &[name(5, f)]));
    }

    let mut files = Vec::new();
    let mut next = 300u64;
    let mut rec = 24u64;
    let kinds: [(&str, u64, fn(&mut Rng) -> Vec<u8>); 4] = [("jpg", 16, jpeg), ("png", 16, png), ("pdf", 17, pdf), ("zip", 18, zip_file)];
    'fill: for round in 0.. {
        for (ext, folder, make) in kinds {
            let data = make(r);
            let need = (data.len() as u64).div_ceil(CL);
            // one file in three is in pieces, with other data in between
            let pieces = if round % 3 == 1 { 2 + r.below(2) as usize } else { 1 };
            let mut runs = Vec::new();
            let mut left = need;
            for p in 0..pieces {
                next += r.below(24); // something else lies between files
                let n = if p + 1 == pieces { left } else { (left / (pieces - p) as u64).max(1) };
                if next + n + 40 >= CLUSTERS {
                    break 'fill;
                }
                runs.push((next, n));
                next += n + if pieces > 1 { 3 + r.below(30) } else { 0 };
                left -= n;
            }
            let mut written = 0usize;
            for &(c, n) in &runs {
                let at = (c * CL) as usize;
                let len = ((n * CL) as usize).min(data.len() - written);
                img[at..at + len].copy_from_slice(&data[written..written + len]);
                written += len;
            }
            let file_name = format!("{}_{:03}.{ext}", folders.iter().find(|f| f.0 == folder).unwrap().1.to_lowercase(), files.len() + 1);
            let entry_lost = r.below(5) == 0;
            if entry_lost {
                put(&mut img, rec, record(1, &[name(17, "plus-recent.txt"), resident(0x80, b"nouveau contenu")]));
            } else {
                put(&mut img, rec, record(0, &[name(folder, &file_name), outside(data.len() as u64, &run_list(&runs))]));
            }
            rec += 1;
            files.push(Original { name: file_name, data, pieces, entry_lost });
            if rec >= RECORDS {
                break 'fill;
            }
        }
    }
    (img, files)
}

fn digest(b: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

fn content(f: &Found) -> Vec<u8> {
    if let Origin::Staged { path } = &f.origin {
        return std::fs::read(path).unwrap_or_default();
    }
    let mut v = Vec::new();
    let _ = read_found(f, u64::MAX, |b| {
        v.extend_from_slice(b);
        true
    });
    v
}

/// Which originals came back identical, how many came back damaged, how many files in all.
fn score(originals: &[Original], found: &[Found]) -> (Vec<bool>, usize, usize) {
    let mut by_hash: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, o) in originals.iter().enumerate() {
        by_hash.entry(digest(&o.data)).or_default().push(i);
    }
    let mut ok = vec![false; originals.len()];
    let mut damaged = 0;
    for f in found {
        let c = content(f);
        let hit = by_hash.get(&digest(&c)).and_then(|v| v.iter().find(|&&i| originals[i].data == c));
        match hit {
            Some(&i) => ok[i] = true,
            None => {
                let head = &c[..c.len().min(4096)];
                if !head.is_empty() && originals.iter().any(|o| o.data.starts_with(head)) {
                    damaged += 1;
                }
            }
        }
    }
    (ok, damaged, found.len())
}

fn line(label: &str, originals: &[Original], ok: &[bool], damaged: usize, total: usize, secs: f32) {
    let n = originals.len();
    let good = ok.iter().filter(|&&b| b).count();
    let frag: Vec<usize> = (0..n).filter(|&i| originals[i].pieces > 1).collect();
    let frag_ok = frag.iter().filter(|&&i| ok[i]).count();
    println!(
        "{label:<34} {good:>3}/{n} intacts ({:>3.0} %)  en morceaux {frag_ok:>2}/{}  abîmés {damaged:>3}  fichiers rendus {total:>4}  {secs:>6.1} s",
        100.0 * good as f32 / n as f32,
        frag.len()
    );
}

pub fn run(work: &Path) {
    let _ = std::fs::create_dir_all(work);
    let image = work.join("disque-test.img");
    let mut r = Rng(0x5EED_CAFE_F00D_1234);
    let (img, originals) = build(&mut r);
    std::fs::write(&image, &img).expect("écriture de l'image");
    drop(img);
    let device = image.to_string_lossy().to_string();
    let frag = originals.iter().filter(|o| o.pieces > 1).count();
    let lost = originals.iter().filter(|o| o.entry_lost).count();
    println!(
        "Image de {} Mo : {} fichiers effacés (JPG, PNG, PDF, ZIP), dont {frag} en morceaux et {lost} dont l'entrée a été réutilisée.\n",
        (CLUSTERS * CL) >> 20,
        originals.len()
    );
    let ctx = eframe::egui::Context::default();
    let wanted = Wanted::default();
    let cancel = AtomicBool::new(false);

    // the quick look at the file table
    let s = Mutex::new(ScanShared::default());
    let t = Instant::now();
    let file = std::fs::File::open(&image).unwrap();
    let mut vol = Volume { inner: file, size: CLUSTERS * CL };
    let _ = crate::ntfs::scan_volume(&mut vol, &device, "C:", None, &wanted, &s, &cancel, &mut 0, &ctx);
    let quick_secs = t.elapsed().as_secs_f32();
    let quick = std::mem::take(&mut s.lock().unwrap().new_files);
    let (quick_ok, d, n) = score(&originals, &quick);
    line("Analyse rapide (table des fichiers)", &originals, &quick_ok, d, n, quick_secs);

    // our own signature search
    let disk = crate::sys::Disk {
        id: "img".into(),
        root: work.to_path_buf(),
        device: device.clone(),
        label: String::new(),
        fs: "image".into(),
        total: CLUSTERS * CL,
        free: 0,
        removable: false,
        cluster: CL,
        kind: crate::sys::Kind::Hdd,
        model: String::new(),
    };
    let s = Mutex::new(ScanShared::default());
    let t = Instant::now();
    let _ = crate::engine::deep_scan(&disk, &wanted, &s, &cancel, &mut 0, &ctx);
    let carve_secs = t.elapsed().as_secs_f32();
    let carved = std::mem::take(&mut s.lock().unwrap().new_files);
    let (ok, d, n) = score(&originals, &carved);
    line("Recherche par signatures (interne)", &originals, &ok, d, n, carve_secs);

    // PhotoRec
    let mut photorec_ok = None;
    if let Some(exe) = crate::sys::find_photorec() {
        let stage = work.join("photorec");
        let _ = std::fs::remove_dir_all(&stage);
        let job = crate::photorec::Job { exe: &exe, device: device.clone(), free_only: false, stage: &stage, wanted: &wanted };
        let s = Mutex::new(ScanShared::default());
        let t = Instant::now();
        let _ = crate::photorec::scan(&job, &s, &cancel, &mut 0, &ctx);
        let secs = t.elapsed().as_secs_f32();
        let found = std::mem::take(&mut s.lock().unwrap().new_files);
        let (ok, d, n) = score(&originals, &found);
        line("PhotoRec 7.2", &originals, &ok, d, n, secs);
        // as the application does it: the quick scan first, then PhotoRec without what it already listed
        let _ = std::fs::remove_dir_all(&stage);
        let s = Mutex::new(ScanShared::default());
        s.lock().unwrap().known_starts = quick
            .iter()
            .filter_map(|f| match &f.origin {
                Origin::Runs { runs, .. } => runs.first().and_then(|r| r.0),
                _ => None,
            })
            .collect();
        let t = Instant::now();
        let _ = crate::photorec::scan(&job, &s, &cancel, &mut 0, &ctx);
        let secs = t.elapsed().as_secs_f32();
        let mut all = quick.clone();
        all.append(&mut s.lock().unwrap().new_files);
        let (both, d, n) = score(&originals, &all);
        line("Complète (rapide puis PhotoRec)", &originals, &both, d, n, quick_secs + secs);
        photorec_ok = Some(ok);
        let _ = std::fs::remove_dir_all(&stage);
    } else {
        println!("PhotoRec introuvable : comparaison sautée.");
    }
    println!("\nDétail des fichiers en morceaux :");
    for (i, o) in originals.iter().enumerate().filter(|(_, o)| o.pieces > 1) {
        let mark = |b: bool| if b { "oui" } else { "non" };
        println!(
            "  {:<18} {:>8} o en {} morceaux  rapide {}  PhotoRec {}{}",
            o.name,
            o.data.len(),
            o.pieces,
            mark(quick_ok[i]),
            photorec_ok.as_ref().map_or("—", |p| mark(p[i])),
            if o.entry_lost { "  (entrée réutilisée)" } else { "" }
        );
    }
    let _ = std::fs::remove_file(&image);
}
