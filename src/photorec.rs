//! Drives the bundled PhotoRec engine (free software by Christophe Grenier, GPL)
//! in the background: it never shows its own text screen, we read its progress
//! off that screen and pick up the files it saves in a scratch folder.

use crate::engine::{Category, Chance, Eta, FORMATS, Found, Origin, ScanShared, Wanted, date_of, unix_secs};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

pub struct Job<'a> {
    pub exe: &'a Path,
    /// `\\.\X:` for a drive, or the path of a disk image.
    pub device: String,
    /// Only look at the unused part of the drive (where deleted files lie).
    pub free_only: bool,
    pub stage: &'a Path,
    pub wanted: &'a Wanted,
}

fn command_line(job: &Job) -> String {
    let mut cmd = String::from("partition_none,options,keep_corrupted_file_no,");
    let families = job.wanted.families();
    // no family listed: PhotoRec keeps its own default set, which covers every kind
    if !families.is_empty() {
        cmd.push_str("fileopt,everything,disable,");
        for f in families {
            cmd.push_str(f);
            cmd.push_str(",enable,");
        }
    }
    cmd.push_str(if job.free_only { "freespace," } else { "wholespace," });
    cmd.push_str("search");
    cmd
}

#[derive(Default, Clone, Copy)]
enum Esc {
    #[default]
    Text,
    Esc,
    Csi,
    Skip,
}

/// Just enough of a text terminal to read PhotoRec's progress line.
#[derive(Default)]
struct Screen {
    rows: Vec<Vec<u8>>,
    r: usize,
    c: usize,
    state: Esc,
    params: Vec<u8>,
}

impl Screen {
    fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            match self.state {
                Esc::Text => match b {
                    0x1b => self.state = Esc::Esc,
                    b'\r' => self.c = 0,
                    b'\n' => self.r += 1,
                    8 => self.c = self.c.saturating_sub(1),
                    0x20.. => self.put(b),
                    _ => {}
                },
                Esc::Esc => {
                    self.state = match b {
                        b'[' => {
                            self.params.clear();
                            Esc::Csi
                        }
                        b'(' | b')' => Esc::Skip,
                        _ => Esc::Text,
                    }
                }
                Esc::Skip => self.state = Esc::Text,
                Esc::Csi => {
                    if (0x40..=0x7e).contains(&b) {
                        self.csi(b);
                        self.state = Esc::Text;
                    } else {
                        self.params.push(b);
                    }
                }
            }
        }
    }

    fn put(&mut self, b: u8) {
        if self.r >= 200 || self.c >= 4096 {
            return;
        }
        if self.rows.len() <= self.r {
            self.rows.resize(self.r + 1, Vec::new());
        }
        let row = &mut self.rows[self.r];
        if row.len() <= self.c {
            row.resize(self.c + 1, b' ');
        }
        row[self.c] = b;
        self.c += 1;
    }

    fn csi(&mut self, f: u8) {
        if self.params.first() == Some(&b'?') {
            return;
        }
        let s = String::from_utf8_lossy(&self.params).to_string();
        let n: Vec<usize> = s.split(';').map(|x| x.parse().unwrap_or(0)).collect();
        let a = n.first().copied().unwrap_or(0);
        let one = a.max(1);
        match f {
            b'H' | b'f' => {
                self.r = one - 1;
                self.c = n.get(1).copied().unwrap_or(0).max(1) - 1;
            }
            b'd' => self.r = one - 1,
            b'G' => self.c = one - 1,
            b'A' => self.r = self.r.saturating_sub(one),
            b'B' => self.r += one,
            b'C' => self.c += one,
            b'D' => self.c = self.c.saturating_sub(one),
            b'K' => {
                if let Some(row) = self.rows.get_mut(self.r) {
                    if a == 0 {
                        row.truncate(self.c);
                    } else {
                        row.clear();
                    }
                }
            }
            b'J' if a == 2 => self.rows.clear(),
            _ => {}
        }
    }

    /// The lines that tell where PhotoRec is: pass, sectors, files found, time.
    fn status(&self) -> String {
        let keep = ["Pass ", "Elapsed", "Estimated", "files found", "recovered"];
        self.rows
            .iter()
            .map(|r| String::from_utf8_lossy(r).split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|l| keep.iter().any(|k| l.contains(k)))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// (pass, sectors read, sectors to read) from "Pass 1 - Reading sector  1234/56789".
    fn progress(&self) -> Option<(u32, u64, u64)> {
        for row in &self.rows {
            let line = String::from_utf8_lossy(row);
            let Some(i) = line.find("Reading sector") else { continue };
            let pass = line[..i].trim().strip_prefix("Pass ").and_then(|s| s.split_whitespace().next()?.parse().ok()).unwrap_or(1);
            let nums: String = line[i + 14..].trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '/').collect();
            let (done, total) = nums.split_once('/')?;
            return Some((pass, done.parse().ok()?, total.parse().ok()?));
        }
        None
    }
}

