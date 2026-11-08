//! The work that runs in the background: searching a drive, copying files back, previews.

use crate::carve::{self, Source, Volume};
use crate::sys::{self, Disk};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Photos,
    Videos,
    Music,
    Documents,
    Archives,
    Other,
}

impl Category {
    pub const ALL: [Category; 6] =
        [Category::Photos, Category::Videos, Category::Music, Category::Documents, Category::Archives, Category::Other];

    pub fn label(self) -> &'static str {
        match self {
            Category::Photos => t!("Photos"),
            Category::Videos => t!("Vidéos"),
            Category::Music => t!("Musique"),
            Category::Documents => t!("Documents"),
            Category::Archives => t!("Archives"),
            Category::Other => t!("Autres"),
        }
    }

    /// The start of the names given to files found without one ("photo_0001.jpg").
    pub fn stem(self) -> &'static str {
        match self {
            Category::Photos => t!("photo"),
            Category::Videos => t!("video"),
            Category::Music => t!("son"),
            Category::Documents => t!("document"),
            Category::Archives => t!("archive"),
            Category::Other => t!("fichier"),
        }
    }

    pub fn from_ext(ext: &str) -> Category {
        match ext.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "heic" | "heif" | "tif" | "tiff" | "raw" | "cr2" | "cr3" | "crw" | "nef"
            | "dng" | "arw" | "orf" | "raf" | "rw2" | "x3f" | "mrw" | "psd" => Category::Photos,
            "mp4" | "mov" | "avi" | "mkv" | "wmv" | "asf" | "3gp" | "m4v" | "mts" | "m2ts" | "webm" | "mpg" | "mpeg" | "flv" | "wtv" => {
                Category::Videos
            }
            "mp3" | "wav" | "m4a" | "flac" | "ogg" | "opus" | "wma" | "aac" | "aif" | "aiff" | "mid" | "au" | "ape" => Category::Music,
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp" | "odg" | "txt" | "rtf" | "csv" | "md"
            | "epub" | "one" | "wpd" | "ps" | "pub" | "msg" | "eml" | "xml" | "html" | "htm" | "json" | "yaml" | "yml" | "ini" | "tex"
            | "c" | "h" | "cpp" | "hpp" | "cc" | "cs" | "java" | "kt" | "py" | "js" | "ts" | "rs" | "go" | "php" | "rb" | "pl" | "sh"
            | "ps1" | "bat" | "sql" | "css" | "swift" | "lua" => Category::Documents,
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "cab" => Category::Archives,
            _ => Category::Other,
        }
    }

    /// The PhotoRec family names that cover this kind of file (checked against PhotoRec 7.2).
    pub fn families(self) -> &'static [&'static str] {
        match self {
            Category::Photos => &["jpg", "png", "gif", "bmp", "tif", "psd", "raw", "crw", "orf", "raf", "rw2", "x3f", "mrw", "riff", "mov"],
            Category::Videos => &["mov", "mkv", "riff", "mpg", "asf", "flv", "m2ts", "wtv"],
            Category::Music => &["mp3", "flac", "ogg", "riff", "aif", "mid", "au", "ape", "asf", "mov"],
            Category::Documents => &["doc", "zip", "pdf", "txt", "xml", "one", "wpd", "ps"],
            Category::Archives => &["zip", "rar", "7z", "gz", "bz2", "xz", "tar", "cab"],
            Category::Other => &[],
        }
    }
}

/// A precise format one can ask for, with the extensions it produces and the
/// PhotoRec families that find it.
pub struct Format {
    pub label: &'static str,
    pub cat: Category,
    pub exts: &'static [&'static str],
    pub families: &'static [&'static str],
}

const fn fmt(label: &'static str, cat: Category, exts: &'static [&'static str], families: &'static [&'static str]) -> Format {
    Format { label, cat, exts, families }
}

impl Format {
    /// The label in the chosen language (most are the same everywhere).
    pub fn name(&self) -> &'static str {
        match self.label {
            "Textes et notes" => t!("Textes et notes"),
            "Code source" => t!("Code source"),
            other => other,
        }
    }
}

pub const FORMATS: &[Format] = &[
    fmt("JPG", Category::Photos, &["jpg", "jpeg"], &["jpg"]),
    fmt("PNG", Category::Photos, &["png"], &["png"]),
    fmt("HEIC", Category::Photos, &["heic", "heif"], &["mov"]),
    fmt(
        "RAW",
        Category::Photos,
        &["cr2", "cr3", "crw", "nef", "dng", "arw", "orf", "raf", "rw2", "x3f", "mrw", "raw"],
        &["tif", "raw", "crw", "orf", "raf", "rw2", "x3f", "mrw"],
    ),
    fmt("GIF", Category::Photos, &["gif"], &["gif"]),
    fmt("BMP", Category::Photos, &["bmp"], &["bmp"]),
    fmt("TIFF", Category::Photos, &["tif", "tiff"], &["tif"]),
    fmt("WEBP", Category::Photos, &["webp"], &["riff"]),
    fmt("Photoshop", Category::Photos, &["psd"], &["psd"]),
    fmt("MP4 / MOV", Category::Videos, &["mp4", "mov", "m4v", "3gp"], &["mov"]),
    fmt("AVI", Category::Videos, &["avi"], &["riff"]),
    fmt("MKV", Category::Videos, &["mkv", "webm"], &["mkv"]),
    fmt("WMV", Category::Videos, &["wmv", "asf"], &["asf"]),
    fmt("MPEG", Category::Videos, &["mpg", "mpeg", "m2ts", "mts"], &["mpg", "m2ts"]),
    fmt("MP3", Category::Music, &["mp3"], &["mp3"]),
    fmt("WAV", Category::Music, &["wav"], &["riff"]),
    fmt("M4A / AAC", Category::Music, &["m4a", "aac"], &["mov"]),
    fmt("FLAC", Category::Music, &["flac"], &["flac"]),
    fmt("OGG", Category::Music, &["ogg", "opus"], &["ogg"]),
    fmt("WMA", Category::Music, &["wma"], &["asf"]),
    fmt("Word", Category::Documents, &["doc", "docx", "odt", "rtf"], &["doc", "zip", "txt"]),
    fmt("Excel", Category::Documents, &["xls", "xlsx", "ods", "csv"], &["doc", "zip", "txt"]),
    fmt("PowerPoint", Category::Documents, &["ppt", "pptx", "odp"], &["doc", "zip"]),
    fmt("PDF", Category::Documents, &["pdf"], &["pdf"]),
    fmt("Textes et notes", Category::Documents, &["txt", "md", "tex", "ini", "yaml", "yml"], &["txt"]),
    fmt(
        "Code source",
        Category::Documents,
        &[
            "c", "h", "cpp", "hpp", "cc", "cs", "java", "kt", "py", "js", "ts", "rs", "go", "php", "rb", "pl", "sh", "ps1", "bat", "sql",
            "css", "swift", "lua",
        ],
        &["txt"],
    ),
    fmt("Web / XML", Category::Documents, &["html", "htm", "xml", "json"], &["txt", "xml"]),
    fmt("ZIP", Category::Archives, &["zip"], &["zip"]),
    fmt("RAR", Category::Archives, &["rar"], &["rar"]),
    fmt("7z", Category::Archives, &["7z"], &["7z"]),
    fmt("TAR / GZ", Category::Archives, &["tar", "gz", "bz2", "xz", "cab"], &["tar", "gz", "bz2", "xz", "cab"]),
];

/// What the person is looking for.
#[derive(Clone, Default, PartialEq)]
pub struct Wanted {
    /// Empty means every kind of file.
    pub kinds: Vec<Category>,
    /// Indexes into FORMATS. A kind with none of its formats picked means all of them.
    pub formats: Vec<usize>,
    pub min_size: u64,
}

impl Wanted {
    fn formats_of(&self, c: Category) -> impl Iterator<Item = &'static Format> + '_ {
        self.formats.iter().map(|&i| &FORMATS[i]).filter(move |f| f.cat == c)
    }

    pub fn accepts(&self, ext: &str, cat: Category, size: u64, is_dir: bool) -> bool {
        if is_dir {
            return true; // a folder may hold anything
        }
        if size < self.min_size {
            return false;
        }
        if self.kinds.is_empty() {
            return true;
        }
        if !self.kinds.contains(&cat) {
            return false;
        }
        let mut picked = self.formats_of(cat).peekable();
        picked.peek().is_none() || picked.any(|f| f.exts.contains(&ext))
    }

    /// PhotoRec families to switch on; empty means PhotoRec's own default set.
    pub fn families(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for &c in &self.kinds {
            let picked: Vec<&Format> = self.formats_of(c).collect();
            let fams: Vec<&'static str> =
                if picked.is_empty() { c.families().to_vec() } else { picked.iter().flat_map(|f| f.families.iter().copied()).collect() };
            for f in fams {
                if !out.contains(&f) {
                    out.push(f);
                }
            }
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chance {
    Excellent,
    Good,
    /// Part of the space was reused, or the drive already blanked it.
    Low,
}

#[derive(Clone, Debug)]
pub enum Origin {
    /// Still whole in the Recycle Bin.
    Recycled { data: PathBuf, is_dir: bool },
    /// Rebuilt from the raw bytes of the drive.
    Raw { device: String, offset: u64 },
    /// Where the file table says its pieces lie: (byte offset, or a hole, ; length).
    Runs { device: String, runs: Vec<(Option<u64>, u64)> },
    /// Small files kept inside the file table itself.
    Bytes(Arc<[u8]>),
    /// Rebuilt by PhotoRec into the scratch folder.
    Staged { path: PathBuf },
}

#[derive(Clone, Debug)]
pub struct Found {
    pub id: usize,
    pub name: String,
    pub ext: String,
    pub category: Category,
    pub size: u64,
    pub place: String,
    pub date: Option<String>,
    /// Seconds since 1970 behind `date`, 0 when unknown (used for sorting).
    pub stamp: u64,
    pub chance: Chance,
    pub origin: Origin,
    pub checked: bool,
}

#[derive(Default)]
pub struct ScanShared {
    pub new_files: Vec<Found>,
    /// Everything found so far, even when the window hasn't picked it up yet.
    pub found: usize,
    pub found_bytes: u64,
    pub progress: f32,
    /// Seconds left, once there is enough to go on.
    pub eta: Option<u64>,
    pub phase: String,
    pub done: bool,
    pub deep_skipped: bool,
    pub problem: Option<String>,
    /// Lines for the expert console, taken by the window as they come.
    pub log: Vec<String>,
    /// Where the files found by the quick scan start on the drive, so the deep search
    /// doesn't list them a second time.
    pub known_starts: std::collections::HashSet<u64>,
    t0: Option<std::time::Instant>,
}

impl ScanShared {
    pub fn add(&mut self, f: Found) {
        self.found += 1;
        self.found_bytes += f.size;
        if let Origin::Runs { runs, .. } = &f.origin
            && let Some(&(Some(at), _)) = runs.first()
        {
            self.known_starts.insert(at);
        }
        self.new_files.push(f);
    }

    /// Adds a line to the expert console, stamped with the time since the search began.
    pub fn note(&mut self, line: impl Into<String>) {
        let t = self.t0.get_or_insert_with(std::time::Instant::now).elapsed().as_secs();
        if self.log.len() >= 20_000 {
            self.log.drain(..5_000);
        }
        self.log.push(format!("[{:02}:{:02}:{:02}] {}", t / 3600, t / 60 % 60, t % 60, line.into()));
    }
}

/// Estimates the time left from how fast the progress has moved since `t0`.
pub struct Eta {
    t0: std::time::Instant,
    p0: f32,
}

impl Eta {
    pub fn new(p: f32) -> Eta {
        Eta { t0: std::time::Instant::now(), p0: p }
    }

    pub fn secs(&self, p: f32) -> Option<u64> {
        let el = self.t0.elapsed().as_secs_f32();
        let dp = p - self.p0;
        (el > 5.0 && dp > 0.002).then(|| (el * (1.0 - p) / dp) as u64)
    }
}

pub struct ScanJob {
    pub shared: Arc<Mutex<ScanShared>>,
    pub cancel: Arc<AtomicBool>,
    /// PhotoRec's scratch folder, to be emptied when the results are no longer needed.
    pub stage: Option<PathBuf>,
}

pub struct ScanPlan {
    pub disk: Disk,
    pub deep: bool,
    /// Read the whole drive too, not only the file table.
    pub full: bool,
    pub wanted: Wanted,
    pub photorec: Option<PathBuf>,
    pub stage: Option<PathBuf>,
}

impl ScanJob {
    pub fn start(plan: ScanPlan, ctx: eframe::egui::Context) -> ScanJob {
        let shared = Arc::new(Mutex::new(ScanShared::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let (s, c) = (shared.clone(), cancel.clone());
        let stage = plan.stage.clone().filter(|_| plan.deep && plan.photorec.is_some());
        let ScanPlan { disk, deep, full, wanted, photorec, .. } = plan;
        let stage_dir = stage.clone();
        std::thread::spawn(move || {
            let mut next_id = 0usize;
            set_phase(&s, t!("Recherche dans la corbeille…"), 0.0);
            s.lock().unwrap().note(t!("Disque {} ({}, {})", disk.title(), disk.fs, crate::app::human_size(disk.total)));
            let mut in_bin = 0;
            for f in scan_trash(&disk, &mut next_id) {
                let is_dir = matches!(f.origin, Origin::Recycled { is_dir: true, .. });
                if wanted.accepts(&f.ext, f.category, f.size, is_dir) {
                    s.lock().unwrap().add(f);
                    in_bin += 1;
                }
            }
            s.lock().unwrap().note(t!("Corbeille : {} élément(s) retenu(s)", in_bin));
            ctx.request_repaint();
            // The quick search reads the file table: names and folders come back, in minutes.
            let quick = deep && crate::ntfs::is_ntfs(&disk) && !c.load(Ordering::Relaxed);
            if quick && let Err(msg) = crate::ntfs::scan(&disk, &wanted, &s, &c, &mut next_id, &ctx) {
                s.lock().unwrap().note(t!("Analyse rapide impossible : {}", msg));
            }
            if deep && (full || !quick) && !c.load(Ordering::Relaxed) {
                let mut handled = false;
                if let (Some(exe), Some(stage)) = (&photorec, &stage_dir) {
                    set_phase(&s, t!("Préparation de l'analyse approfondie…"), 0.0);
                    let fs = disk.fs.to_ascii_uppercase();
                    let job = crate::photorec::Job {
                        exe,
                        device: disk.device.clone(),
                        // file systems whose free space PhotoRec can tell apart
                        free_only: fs == "NTFS" || fs.starts_with("FAT") || fs == "VFAT" || fs.starts_with("EXT"),
                        stage,
                        wanted: &wanted,
                    };
                    match crate::photorec::scan(&job, &s, &c, &mut next_id, &ctx) {
                        Ok(ran) => handled = ran,
                        Err(msg) => {
                            s.lock().unwrap().problem = Some(msg);
                            handled = true;
                        }
                    }
                }
                if !handled && !c.load(Ordering::Relaxed) {
                    s.lock().unwrap().note(t!("PhotoRec indisponible : recherche intégrée à la place"));
                    if let Err(msg) = deep_scan(&disk, &wanted, &s, &c, &mut next_id, &ctx) {
                        s.lock().unwrap().problem = Some(msg);
                    }
                }
            } else if !deep {
                s.lock().unwrap().deep_skipped = true;
            }
            let mut g = s.lock().unwrap();
            let (n, bytes) = (g.found, crate::app::human_size(g.found_bytes));
            let line = if c.load(Ordering::Relaxed) {
                t!("Recherche arrêtée : {} fichier(s), {}", n, bytes)
            } else {
                t!("Recherche terminée : {} fichier(s), {}", n, bytes)
            };
            g.note(line);
            g.done = true;
            g.progress = 1.0;
            g.eta = None;
            drop(g);
            ctx.request_repaint();
        });
        ScanJob { shared, cancel, stage }
    }
}

pub fn unix_secs(t: std::time::SystemTime) -> u64 {
    t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn date_of(t: std::time::SystemTime) -> Option<String> {
    secs_to_date(unix_secs(t))
}

fn set_phase(s: &Mutex<ScanShared>, phase: &str, progress: f32) {
    let mut g = s.lock().unwrap();
    g.phase = phase.to_string();
    g.progress = progress;
}

/// Seconds since 1970 to a date as the chosen language writes it ("dd/mm/yyyy" in French); nothing for 0 (unknown).
pub fn secs_to_date(secs: u64) -> Option<String> {
    if secs == 0 {
        return None;
    }
    let days = (secs / 86_400) as i64;
    // Days since 1970 to a calendar date (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    // day, month, year
    Some(t!("{0}/{1}/{2}", format!("{d:02}"), format!("{m:02}"), y))
}

fn dir_size(p: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    rd.flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Files that were deleted normally and are still in the bin of this drive.
pub fn scan_trash(disk: &Disk, next_id: &mut usize) -> Vec<Found> {
    let mut out = Vec::new();
    for item in sys::trash_items(disk) {
        let Ok(meta) = std::fs::metadata(&item.data) else { continue };
        let is_dir = meta.is_dir();
        let orig = &item.original;
        let name = orig.file_name().or_else(|| item.data.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let ext = orig.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let folder = orig.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        out.push(Found {
            id: *next_id,
            name,
            category: if is_dir { Category::Other } else { Category::from_ext(&ext) },
            // a marker, shown translated
            ext: if is_dir { "dossier".into() } else { ext },
            size: if is_dir { dir_size(&item.data) } else { item.size.max(meta.len()) },
            place: if folder.is_empty() { t!("Corbeille").into() } else { t!("Corbeille · {}", folder) },
            date: secs_to_date(item.deleted),
            stamp: item.deleted,
            chance: Chance::Excellent,
            origin: Origin::Recycled { data: item.data, is_dir },
            checked: false,
        });
        *next_id += 1;
    }
    out
}

/// The built-in search, used when PhotoRec is missing or has nowhere safe to write.
pub(crate) fn deep_scan(
    disk: &Disk,
    wanted: &Wanted,
    s: &Mutex<ScanShared>,
    cancel: &AtomicBool,
    next_id: &mut usize,
    ctx: &eframe::egui::Context,
) -> Result<(), String> {
    let file = std::fs::File::open(&disk.device)
        .map_err(|_| t!("Impossible de lire ce disque en profondeur. Relancez l'application en {}.", sys::admin_word()))?;
    let ntfs = disk.fs.eq_ignore_ascii_case("NTFS");
    // On NTFS only the free space is searched: that is where deleted files lie.
    let bitmap = if ntfs { sys::volume_bitmap(&file) } else { None };
    let cluster = disk.cluster;
    let in_use = |off: u64| -> bool {
        let Some(bm) = &bitmap else { return false };
        let c = (off / cluster) as usize;
        bm.get(c / 8).is_some_and(|b| b & (1 << (c % 8)) != 0)
    };
    let size = disk.total.max(1);
    let mut vol = Volume { inner: file, size };
    let phase = if bitmap.is_some() { t!("Analyse de l'espace libre du disque…") } else { t!("Analyse complète du disque…") };
    let step: usize = if bitmap.is_some() { cluster as usize } else { 512 };
    const CHUNK: usize = 8 << 20;
    let mut buf = vec![0u8; CHUNK];
    let mut pos = 0u64;
    let mut skip_until = 0u64;
    let mut counts = std::collections::HashMap::<&str, usize>::new();
    let mut last_report = std::time::Instant::now();
    let eta = Eta::new(0.0);
    while pos < size {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if last_report.elapsed().as_millis() > 150 {
            let p = pos as f32 / size as f32;
            set_phase(s, phase, p);
            s.lock().unwrap().eta = eta.secs(p);
            ctx.request_repaint();
            last_report = std::time::Instant::now();
        }
        let span = CHUNK.min((size - pos) as usize).max(512);
        // Skip chunks that are entirely in use without reading them.
        if let Some(bm) = &bitmap {
            let c0 = (pos / cluster) as usize;
            let c1 = ((pos + span as u64) / cluster) as usize;
            let all_used = (c0 / 8..c1.div_ceil(8)).all(|i| bm.get(i).is_none_or(|&b| b == 0xFF));
            if all_used && c0.is_multiple_of(8) {
                pos += span as u64;
                continue;
            }
        }
        let got = vol.read_at(pos, &mut buf[..span]);
        if got == 0 {
            if pos == 0 {
                return Err(t!("Ce disque ne se laisse pas lire. Vérifiez qu'il est bien branché (ou le disque bien inséré).").into());
            }
            // a scratched disc or a worn-out card: leave the bad patch, go on with the rest
            s.lock().unwrap().note(t!("zone illisible à l'octet {} ({}) : ignorée", pos, crate::app::human_size(span as u64)));
            pos += span as u64;
            continue;
        }
        let mut off = 0usize;
        while off + 16 <= got {
            let abs = pos + off as u64;
            if abs >= skip_until
                && !in_use(abs)
                && let Some(kind) = carve::detect(&buf[off..got.min(off + 512)])
                && let Some((len, ext)) = carve::measure(&mut vol, abs, kind)
            {
                skip_until = abs + len;
                let cat = Category::from_ext(ext);
                if !wanted.accepts(ext, cat, len, false) {
                    off += step;
                    continue;
                }
                let n = counts.entry(ext).or_default();
                *n += 1;
                let name = format!("{}_{:04}.{ext}", cat.stem(), *n);
                let mut g = s.lock().unwrap();
                g.note(t!("trouvé {} ({}) à l'octet {}", name, crate::app::human_size(len), abs));
                g.add(Found {
                    id: *next_id,
                    name,
                    ext: ext.to_string(),
                    category: cat,
                    size: len,
                    place: t!("Retrouvé dans l'espace libre · {}", crate::app::human_size(abs)),
                    date: None,
                    stamp: 0,
                    chance: Chance::Good,
                    origin: Origin::Raw { device: disk.device.clone(), offset: abs },
                    checked: false,
                });
                *next_id += 1;
            }
            off += step;
        }
        pos += got as u64;
    }
    Ok(())
}

// ---------------------------------------------------------------- recovery

#[derive(Default)]
pub struct CopyShared {
    pub done_files: usize,
    pub total_files: usize,
    pub failed: usize,
    pub finished: bool,
    pub folder: PathBuf,
}

pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let clean: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let p = dir.join(&clean);
    if !p.exists() {
        return p;
    }
    let (stem, ext) = match clean.rfind('.') {
        Some(i) if i > 0 => (&clean[..i], &clean[i..]),
        _ => (clean.as_str(), ""),
    };
    (2..).map(|i| dir.join(format!("{stem} ({i}){ext}"))).find(|p| !p.exists()).unwrap()
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

pub fn read_raw(device: &str, offset: u64, len: u64, mut sink: impl FnMut(&[u8]) -> bool) -> std::io::Result<()> {
    let file = std::fs::File::open(device)?;
    let mut vol = Volume { inner: file, size: u64::MAX / 2 };
    let mut done = 0u64;
    let mut buf = vec![0u8; 4 << 20];
    while done < len {
        let want = (len - done).min(buf.len() as u64) as usize;
        let got = vol.read_at(offset + done, &mut buf[..want]);
        if got == 0 {
            return Err(std::io::Error::other(t!("lecture interrompue")));
        }
        if !sink(&buf[..got]) {
            break;
        }
        done += got as u64;
    }
    Ok(())
}

/// The first `max` bytes of a file that only exists on the raw drive, handed to `sink` piece by piece.
pub fn read_found(f: &Found, max: u64, mut sink: impl FnMut(&[u8]) -> bool) -> std::io::Result<()> {
    let len = f.size.min(max);
    match &f.origin {
        Origin::Raw { device, offset } => read_raw(device, *offset, len, sink),
        Origin::Bytes(b) => {
            sink(&b[..(len as usize).min(b.len())]);
            Ok(())
        }
        Origin::Runs { device, runs } => {
            let mut left = len;
            let zeros = vec![0u8; 1 << 20];
            let mut go_on = true;
            for &(at, n) in runs {
                if left == 0 || !go_on {
                    break;
                }
                let n = n.min(left);
                match at {
                    Some(off) => read_raw(device, off, n, |b| {
                        go_on = sink(b);
                        go_on
                    })?,
                    // a hole in a sparse file reads as zeros
                    None => {
                        let mut z = n;
                        while z > 0 && go_on {
                            let k = z.min(zeros.len() as u64);
                            go_on = sink(&zeros[..k as usize]);
                            z -= k;
                        }
                    }
                }
                left -= n;
            }
            Ok(())
        }
        _ => Err(std::io::Error::other(t!("fichier déjà sur le disque"))),
    }
}

fn recover_one(f: &Found, dir: &Path) -> std::io::Result<()> {
    let sub = dir.join(f.category.label());
    std::fs::create_dir_all(&sub)?;
    let target = unique_path(&sub, &f.name);
    match &f.origin {
        Origin::Recycled { data, is_dir: true } => copy_dir(data, &target),
        Origin::Recycled { data, .. } => std::fs::copy(data, target).map(|_| ()),
        Origin::Staged { path } => std::fs::copy(path, target).map(|_| ()),
        Origin::Raw { .. } | Origin::Runs { .. } | Origin::Bytes(_) => {
            let mut out = std::io::BufWriter::new(std::fs::File::create(&target)?);
            let mut err = None;
            read_found(f, u64::MAX, |b| match out.write_all(b) {
                Ok(()) => true,
                Err(e) => {
                    err = Some(e);
                    false
                }
            })?;
            if let Some(e) = err {
                return Err(e);
            }
            out.flush()
        }
    }
}

pub fn start_recovery(files: Vec<Found>, dest: PathBuf, ctx: eframe::egui::Context) -> Arc<Mutex<CopyShared>> {
    let stamp = date_of(std::time::SystemTime::now()).unwrap_or_default().replace('/', "-");
    let folder = unique_path(&dest, &t!("Fichiers récupérés {}", stamp));
    let shared = Arc::new(Mutex::new(CopyShared { total_files: files.len(), folder: folder.clone(), ..Default::default() }));
    let s = shared.clone();
    std::thread::spawn(move || {
        for f in &files {
            let ok = std::fs::create_dir_all(&folder).and_then(|_| recover_one(f, &folder)).is_ok();
            let mut g = s.lock().unwrap();
            g.done_files += 1;
            if !ok {
                g.failed += 1;
            }
            drop(g);
            ctx.request_repaint();
        }
        s.lock().unwrap().finished = true;
        ctx.request_repaint();
    });
    shared
}