/// Files PhotoRec has finished writing that we haven't listed yet.
fn new_files(stage: &Path, all: bool, seen: &HashSet<PathBuf>) -> Vec<(PathBuf, u64, SystemTime)> {
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(stage) else { return out };
    for d in dirs.flatten() {
        if !d.file_name().to_string_lossy().starts_with("recup") || !d.path().is_dir() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(d.path()) else { continue };
        for f in files.flatten() {
            let p = f.path();
            if seen.contains(&p) || f.file_name().eq_ignore_ascii_case("report.xml") {
                continue;
            }
            let Ok(m) = f.metadata() else { continue };
            if m.is_file() && m.len() > 0 {
                out.push((p, m.len(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH)));
            }
        }
    }
    if !all {
        // The newest file may still be growing, and PhotoRec can still throw it away.
        out.sort_by_key(|f| f.2);
        out.pop();
        out.retain(|f| f.2.elapsed().is_ok_and(|e| e > Duration::from_secs(3)));
    }
    out
}

fn friendly_name(path: &Path, cat: Category, counts: &mut std::collections::HashMap<Category, usize>) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = path.extension().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let dot = if ext.is_empty() { String::new() } else { format!(".{ext}") };
    // PhotoRec appends the original name when the file carries it: f0012345_Holidays.docx
    if let Some((_, original)) = stem.split_once('_')
        && !original.trim().is_empty()
    {
        return format!("{original}{dot}");
    }
    let n = counts.entry(cat).or_default();
    *n += 1;
    format!("{}_{:04}{dot}", cat.stem(), *n)
}

/// Runs PhotoRec until it is done or cancelled. `Ok(false)` means it could not
/// do its job at all, so the caller can fall back on the built-in search.
pub fn scan(
    job: &Job,
    s: &Mutex<ScanShared>,
    cancel: &AtomicBool,
    next_id: &mut usize,
    ctx: &eframe::egui::Context,
) -> Result<bool, String> {
    if std::fs::create_dir_all(job.stage).is_err() {
        return Ok(false);
    }
    let mut cmd = Command::new(job.exe);
    cmd.current_dir(job.stage)
        // The app already has the rights it needs; don't let Windows ask a second time.
        .env("__COMPAT_LAYER", "RunAsInvoker")
        .arg("/log")
        .arg("/d")
        .arg(job.stage.join("recup"))
        .arg("/cmd")
        .arg(&job.device)
        .arg(command_line(job))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let Ok(mut child) = cmd.spawn() else {
        s.lock().unwrap().note(t!("PhotoRec n'a pas pu démarrer"));
        return Ok(false);
    };
    s.lock().unwrap().note(format!("photorec /log /d recup /cmd {} {}", job.device, command_line(job)));
    let log_path = job.stage.join("photorec.log");
    let mut log_seen = 0usize;
    let mut last_status = String::new();
    let mut status_at = Instant::now();

    let screen = Arc::new(Mutex::new(Screen::default()));
    if let Some(mut out) = child.stdout.take() {
        let screen = screen.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 16 << 10];
            while let Ok(n) = out.read(&mut buf) {
                if n == 0 {
                    break;
                }
                screen.lock().unwrap().feed(&buf[..n]);
            }
        });
    }

    let wall = SystemTime::now();
    let mut seen = HashSet::new();
    let mut counts = std::collections::HashMap::new();
    let mut saw_progress = false;
    let mut found = 0usize;
    let mut eta: Option<(u32, Eta)> = None;
    let mut problem = None;
    loop {
        let exited = !matches!(child.try_wait(), Ok(None));
        if !exited && cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
        }
        // PhotoRec's own journal, line by line, and its status line every few seconds
        let mut lines = Vec::new();
        if let Ok(text) = std::fs::read(&log_path)
            && text.len() > log_seen
        {
            let fresh = &text[log_seen..];
            let end = if exited { fresh.len() } else { fresh.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1) };
            lines.extend(String::from_utf8_lossy(&fresh[..end]).lines().map(str::trim_end).filter(|l| !l.is_empty()).map(String::from));
            log_seen += end;
        }
        if status_at.elapsed() > Duration::from_secs(3) {
            status_at = Instant::now();
            let now = screen.lock().unwrap().status();
            if !now.is_empty() && now != last_status {
                lines.push(format!("» {now}"));
                last_status = now;
            }
        }
        if !lines.is_empty() {
            let mut g = s.lock().unwrap();
            for l in lines {
                g.note(l);
            }
        }
        let prog = screen.lock().unwrap().progress();
        if let Some((pass, done, total)) = prog.filter(|p| p.2 > 0) {
            saw_progress = true;
            let p = (done as f32 / total as f32).clamp(0.0, 1.0);
            if eta.as_ref().is_none_or(|(n, _)| *n != pass) {
                eta = Some((pass, Eta::new(p)));
            }
            let mut g = s.lock().unwrap();
            g.phase = if pass == 0 {
                t!("Préparation de l'analyse approfondie…").into()
            } else {
                t!("Analyse approfondie du disque…").into()
            };
            // Pass 0 only learns the block size and usually stops early; pass 1 then reads everything.
            g.progress = p;
            g.eta = eta.as_ref().and_then(|(_, e)| e.secs(p));
        }
        let mut batch = Vec::new();
        for (path, size, modified) in new_files(job.stage, exited, &seen) {
            seen.insert(path.clone());
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let cat = Category::from_ext(&ext);
            let short = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if !job.wanted.accepts(&ext, cat, size, false) {
                let _ = std::fs::remove_file(&path); // not what the person is looking for
                s.lock().unwrap().note(t!("écarté {} ({}) : hors des critères", short, crate::app::human_size(size)));
                continue;
            }
            // PhotoRec names a file after the sector where it starts (f0012345.jpg)
            let start = short.strip_prefix('f').and_then(|r| r.split(['.', '_']).next()).and_then(|n| n.parse::<u64>().ok());
            if start.is_some_and(|sec| s.lock().unwrap().known_starts.contains(&(sec * 512))) {
                let _ = std::fs::remove_file(&path);
                s.lock().unwrap().note(t!("déjà trouvé par l'analyse rapide : {}", short));
                continue;
            }
            s.lock().unwrap().note(t!("gardé {} ({})", short, crate::app::human_size(size)));
            // PhotoRec dates a file from what is written inside it when it can.
            let old = wall.duration_since(modified).is_ok_and(|d| d > Duration::from_secs(600));
            batch.push(Found {
                id: *next_id,
                name: friendly_name(&path, cat, &mut counts),
                ext,
                category: cat,
                size,
                place: t!("Retrouvé par l'analyse approfondie").into(),
                date: if old { date_of(modified) } else { None },
                stamp: if old { unix_secs(modified) } else { 0 },
                chance: Chance::Good,
                origin: Origin::Staged { path },
                checked: false,
            });
            *next_id += 1;
        }
        if !batch.is_empty() {
            found += batch.len();
            let mut g = s.lock().unwrap();
            for f in batch {
                g.add(f);
            }
        }
        ctx.request_repaint();
        if exited {
            break;
        }
        if crate::sys::free_space(job.stage).is_some_and(|f| f < 400 << 20) {
            let _ = child.kill();
            problem = Some(t!("La recherche s'est arrêtée : plus assez de place sur le disque de travail.").to_string());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    if let Some(p) = problem {
        return Err(p);
    }
    let finished = std::fs::read_to_string(job.stage.join("photorec.log")).is_ok_and(|l| l.contains("exited normally"));
    Ok(finished || saw_progress || found > 0 || cancel.load(Ordering::Relaxed))
}

/// Started with `--photorec-test <image> <scratch folder> [photos,documents,…]`: runs the
/// engine on a disk image and prints what it found, to check the plumbing without a real drive.
pub fn self_test(image: &str, stage: &str, kinds: Option<&str>) {
    let Some(exe) = crate::sys::find_photorec() else {
        println!("PhotoRec introuvable");
        return;
    };
    // a list of kinds ("photos"), formats ("jpg") and/or a minimum size ("min=1000000")
    let mut wanted = Wanted::default();
    for k in kinds.unwrap_or("").split(',').map(str::trim) {
        if let Some(n) = k.strip_prefix("min=") {
            wanted.min_size = n.parse().unwrap_or(0);
        } else if let Some(c) = Category::ALL.into_iter().find(|c| c.label().eq_ignore_ascii_case(k)) {
            if !wanted.kinds.contains(&c) {
                wanted.kinds.push(c);
            }
        } else if let Some(i) = FORMATS.iter().position(|f| f.label.eq_ignore_ascii_case(k)) {
            if !wanted.kinds.contains(&FORMATS[i].cat) {
                wanted.kinds.push(FORMATS[i].cat);
            }
            wanted.formats.push(i);
        }
    }
    let job = Job { exe: &exe, device: image.into(), free_only: false, stage: Path::new(stage), wanted: &wanted };
    println!("{}", command_line(&job));
    let s = Arc::new(Mutex::new(ScanShared::default()));
    let t = Instant::now();
    let watch = s.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(3));
            let g = watch.lock().unwrap();
            println!("  … {:.0} % , reste {:?} s, {}", g.progress * 100.0, g.eta, g.phase);
        }
    });
    let r = scan(&job, &s, &AtomicBool::new(false), &mut 0, &eframe::egui::Context::default());
    let g = s.lock().unwrap();
    for l in &g.log {
        println!("  | {l}");
    }
    println!("résultat {r:?} en {:.1} s, {} fichiers", t.elapsed().as_secs_f32(), g.new_files.len());
    for f in &g.new_files {
        println!("  {} ({}, {} o) {:?}", f.name, f.category.label(), f.size, f.date);
    }
}
