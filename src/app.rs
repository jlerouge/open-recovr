//! The single screen: pick a drive, search, tick, recover.

use crate::art::{self, Art};
use crate::engine::{self, Category, Chance, CopyShared, FORMATS, Found, Origin, ScanJob, ScanPlan, ScanShared, Wanted};
use crate::i18n;
use crate::icons::{self, Icon};
use crate::preview::{self, Preview};
use crate::sys::{self, Disk};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Layout, Margin, Rect, Sense, Shadow, Stroke, StrokeKind,
    TextureHandle, Vec2, pos2, vec2,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub const BG: Color32 = Color32::from_rgb(244, 246, 251);
pub const CARD: Color32 = Color32::WHITE;
pub const ACCENT: Color32 = Color32::from_rgb(79, 70, 229);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(67, 56, 202);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(238, 240, 255);
pub const TEXT: Color32 = Color32::from_rgb(17, 24, 39);
pub const MUTED: Color32 = Color32::from_rgb(107, 114, 128);
pub const BORDER: Color32 = Color32::from_rgb(229, 231, 235);
pub const GREEN: Color32 = Color32::from_rgb(22, 163, 74);
pub const GREEN_SOFT: Color32 = Color32::from_rgb(220, 252, 231);
pub const AMBER: Color32 = Color32::from_rgb(180, 83, 9);

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}
pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn human_size(n: u64) -> String {
    // the units and the decimal mark of the chosen language
    let units: Vec<&str> = t!("o Ko Mo Go To").split(' ').collect();
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < units.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    let unit = units.get(u).copied().unwrap_or("");
    if u == 0 || v >= 100.0 || v.fract() == 0.0 { format!("{:.0} {}", v, unit) } else { format!("{:.1} {}", v, unit).replace('.', t!(",")) }
}

/// "1 fichier", "3 fichiers".
fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", i18n::plural(n as u64, one, many))
}

fn elide(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

fn time_left(eta: Option<u64>) -> String {
    match eta {
        None => t!("Calcul du temps restant…").into(),
        Some(s) if s < 60 => t!("Moins d'une minute restante").into(),
        Some(s) if s < 3600 => t!("Environ {} min restantes", s.div_ceil(60)),
        Some(s) => t!("Environ {} h {} restantes", s / 3600, format!("{:02}", (s % 3600) / 60)),
    }
}

/// One line on the search (or the copy) for the icon by the clock, and whether it is over.
fn background_status(scan: Option<&Arc<Mutex<ScanShared>>>, copy: Option<&Arc<Mutex<CopyShared>>>) -> (String, bool) {
    if let Some(c) = copy {
        let g = c.lock().unwrap();
        if !g.finished {
            return (t!("Récupération : {} / {} fichiers", g.done_files, g.total_files), false);
        }
    }
    if let Some(s) = scan {
        let g = s.lock().unwrap();
        let found = format!("{} ({})", plural(g.found, t!("fichier trouvé"), t!("fichiers trouvés")), human_size(g.found_bytes));
        if g.done {
            return (t!("Recherche terminée : {}", found), true);
        }
        let pct = (g.progress * 100.0).floor() as u32;
        return (t!("Recherche {} % · {} · {}", pct, time_left(g.eta).to_lowercase(), found), false);
    }
    (t!("Récupération terminée").into(), true)
}

/// The kinds of files one can aim the search at before starting it.
const TARGETS: [Category; 5] = [Category::Photos, Category::Videos, Category::Music, Category::Documents, Category::Archives];

fn target_hint(c: Category) -> &'static str {
    match c {
        Category::Photos => t!("JPEG, PNG, HEIC, photos d'appareil (RAW), Photoshop…"),
        Category::Videos => "MP4, MOV, AVI, MKV, WMV…",
        Category::Music => "MP3, WAV, FLAC, OGG, M4A…",
        Category::Documents => t!("Word, Excel, PowerPoint, PDF, OpenOffice, textes, notes, code source…"),
        Category::Archives => "ZIP, RAR, 7z, TAR…",
        Category::Other => "",
    }
}

/// "Windows" in messages, or a neutral word on other systems.
fn system() -> &'static str {
    if cfg!(windows) { "Windows" } else { t!("Le système") }
}

/// Minimum sizes offered before and after the search.
const SIZES: [u64; 4] = [0, 100 << 10, 1 << 20, 10 << 20];

/// "Plus de 1 Mo".
fn size_label(s: u64) -> String {
    if s == 0 { t!("Toutes tailles").into() } else { t!("Plus de {}", human_size(s)) }
}

/// "> 1 Mo", for the chips under the results.
fn size_short(s: u64) -> String {
    if s == 0 { t!("Toutes tailles").into() } else { format!("> {}", human_size(s)) }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Type,
    Name,
    Date,
    Size,
}

/// Everything the visible list depends on, to rebuild it only when one of these changes.
#[derive(Clone, PartialEq)]
struct ViewKey {
    files: usize,
    filter: Option<Category>,
    ext: Option<String>,
    min: u64,
    sort: Option<(SortKey, bool)>,
    query: String,
}

/// What the preview panel holds for the focused file.
enum Shown {
    Picture(TextureHandle),
    Text(String),
    Nothing,
}

/// The few choices remembered between runs.
#[derive(Default)]
struct Settings {
    never_ask_deep: bool,
    /// Shows the search journal and the raw bytes of files.
    expert: bool,
    /// The language chosen by hand ("en"…); none follows the system.
    language: Option<String>,
}

impl Settings {
    fn load() -> Settings {
        let text = sys::settings_file().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        let has = |line: &str| text.lines().any(|l| l.trim() == line);
        let language = text.lines().find_map(|l| l.trim().strip_prefix("langue=")).map(str::to_string);
        Settings { never_ask_deep: has("proposer_recherche_approfondie=non"), expert: has("mode_expert=oui"), language }
    }

    fn save(&self) {
        let Some(p) = sys::settings_file() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let yes = |b: bool| if b { "oui" } else { "non" };
        let mut text = format!("proposer_recherche_approfondie={}\nmode_expert={}\n", yes(!self.never_ask_deep), yes(self.expert));
        if let Some(l) = &self.language {
            text.push_str(&format!("langue={l}\n"));
        }
        let _ = std::fs::write(p, text);
    }
}

/// Sets the interface language: the one chosen by hand, else the system's.
pub fn choose_language(forced: Option<&str>) {
    let saved = Settings::load().language;
    let wanted = forced.or(saved.as_deref());
    if !wanted.is_some_and(i18n::set) {
        i18n::set(i18n::system_language());
    }
}

pub fn window_title() -> &'static str {
    t!("Open Recovr — récupération de fichiers")
}

/// A rounded, clickable tag.
fn chip(ui: &mut egui::Ui, icon: Option<Icon>, label: String, on: bool, color: Color32, enabled: bool) -> egui::Response {
    chip_sized(ui, icon, label, on, color, enabled, false)
}

fn chip_sized(
    ui: &mut egui::Ui,
    icon: Option<Icon>,
    label: String,
    on: bool,
    color: Color32,
    enabled: bool,
    small: bool,
) -> egui::Response {
    let fg = if on {
        Color32::WHITE
    } else if enabled {
        TEXT
    } else {
        MUTED
    };
    let (fs, pad) = if small { (13.0, 11.0) } else { (14.0, 16.0) };
    let galley = ui.painter().layout_no_wrap(label, bold(fs), fg);
    let iw = if icon.is_some() { 22.0 } else { 0.0 };
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (r, resp) = ui.allocate_exact_size(galley.size() + vec2(if small { 22.0 } else { 28.0 } + iw, pad), sense);
    let hovered = enabled && resp.hovered();
    let fill = if on {
        color
    } else if hovered {
        Color32::from_rgb(243, 244, 246)
    } else {
        CARD
    };
    let stroke = if on { Stroke::NONE } else { Stroke::new(1.0, BORDER) };
    ui.painter().rect(r, CornerRadius::same(255), fill, stroke, StrokeKind::Inside);
    if let Some(icon) = icon {
        icons::draw(ui.painter(), icon, pos2(r.left() + 22.0, r.center().y), 16.0, if on { fg } else { color });
    }
    let x = r.left() + if small { 11.0 } else { 14.0 } + iw;
    ui.painter().galley(pos2(x, r.center().y - galley.size().y / 2.0), galley, fg);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// A bold label at the height of the chips that follow it.
fn row_label(ui: &mut egui::Ui, text: &str, h: f32) {
    let galley = ui.painter().layout_no_wrap(text.into(), bold(14.0), TEXT);
    let (r, _) = ui.allocate_exact_size(vec2(galley.size().x + 2.0, h), Sense::hover());
    ui.painter().galley(pos2(r.left(), r.center().y - galley.size().y / 2.0), galley, TEXT);
}

/// Clickable accent text, centred in a row of height `h`.
fn text_link(ui: &mut egui::Ui, text: &str, h: f32, enabled: bool) -> egui::Response {
    let color = if enabled { ACCENT } else { MUTED };
    let galley = ui.painter().layout_no_wrap(text.into(), bold(14.0), color);
    let (r, resp) = ui.allocate_exact_size(vec2(galley.size().x, h), if enabled { Sense::click() } else { Sense::hover() });
    let y = r.center().y - galley.size().y / 2.0;
    if enabled && resp.hovered() {
        ui.painter().line_segment([pos2(r.left(), y + galley.size().y), pos2(r.right(), y + galley.size().y)], Stroke::new(1.0, color));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().galley(pos2(r.left(), y), galley, color);
    resp
}

/// A small triangle telling which way a column is sorted.
fn sort_arrow(p: &egui::Painter, at: egui::Pos2, up: bool, color: Color32) {
    let (a, b) = if up { (-3.0, 3.0) } else { (3.0, -3.0) };
    let pts = vec![pos2(at.x - 4.5, at.y + b), pos2(at.x + 4.5, at.y + b), pos2(at.x, at.y + a)];
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

fn cat_style(c: Category) -> (Icon, Color32, Color32) {
    match c {
        Category::Photos => (Icon::Photo, Color32::from_rgb(124, 58, 237), Color32::from_rgb(243, 232, 255)),
        Category::Videos => (Icon::Video, Color32::from_rgb(220, 38, 38), Color32::from_rgb(254, 226, 226)),
        Category::Music => (Icon::Music, Color32::from_rgb(234, 88, 12), Color32::from_rgb(255, 237, 213)),
        Category::Documents => (Icon::Document, Color32::from_rgb(37, 99, 235), Color32::from_rgb(219, 234, 254)),
        Category::Archives => (Icon::Archive, Color32::from_rgb(161, 98, 7), Color32::from_rgb(254, 249, 195)),
        Category::Other => (Icon::Folder, Color32::from_rgb(75, 85, 99), Color32::from_rgb(243, 244, 246)),
    }
}

/// An icon followed by a label, laid out like a normal widget.
fn icon_label(ui: &mut egui::Ui, icon: Icon, size: f32, color: Color32, text: egui::RichText) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
        icons::draw(ui.painter(), icon, r.center(), size, color);
        ui.label(text);
    });
}

fn card() -> Frame {
    Frame::new()
        .fill(CARD)
        .corner_radius(CornerRadius::same(16))
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(Margin::same(18))
        .shadow(Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(10) })
}

/// A large rounded button drawn by hand so it looks the same everywhere.
fn pill_button(ui: &mut egui::Ui, size: Vec2, text: &str, primary: bool, enabled: bool) -> egui::Response {
    icon_button(ui, size, None, text, primary, enabled)
}

fn icon_button(ui: &mut egui::Ui, size: Vec2, icon: Option<Icon>, text: &str, primary: bool, enabled: bool) -> egui::Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let fs = if size.y >= 50.0 { 18.0 } else { 15.0 };
    let icon_w = if icon.is_some() { fs + 10.0 } else { 0.0 };
    // a longer translation widens the button rather than spilling out of it
    let text_w = ui.painter().layout_no_wrap(text.to_string(), bold(fs), TEXT).size().x;
    let size = vec2(size.x.max(text_w + icon_w + 36.0), size.y);
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let hovered = enabled && resp.hovered();
    let (fill, fg, stroke) = match (primary, enabled) {
        (true, true) => (if hovered { ACCENT_DARK } else { ACCENT }, Color32::WHITE, Stroke::NONE),
        (true, false) => (Color32::from_rgb(199, 202, 238), Color32::WHITE, Stroke::NONE),
        (false, e) => (
            if hovered { ACCENT_SOFT } else { CARD },
            if e { ACCENT } else { MUTED },
            Stroke::new(1.0, if e { Color32::from_rgb(199, 202, 238) } else { BORDER }),
        ),
    };
    let r = CornerRadius::same((size.y / 2.0).min(14.0) as u8);
    ui.painter().rect(rect, r, fill, stroke, StrokeKind::Inside);
    let galley = ui.painter().layout_no_wrap(text.to_string(), bold(fs), fg);
    let x0 = rect.center().x - (galley.size().x + icon_w) / 2.0;
    if let Some(icon) = icon {
        icons::draw(ui.painter(), icon, pos2(x0 + fs / 2.0, rect.center().y), fs + 2.0, fg);
    }
    ui.painter().galley(pos2(x0 + icon_w, rect.center().y - galley.size().y / 2.0), galley, fg);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

fn step_title(ui: &mut egui::Ui, n: u32, title: &str, hint: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 14.0, ACCENT);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, n.to_string(), bold(15.0), Color32::WHITE);
        ui.add_space(4.0);
        ui.label(egui::RichText::new(title).font(bold(19.0)).color(TEXT));
        if !hint.is_empty() {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(hint).font(regular(14.0)).color(MUTED));
        }
    });
}

fn checkbox_at(painter: &egui::Painter, center: egui::Pos2, on: bool, partial: bool) {
    let r = Rect::from_center_size(center, vec2(20.0, 20.0));
    if on || partial {
        painter.rect_filled(r, CornerRadius::same(6), ACCENT);
        if on {
            let pts =
                vec![pos2(r.left() + 5.0, r.center().y), pos2(r.left() + 8.5, r.bottom() - 5.5), pos2(r.right() - 4.5, r.top() + 5.5)];
            painter.add(egui::Shape::line(pts, Stroke::new(2.2, Color32::WHITE)));
        } else {
            painter
                .line_segment([pos2(r.left() + 5.0, r.center().y), pos2(r.right() - 5.0, r.center().y)], Stroke::new(2.2, Color32::WHITE));
        }
    } else {
        painter.rect(r, CornerRadius::same(6), CARD, Stroke::new(1.5, Color32::from_rgb(209, 213, 219)), StrokeKind::Inside);
    }
}

/// Empties PhotoRec's scratch folder once its files are no longer listed.
fn clear_stage(job: &ScanJob) {
    if let Some(dir) = &job.stage {
        let _ = std::fs::remove_dir_all(dir);
    }
}

pub struct App {
    disks: Vec<Disk>,
    selected: usize,
    admin: bool,
    files: Vec<Found>,
    filter: Option<Category>,
    /// Narrowing the results by extension ("jpg") and minimum size.
    ext_filter: Option<String>,
    show_min: u64,
    sort: Option<(SortKey, bool)>,
    /// Indexes into `files` as listed, rebuilt when `view_key` changes.
    view: Vec<usize>,
    view_key: Option<ViewKey>,
    /// What the person is looking for, set before the search.
    wanted: Wanted,
    /// Read the whole drive after the quick look at the file table.
    full: bool,
    refine_open: bool,
    settings: Settings,
    ask_deep: bool,
    player: sys::Player,
    playing: Option<usize>,
    scan: Option<ScanJob>,
    scanned_once: bool,
    phase: String,
    progress: f32,
    eta: Option<u64>,
    deep_skipped: bool,
    problem: Option<String>,
    focus: Option<usize>,
    preview_slot: preview::Slot,
    preview: Option<(usize, Shown)>,
    preview_wanted: Option<usize>,
    copy: Option<Arc<Mutex<CopyShared>>>,
    /// The search journal shown in expert mode.
    log: Vec<String>,
    found: usize,
    found_bytes: u64,
    /// Narrows the results to names (or folders) holding these letters.
    query: String,
    /// The last search only read the file table: the complete one can follow.
    last_quick: bool,
    tray: Option<Arc<sys::Tray>>,
    told_background: bool,
    quitting: bool,
    confirm_dest: Option<(PathBuf, Vec<Found>)>,
    photorec: Option<PathBuf>,
    notice: Option<String>,
    screenshot: Option<PathBuf>,
    /// The moment to show in a documentation screenshot (`--scene`).
    scene: Option<String>,
    frames: u32,
    shot_requested: bool,
    shot_done: Arc<AtomicBool>,
    art: Art,
}

impl App {
    pub fn new(screenshot: Option<PathBuf>) -> Self {
        let disks = sys::list_disks();
        // Start on the drive holding the person's own folder, where most lost files live.
        let home = sys::home_dir().and_then(|h| sys::disk_of(&h, &disks).map(|d| d.id.clone()));
        let selected = disks.iter().position(|d| Some(&d.id) == home.as_ref()).unwrap_or(0);
        let admin = sys::is_admin();
        let settings = Settings::load();
        App {
            disks,
            selected,
            admin,
            files: Vec::new(),
            filter: None,
            ext_filter: None,
            show_min: 0,
            sort: None,
            view: Vec::new(),
            view_key: None,
            wanted: Wanted::default(),
            full: false,
            refine_open: false,
            ask_deep: !admin && !settings.never_ask_deep,
            settings,
            player: sys::Player::default(),
            playing: None,
            scan: None,
            scanned_once: false,
            phase: String::new(),
            progress: 0.0,
            eta: None,
            deep_skipped: false,
            problem: None,
            focus: None,
            preview_slot: Arc::new(Mutex::new(None)),
            preview: None,
            preview_wanted: None,
            copy: None,
            log: Vec::new(),
            found: 0,
            found_bytes: 0,
            query: String::new(),
            last_quick: false,
            tray: None,
            told_background: false,
            quitting: false,
            confirm_dest: None,
            photorec: sys::find_photorec(),
            notice: None,
            screenshot,
            scene: None,
            frames: 0,
            shot_requested: false,
            shot_done: Arc::new(AtomicBool::new(false)),
            art: Art::default(),
        }
    }

    fn scanning(&self) -> bool {
        self.scan.as_ref().is_some_and(|s| !s.shared.lock().unwrap().done)
    }

    fn copying(&self) -> bool {
        self.copy.as_ref().is_some_and(|c| !c.lock().unwrap().finished)
    }

    /// The selected drive can be searched through its file table (NTFS, with the rights to read it).
    fn quick_possible(&self) -> bool {
        self.admin && self.disks.get(self.selected).is_some_and(crate::ntfs::is_ntfs)
    }

    pub fn show_journal(&mut self) {
        self.settings.expert = true;
    }

    /// For documentation screenshots: puts the window in one of the moments of a recovery
    /// ("home", "scanning", "selected", "recovered"), on top of a real quick search.
    pub fn set_scene(&mut self, scene: &str) {
        self.scene = Some(scene.to_string());
        if scene != "depth" {
            self.ask_deep = false;
        }
    }

    fn apply_scene(&mut self) {
        let Some(scene) = self.scene.take() else { return };
        let photos = |files: &mut Vec<Found>| {
            for f in files.iter_mut().filter(|f| f.category == Category::Photos) {
                f.checked = true;
            }
        };
        match scene.as_str() {
            "scanning" => {
                if let Some(job) = &self.scan {
                    let mut g = job.shared.lock().unwrap();
                    g.done = false;
                    g.progress = 0.37;
                    g.eta = Some(23 * 60);
                    g.phase = t!("Analyse de l'espace libre du disque…").into();
                }
            }
            "selected" => {
                photos(&mut self.files);
                self.filter = Some(Category::Photos);
                self.focus = self.files.iter().position(|f| f.category == Category::Photos);
            }
            "recovered" => {
                photos(&mut self.files);
                let n = self.files.iter().filter(|f| f.checked).count();
                let name = t!("Fichiers récupérés {}", "2026-11-08");
                let folder = PathBuf::from(if cfg!(windows) { r"E:\" } else { "/media/usb" }).join(name);
                self.copy = Some(Arc::new(Mutex::new(CopyShared { done_files: n, total_files: n, failed: 0, finished: true, folder })));
            }
            _ => {}
        }
    }

    /// Gives the application its icon by the clock (Windows); window is its own window.
    pub fn attach_tray(&mut self, window: isize, ctx: &egui::Context) {
        if self.screenshot.is_some() {
            return;
        }
        let c = ctx.clone();
        self.tray = sys::Tray::start(window, &art::app_icon(32), move || c.request_repaint()).map(Arc::new);
    }

    /// Hides the window by the clock (or minimizes it) while the search goes on,
    /// and keeps the icon's line up to date until the search is over.
    fn go_background(&mut self, ctx: &egui::Context) {
        let scan = self.scan.as_ref().map(|j| j.shared.clone());
        let copy = self.copy.clone().filter(|c| !c.lock().unwrap().finished);
        let tray = self.tray.clone().filter(|t| {
            t.set_status(&background_status(scan.as_ref(), copy.as_ref()).0);
            t.hide_window()
        });
        let Some(tray) = tray else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            return;
        };
        if !self.told_background {
            self.told_background = true;
            tray.notify(
                t!("Open Recovr continue en arrière-plan"),
                t!("Cliquez sur l'icône pour revenir. Clic droit : arrêter et quitter."),
            );
        }
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if !tray.is_hidden() {
                    break;
                }
                let (text, over) = background_status(scan.as_ref(), copy.as_ref());
                tray.set_status(&text);
                if over {
                    tray.notify("Open Recovr", &t!("{}. Cliquez pour voir les fichiers.", text));
                    break;
                }
            }
        });
    }

    /// `D`, `D:` or a mount point.
    pub fn select_disk(&mut self, which: &str) {
        let w = which.trim_end_matches(':');
        if let Some(i) = self.disks.iter().position(|d| d.id.trim_end_matches(':').eq_ignore_ascii_case(w)) {
            self.selected = i;
        }
    }

    /// Lists `path` as if a search had found it (to try the preview without losing a file).
    pub fn show_file(&mut self, path: &std::path::Path) {
        let Ok(meta) = std::fs::metadata(path) else { return };
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        self.ask_deep = false;
        self.files.push(Found {
            id: self.files.len(),
            name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            category: Category::from_ext(&ext),
            ext,
            size: meta.len(),
            place: path.parent().map(|p| p.display().to_string()).unwrap_or_default(),
            date: None,
            stamp: 0,
            chance: Chance::Excellent,
            origin: Origin::Staged { path: path.to_path_buf() },
            checked: false,
        });
    }

    fn stop_sound(&mut self) {
        self.player.stop();
        self.playing = None;
    }

    pub fn start_scan(&mut self, ctx: &egui::Context) {
        let Some(disk) = self.disks.get(self.selected).cloned().filter(|d| d.searchable()) else { return };
        self.ask_deep = false;
        self.stop_sound();
        if let Some(old) = self.scan.take() {
            clear_stage(&old);
        }
        self.files.clear();
        self.focus = None;
        self.preview = None;
        self.filter = None;
        self.ext_filter = None;
        self.view_key = None;
        self.problem = None;
        self.deep_skipped = false;
        self.notice = None;
        self.copy = None;
        self.found = 0;
        self.found_bytes = 0;
        self.last_quick = self.quick_possible() && !self.full;
        self.scanned_once = true;
        self.phase = t!("Préparation…").into();
        self.progress = 0.0;
        self.eta = None;
        let plan = ScanPlan {
            stage: sys::pick_stage(&disk, &self.disks),
            disk,
            deep: self.admin,
            full: self.full,
            wanted: self.wanted.clone(),
            photorec: self.photorec.clone(),
        };
        self.scan = Some(ScanJob::start(plan, ctx.clone()));
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(job) = &self.scan {
            let mut g = job.shared.lock().unwrap();
            self.files.append(&mut g.new_files);
            self.phase = g.phase.clone();
            self.progress = g.progress;
            self.eta = g.eta;
            self.deep_skipped = g.deep_skipped;
            self.found = g.found;
            self.found_bytes = g.found_bytes;
            if g.problem.is_some() {
                self.problem = g.problem.clone();
            }
            if !g.log.is_empty() {
                self.log.append(&mut g.log);
                if self.log.len() > 20_000 {
                    self.log.drain(..self.log.len() - 15_000);
                }
            }
        }
        // "Arrêter et quitter" from the icon's menu
        if self.tray.as_ref().is_some_and(|t| t.take_quit()) {
            if let Some(job) = &self.scan {
                job.cancel.store(true, Ordering::Relaxed);
            }
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let got = self.preview_slot.lock().unwrap().take();
        if let Some((id, p)) = got {
            let shown = match p {
                Preview::Image(img) => Shown::Picture(ctx.load_texture(format!("preview-{id}"), img, egui::TextureOptions::LINEAR)),
                Preview::Text(t) => Shown::Text(t),
                Preview::Nothing => Shown::Nothing,
            };
            self.preview = Some((id, shown));
            self.preview_wanted = None;
        }
        if self.playing.is_some() {
            if self.player.playing() {
                ctx.request_repaint_after(std::time::Duration::from_millis(400));
            } else {
                self.playing = None;
            }
        }
        self.refresh_view();
    }

    /// Rebuilds the visible list when the results, the filters or the sort order changed.
    fn refresh_view(&mut self) {
        let key = ViewKey {
            files: self.files.len(),
            filter: self.filter,
            ext: self.ext_filter.clone(),
            min: self.show_min,
            sort: self.sort,
            query: self.query.trim().to_lowercase(),
        };
        if self.view_key.as_ref() == Some(&key) {
            return;
        }
        let files = &self.files;
        let mut v: Vec<usize> = (0..files.len())
            .filter(|&i| {
                let f = &files[i];
                self.filter.is_none_or(|c| f.category == c)
                    && self.ext_filter.as_ref().is_none_or(|e| &f.ext == e)
                    && f.size >= self.show_min
                    && (key.query.is_empty() || f.name.to_lowercase().contains(&key.query) || f.place.to_lowercase().contains(&key.query))
            })
            .collect();
        if let Some((k, asc)) = self.sort {
            v.sort_by(|&a, &b| {
                let (fa, fb) = (&files[a], &files[b]);
                let o = match k {
                    SortKey::Name => fa.name.to_lowercase().cmp(&fb.name.to_lowercase()),
                    SortKey::Type => (fa.category, &fa.ext).cmp(&(fb.category, &fb.ext)),
                    SortKey::Size => fa.size.cmp(&fb.size),
                    // unknown dates stay at the bottom either way
                    SortKey::Date => {
                        return (fa.stamp == 0).cmp(&(fb.stamp == 0)).then(if asc {
                            fa.stamp.cmp(&fb.stamp)
                        } else {
                            fb.stamp.cmp(&fa.stamp)
                        });
                    }
                };
                let o = if asc { o } else { o.reverse() };
                o.then(fa.id.cmp(&fb.id))
            });
        }
        self.view = v;
        self.view_key = Some(key);
    }

    fn set_sort(&mut self, k: SortKey) {
        self.sort = match self.sort {
            Some((cur, asc)) if cur == k => Some((k, !asc)),
            // biggest and newest first, names and types from A to Z
            _ => Some((k, matches!(k, SortKey::Name | SortKey::Type))),
        };
    }

    fn ask_destination(&mut self, files: Vec<Found>, ctx: &egui::Context) {
        if files.is_empty() {
            return;
        }
        let Some(dest) = rfd::FileDialog::new().set_title(t!("Où enregistrer les fichiers récupérés ?")).pick_folder() else {
            return;
        };
        if sys::disk_of(&dest, &self.disks).is_some_and(|d| !d.writable()) {
            self.notice = Some(t!("On ne peut pas enregistrer sur ce disque : choisissez un autre dossier.").into());
            return;
        }
        let same = self.disks.get(self.selected).is_some_and(|d| sys::disk_of(&dest, &self.disks).is_some_and(|x| x.id == d.id));
        if same {
            self.confirm_dest = Some((dest, files));
        } else {
            self.copy = Some(engine::start_recovery(files, dest, ctx.clone()));
        }
    }

    // ------------------------------------------------------------ sections

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(46.0, 46.0), Sense::hover());
            self.art.paint(ui, art::ICON, r);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.add_space(1.0);
                ui.label(egui::RichText::new("Open Recovr").font(bold(21.0)).color(TEXT));
                ui.label(egui::RichText::new(t!("Retrouvez vos fichiers effacés, simplement.")).font(regular(14.0)).color(MUTED));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.language_chip(ui);
                ui.add_space(10.0);
                if self.admin {
                    let txt = t!("Recherche approfondie activée");
                    let galley = ui.painter().layout_no_wrap(txt.into(), bold(13.0), GREEN);
                    let (r, _) = ui.allocate_exact_size(galley.size() + vec2(44.0, 14.0), Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(255), GREEN_SOFT);
                    icons::draw(ui.painter(), Icon::Shield, pos2(r.left() + 20.0, r.center().y), 16.0, GREEN);
                    ui.painter().galley(r.min + vec2(34.0, 7.0), galley, GREEN);
                } else {
                    let resp = icon_button(
                        ui,
                        vec2(330.0, 40.0),
                        Some(Icon::Shield),
                        t!("Passer en recherche approfondie"),
                        false,
                        !self.scanning(),
                    )
                    .on_hover_text(t!("Pour l'instant seule la corbeille est fouillée. La recherche approfondie lit tout le disque \
                         et retrouve aussi les fichiers effacés depuis longtemps."));
                    if resp.clicked() {
                        self.ask_deep = true;
                    }
                }
                ui.add_space(10.0);
                let expert = self.settings.expert;
                if chip_sized(ui, None, t!("Mode expert").into(), expert, TEXT, true, true)
                    .on_hover_text(t!("Affiche le journal de la recherche (avec les lignes de PhotoRec) et les premiers octets \
                         des fichiers qui n'ont pas d'aperçu."))
                    .clicked()
                {
                    self.settings.expert = !expert;
                    self.settings.save();
                    self.preview = None;
                    self.preview_wanted = None;
                }
                if self.photorec.is_some() {
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(t!("Analyse par PhotoRec, logiciel libre")).font(regular(12.5)).color(MUTED))
                        .on_hover_text(t!(
                            "La recherche approfondie utilise PhotoRec 7.2 © Christophe Grenier, distribué sous licence GNU GPL \
                             (www.cgsecurity.org). Il reconnaît des centaines de types de fichiers."
                        ));
                }
            });
        });
    }

    /// The language, with a menu of the others.
    fn language_chip(&mut self, ui: &mut egui::Ui) {
        let current = &i18n::LANGS[i18n::current()];
        let resp =
            chip_sized(ui, Some(Icon::Globe), current.name.into(), false, ACCENT, true, true).on_hover_text(t!("Langue de l'interface"));
        let mut chosen: Option<&'static str> = None;
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(150.0);
            for l in i18n::LANGS {
                let on = l.code == current.code;
                if ui.selectable_label(on, egui::RichText::new(l.name).font(regular(15.0))).clicked() {
                    chosen = Some(l.code);
                }
            }
        });
        if let Some(code) = chosen
            && code != current.code
            && i18n::set(code)
        {
            self.settings.language = Some(code.to_string());
            self.settings.save();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(window_title().into()));
            // the dates and the words shown in the list come from the search: they stay as they were
            self.view_key = None;
        }
    }

    fn disk_section(&mut self, ui: &mut egui::Ui) {
        let busy = self.scanning();
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let left_w = (ui.available_width() - 340.0).max(300.0);
                ui.allocate_ui_with_layout(vec2(left_w, 0.0), Layout::top_down(Align::Min), |ui| {
                    step_title(ui, 1, t!("Où étaient vos fichiers ?"), t!("Choisissez le disque, la carte mémoire ou la clé USB."));
                    ui.add_space(12.0);
                    if self.disks.is_empty() {
                        ui.label(egui::RichText::new(t!("Aucun disque trouvé.")).color(MUTED));
                    }
                    egui::ScrollArea::horizontal().id_salt("disks").show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for i in 0..self.disks.len() {
                                if self.disk_card(ui, i, busy).clicked() && !busy {
                                    self.selected = i;
                                }
                                ui.add_space(4.0);
                            }
                        });
                    });
                    ui.add_space(10.0);
                    self.target_chips(ui, busy);
                    if self.quick_possible() {
                        self.mode_row(ui, busy);
                    }
                });
                ui.add_space(12.0);
                let phone = self.disks.get(self.selected).is_some_and(|d| !d.searchable());
                ui.allocate_ui_with_layout(vec2(320.0, 180.0), Layout::top_down(Align::Center), |ui| {
                    ui.add_space(if busy {
                        24.0
                    } else if phone {
                        14.0
                    } else {
                        52.0
                    });
                    if phone && !busy {
                        self.phone_help(ui);
                    } else if busy {
                        if icon_button(ui, vec2(310.0, 56.0), Some(Icon::Stop), t!("Arrêter la recherche"), false, true).clicked()
                            && let Some(job) = &self.scan
                        {
                            job.cancel.store(true, Ordering::Relaxed);
                        }
                        ui.add_space(12.0);
                        self.progress_bar(ui, 310.0, self.progress, true);
                        ui.add_space(6.0);
                        let pct = (self.progress * 100.0).floor() as u32;
                        ui.label(egui::RichText::new(format!("{}  {pct} %", self.phase)).font(regular(13.5)).color(MUTED));
                        ui.label(egui::RichText::new(time_left(self.eta)).font(bold(13.5)).color(TEXT));
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · {}",
                                plural(self.found, t!("fichier trouvé"), t!("fichiers trouvés")),
                                human_size(self.found_bytes)
                            ))
                            .font(regular(13.5))
                            .color(MUTED),
                        );
                        ui.add_space(2.0);
                        let hint = if sys::Tray::AVAILABLE {
                            t!("Range la fenêtre près de l'horloge : la recherche continue et l'icône indique où elle en est. \
                                Fermer la fenêtre pendant la recherche fait la même chose.")
                        } else {
                            t!("Réduit la fenêtre : la recherche continue.")
                        };
                        if ui
                            .link(egui::RichText::new(t!("Continuer en arrière-plan")).font(bold(13.5)).color(ACCENT))
                            .on_hover_text(hint)
                            .clicked()
                        {
                            self.go_background(ui.ctx());
                        }
                    } else {
                        let label = if self.scanned_once { t!("Relancer la recherche") } else { t!("Rechercher les fichiers perdus") };
                        let can = !self.disks.is_empty() && !self.copying();
                        if icon_button(ui, vec2(310.0, 56.0), Some(Icon::Search), label, true, can).clicked() {
                            self.start_scan(ui.ctx());
                        }
                        ui.add_space(10.0);
                        if self.admin {
                            let what = if self.quick_possible() && !self.full {
                                t!("Analyse rapide : quelques minutes, noms d'origine.")
                            } else {
                                t!("Recherche approfondie : tout le disque est lu.")
                            };
                            ui.label(egui::RichText::new(what).font(regular(13.5)).color(MUTED));
                            ui.label(egui::RichText::new(t!("Rien n'est modifié sur le disque.")).font(regular(13.5)).color(MUTED));
                            if self.last_quick
                                && self.quick_possible()
                                && ui
                                    .link(egui::RichText::new(t!("Pas trouvé ? Lancer l'analyse complète")).font(bold(13.5)).color(ACCENT))
                                    .on_hover_text(t!("Lit tout le disque avec PhotoRec pour retrouver aussi les fichiers plus anciens."))
                                    .clicked()
                            {
                                self.full = true;
                                self.start_scan(ui.ctx());
                            }
                        } else {
                            ui.label(
                                egui::RichText::new(t!("Recherche rapide : seule la corbeille est fouillée."))
                                    .font(regular(13.5))
                                    .color(MUTED),
                            );
                            if ui
                                .link(egui::RichText::new(t!("Chercher aussi sur tout le disque…")).font(bold(13.5)).color(ACCENT))
                                .clicked()
                            {
                                self.ask_deep = true;
                            }
                        }
                    }
                });
            });
        });
    }

    /// A phone plugged by USB only lends its files one by one (MTP): there is no free space
    /// to read, and its memory is encrypted anyway. Says where to look instead.
    fn phone_help(&self, ui: &mut egui::Ui) {
        Frame::new().fill(Color32::from_rgb(255, 251, 235)).corner_radius(CornerRadius::same(12)).inner_margin(Margin::same(12)).show(
            ui,
            |ui| {
                ui.set_width(296.0);
                icon_label(
                    ui,
                    Icon::Warning,
                    18.0,
                    AMBER,
                    egui::RichText::new(t!("Un téléphone ne se fouille pas ainsi")).font(bold(14.5)).color(AMBER),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(t!("Branché en USB, il ne montre que ses fichiers présents, et sa mémoire est chiffrée : \
                         les fichiers effacés ne peuvent pas être lus depuis l'ordinateur."))
                    .font(regular(13.0))
                    .color(TEXT),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(t!(
                        "Regardez plutôt dans la corbeille de l'appli Galerie ou Photos (30 jours), dans Google Photos \
                         ou iCloud. Si le téléphone a une carte microSD, mettez-la dans un lecteur de cartes."
                    ))
                    .font(regular(13.0))
                    .color(MUTED),
                );
            },
        );
    }

    /// "Je cherche : Tout · Photos · …" — narrowing it makes the search sort better and finish sooner.
    fn target_chips(&mut self, ui: &mut egui::Ui, busy: bool) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            row_label(ui, t!("Je cherche :"), 34.0);
            let all = chip(ui, None, t!("Tout").into(), self.wanted.kinds.is_empty(), TEXT, !busy)
                .on_hover_text(t!("Tous les types de fichiers que l'analyse sait reconnaître."));
            if all.clicked() {
                self.wanted.kinds.clear();
                self.wanted.formats.clear();
            }
            for c in TARGETS {
                let (icon, color, _) = cat_style(c);
                let on = self.wanted.kinds.contains(&c);
                if chip(ui, Some(icon), c.label().into(), on, color, !busy).on_hover_text(target_hint(c)).clicked() {
                    if on {
                        self.wanted.kinds.retain(|t| *t != c);
                        self.wanted.formats.retain(|&i| FORMATS[i].cat != c);
                    } else {
                        self.wanted.kinds.push(c);
                    }
                }
            }
            // "Préciser (JPG · plus de 1 Mo) ▸"
            let mut parts: Vec<String> = self.wanted.formats.iter().map(|&i| FORMATS[i].name().to_string()).collect();
            if self.wanted.min_size > 0 {
                parts.push(size_label(self.wanted.min_size).to_lowercase());
            }
            let text =
                if parts.is_empty() { t!("Préciser").to_string() } else { format!("{}  ({})", t!("Préciser"), parts.join(" · ")) };
            ui.add_space(4.0);
            let link = text_link(ui, &text, 34.0, !busy);
            // the triangle is drawn: the bold font has no arrow characters
            ui.spacing_mut().item_spacing.x = 4.0;
            let (ar, arrow) = ui.allocate_exact_size(vec2(10.0, 34.0), if busy { Sense::hover() } else { Sense::click() });
            let (c, color) = (ar.center(), if busy { MUTED } else { ACCENT });
            let pts = if self.refine_open {
                vec![pos2(c.x - 4.5, c.y - 2.5), pos2(c.x + 4.5, c.y - 2.5), pos2(c.x, c.y + 3.0)]
            } else {
                vec![pos2(c.x - 2.5, c.y - 4.5), pos2(c.x + 3.0, c.y), pos2(c.x - 2.5, c.y + 4.5)]
            };
            ui.painter().add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
            if (link | arrow)
                .on_hover_text(t!(
                    "Choisir un format précis (JPG, Word…) ou une taille minimale, pour écarter les petits fichiers sans intérêt."
                ))
                .clicked()
            {
                self.refine_open = !self.refine_open;
            }
        });
        if self.refine_open {
            ui.add_space(4.0);
            self.refine_rows(ui, busy);
        }
    }

    /// "Analyse : Rapide · Complète", for drives whose file table can be read.
    fn mode_row(&mut self, ui: &mut egui::Ui, busy: bool) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            row_label(ui, t!("Analyse :"), 28.0);
            if chip_sized(ui, None, t!("Rapide").into(), !self.full, ACCENT, !busy, true)
                .on_hover_text(t!(
                    "Lit la table des fichiers du disque : quelques minutes, et les fichiers gardent leur nom et leur dossier. \
                     Retrouve ce qui a été effacé récemment, même après avoir vidé la corbeille."
                ))
                .clicked()
            {
                self.full = false;
            }
            if chip_sized(ui, None, t!("Complète").into(), self.full, ACCENT, !busy, true)
                .on_hover_text(t!(
                    "Fait l'analyse rapide puis lit tout le disque avec PhotoRec : retrouve aussi les fichiers plus anciens, \
                     souvent sans leur nom. Peut prendre plus d'une heure."
                ))
                .clicked()
            {
                self.full = true;
            }
        });
    }

    /// The formats of the chosen kinds, and a minimum size.
    fn refine_rows(&mut self, ui: &mut egui::Ui, busy: bool) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            row_label(ui, t!("Formats :"), 28.0);
            if self.wanted.kinds.is_empty() {
                let galley = ui.painter().layout_no_wrap(
                    t!("Choisissez d'abord Photos, Documents… pour préciser le format.").into(),
                    regular(13.5),
                    MUTED,
                );
                let (r, _) = ui.allocate_exact_size(vec2(galley.size().x, 28.0), Sense::hover());
                ui.painter().galley(pos2(r.left(), r.center().y - galley.size().y / 2.0), galley, MUTED);
            }
            for (i, f) in FORMATS.iter().enumerate() {
                if !self.wanted.kinds.contains(&f.cat) {
                    continue;
                }
                let (_, color, _) = cat_style(f.cat);
                let on = self.wanted.formats.contains(&i);
                let hint = f.exts.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(" ");
                if chip_sized(ui, None, f.name().into(), on, color, !busy, true).on_hover_text(hint).clicked() {
                    if on {
                        self.wanted.formats.retain(|&j| j != i);
                    } else {
                        self.wanted.formats.push(i);
                    }
                }
            }
        });
        if self.wanted.kinds.iter().any(|&c| !self.wanted.formats.iter().any(|&i| FORMATS[i].cat == c)) {
            ui.label(
                egui::RichText::new(t!("Aucun format coché dans une catégorie : tous ses formats sont cherchés."))
                    .font(regular(12.5))
                    .color(MUTED),
            );
        }
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            row_label(ui, t!("Taille :"), 28.0);
            for s in SIZES {
                let label = if s == 0 { t!("Toutes").to_string() } else { size_label(s) };
                if chip_sized(ui, None, label, self.wanted.min_size == s, ACCENT, !busy, true).clicked() {
                    self.wanted.min_size = s;
                }
            }
        });
    }

    fn disk_card(&self, ui: &mut egui::Ui, i: usize, busy: bool) -> egui::Response {
        let d = &self.disks[i];
        let sel = i == self.selected;
        let (rect, resp) = ui.allocate_exact_size(vec2(238.0, 96.0), if busy { Sense::hover() } else { Sense::click() });
        let hovered = resp.hovered() && !busy;
        let p = ui.painter();
        let fill = if sel {
            ACCENT_SOFT
        } else if hovered {
            Color32::from_rgb(249, 250, 251)
        } else {
            CARD
        };
        let stroke = if sel { Stroke::new(2.0, ACCENT) } else { Stroke::new(1.0, BORDER) };
        p.rect(rect, CornerRadius::same(14), fill, stroke, StrokeKind::Inside);
        // the picture of the kind of drive: hard disk, SSD, card, key, phone…
        let pic = Rect::from_min_size(rect.min + vec2(10.0, 10.0), vec2(48.0, 48.0));
        self.art.paint(ui, d.kind.art(), pic);
        let x = pic.right() + 10.0;
        p.text(pos2(x, rect.top() + 24.0), Align2::LEFT_CENTER, elide(&d.title(), 20), bold(15.0), TEXT);
        let second =
            if d.total > 0 { format!("{} · {}", d.kind.label(), t!("{} libres", human_size(d.free))) } else { d.kind.label().to_string() };
        p.text(pos2(x, rect.top() + 45.0), Align2::LEFT_CENTER, elide(&second, 26), regular(12.5), MUTED);
        if d.total > 0 {
            let bar = Rect::from_min_max(pos2(rect.left() + 14.0, rect.bottom() - 22.0), pos2(rect.right() - 14.0, rect.bottom() - 14.0));
            p.rect_filled(bar, CornerRadius::same(4), Color32::from_rgb(229, 231, 235));
            let used = 1.0 - d.free as f32 / d.total as f32;
            let mut fillr = bar;
            fillr.set_width(bar.width() * used.clamp(0.02, 1.0));
            p.rect_filled(fillr, CornerRadius::same(4), if sel { ACCENT } else { Color32::from_rgb(156, 163, 175) });
        } else {
            p.text(
                pos2(rect.left() + 14.0, rect.bottom() - 18.0),
                Align2::LEFT_CENTER,
                t!("Pas de recherche possible"),
                regular(12.0),
                AMBER,
            );
        }
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let mut tip = Vec::new();
        if !d.model.is_empty() {
            tip.push(d.model.clone());
        }
        if !d.fs.is_empty() {
            tip.push(t!("Système de fichiers : {}", d.fs));
        }
        if d.total > 0 {
            tip.push(t!("{} libres sur {}", human_size(d.free), human_size(d.total)));
        }
        if tip.is_empty() { resp } else { resp.on_hover_text(tip.join("\n")) }
    }

    fn progress_bar(&self, ui: &mut egui::Ui, w: f32, v: f32, animate: bool) {
        let (r, _) = ui.allocate_exact_size(vec2(w, 10.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::same(5), Color32::from_rgb(229, 231, 235));
        let mut f = r;
        f.set_width((r.width() * v.clamp(0.0, 1.0)).max(10.0));
        p.rect_filled(f, CornerRadius::same(5), ACCENT);
        if animate {
            let t = ui.input(|i| i.time) as f32;
            let x = f.left() + (t * 0.6).fract() * f.width();
            let glow = Rect::from_center_size(pos2(x, f.center().y), vec2(30.0, 10.0)).intersect(f);
            p.rect_filled(glow, CornerRadius::same(5), Color32::from_white_alpha(70));
            ui.ctx().request_repaint();
        }
    }

    fn results_section(&mut self, ui: &mut egui::Ui) {
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_height(ui.available_height());
            step_title(ui, 2, t!("Cochez ce que vous voulez récupérer"), "");
            ui.add_space(12.0);
            if self.files.is_empty() {
                self.empty_state(ui);
                return;
            }
            self.chips(ui);
            ui.add_space(12.0);
            // the list needs little room: the preview gets a good share of a wide window
            let preview_w = (ui.available_width() * 0.5).clamp(300.0, 760.0);
            ui.horizontal_top(|ui| {
                let list_w = ui.available_width() - preview_w - 16.0 - 2.0 * ui.spacing().item_spacing.x;
                ui.allocate_ui_with_layout(vec2(list_w, ui.available_height()), Layout::top_down(Align::Min), |ui| {
                    self.file_list(ui);
                });
                ui.add_space(16.0);
                ui.allocate_ui_with_layout(vec2(preview_w, ui.available_height()), Layout::top_down(Align::Min), |ui| {
                    self.preview_panel(ui);
                });
            });
        });
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        let h = ui.available_height();
        ui.vertical_centered(|ui| {
            ui.add_space((h / 2.0 - 110.0).max(4.0));
            let (r, _) = ui.allocate_exact_size(vec2(120.0, 120.0), Sense::hover());
            let p = ui.painter();
            p.circle_filled(r.center(), 60.0, ACCENT_SOFT);
            p.circle_filled(r.center(), 42.0, Color32::from_rgb(224, 231, 255));
            if self.scanning() {
                let t = ui.input(|i| i.time) as f32;
                let a = t * 2.4;
                let c = r.center() + vec2(a.cos(), a.sin()) * 12.0;
                p.circle_stroke(c, 16.0, Stroke::new(4.0, ACCENT));
                p.line_segment([c + vec2(11.0, 11.0), c + vec2(22.0, 22.0)], Stroke::new(5.0, ACCENT));
                ui.ctx().request_repaint();
            } else {
                let c = r.center() + vec2(-6.0, -6.0);
                p.circle_stroke(c, 18.0, Stroke::new(4.5, ACCENT));
                p.line_segment([c + vec2(13.0, 13.0), c + vec2(26.0, 26.0)], Stroke::new(5.5, ACCENT));
            }
            ui.add_space(18.0);
            let (title, sub) = if self.scanning() {
                (t!("Recherche en cours…"), t!("Les fichiers apparaîtront ici dès qu'ils sont trouvés."))
            } else if self.scanned_once {
                (
                    t!("Aucun fichier retrouvé sur ce disque"),
                    if self.admin {
                        t!("Essayez un autre disque, ou une clé USB si les fichiers y étaient.")
                    } else {
                        t!("La recherche rapide n'a regardé que la corbeille. Passez en recherche approfondie pour lire tout le disque.")
                    },
                )
            } else {
                (
                    t!("Vos fichiers ne sont peut-être pas perdus"),
                    t!("Choisissez un disque, cliquez sur « Rechercher », puis cochez ce que vous voulez retrouver."),
                )
            };
            ui.label(egui::RichText::new(title).font(bold(22.0)).color(TEXT));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(sub).font(regular(15.0)).color(MUTED));
            if let Some(p) = &self.problem {
                ui.add_space(10.0);
                ui.label(egui::RichText::new(p).font(regular(14.0)).color(AMBER));
            }
        });
    }

    fn chips(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if chip(ui, None, format!("{}  {}", t!("Tout"), self.files.len()), self.filter.is_none(), TEXT, true).clicked() {
                self.filter = None;
            }
            for c in Category::ALL {
                let n = self.files.iter().filter(|f| f.category == c).count();
                if n == 0 {
                    continue;
                }
                let (icon, color, _) = cat_style(c);
                if chip(ui, Some(icon), format!("{}  {n}", c.label()), self.filter == Some(c), color, true).clicked()
                    && self.filter != Some(c)
                {
                    self.filter = Some(c);
                    self.ext_filter = None;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(t!("Rechercher un nom…"))
                        .desired_width(230.0)
                        .margin(Margin::symmetric(10, 7)),
                )
                .on_hover_text(t!("Ne montre que les fichiers dont le nom ou le dossier contient ces lettres."));
            });
        });
        ui.add_space(8.0);
        // second row: the most common extensions of what is shown, and a minimum size
        let mut counts: Vec<(String, usize)> = Vec::new();
        for f in self.files.iter().filter(|f| self.filter.is_none_or(|c| f.category == c)) {
            match counts.iter_mut().find(|(e, _)| *e == f.ext) {
                Some((_, n)) => *n += 1,
                None => counts.push((f.ext.clone(), 1)),
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        counts.truncate(7);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if counts.len() > 1 || self.ext_filter.is_some() {
                row_label(ui, t!("Format :"), 28.0);
                if chip_sized(ui, None, t!("Tous").into(), self.ext_filter.is_none(), TEXT, true, true).clicked() {
                    self.ext_filter = None;
                }
                for (e, n) in &counts {
                    let on = self.ext_filter.as_ref() == Some(e);
                    let label = if e.is_empty() {
                        format!("{}  {n}", t!("sans extension"))
                    } else if e == "dossier" {
                        format!("{}  {n}", t!("dossier"))
                    } else {
                        format!("{}  {n}", e.to_uppercase())
                    };
                    if chip_sized(ui, None, label, on, ACCENT, true, true).clicked() {
                        self.ext_filter = if on { None } else { Some(e.clone()) };
                    }
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for s in SIZES.iter().rev() {
                    if chip_sized(ui, None, size_short(*s), self.show_min == *s, ACCENT, true, true).clicked() {
                        self.show_min = *s;
                    }
                }
                row_label(ui, t!("Taille :"), 28.0);
            });
        });
    }

    fn file_list(&mut self, ui: &mut egui::Ui) {
        let visible = std::mem::take(&mut self.view);
        let all_on = !visible.is_empty() && visible.iter().all(|&i| self.files[i].checked);
        let some_on = visible.iter().any(|&i| self.files[i].checked);
        // header: the box ticks everything shown, the titles sort the list
        let w = ui.available_width();
        let (hr, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
        let boxr = Rect::from_center_size(pos2(hr.left() + 22.0, hr.center().y), vec2(36.0, 30.0));
        let box_resp = ui.interact(boxr, ui.scope_id().with("all-box"), Sense::click()).on_hover_text(if all_on {
            t!("Tout décocher")
        } else {
            t!("Cocher tout ce qui est affiché")
        });
        if box_resp.clicked() {
            for &i in &visible {
                self.files[i].checked = !all_on;
            }
        }
        let p = ui.painter().clone();
        checkbox_at(&p, boxr.center(), all_on, some_on && !all_on);
        let columns = [
            (SortKey::Type, t!("Type"), hr.left() + 68.0, Align2::CENTER_CENTER),
            (SortKey::Name, t!("Nom"), hr.left() + 100.0, Align2::LEFT_CENTER),
            (SortKey::Date, t!("Date"), hr.right() - 300.0, Align2::LEFT_CENTER),
            (SortKey::Size, t!("Taille"), hr.right() - 120.0, Align2::RIGHT_CENTER),
        ];
        for (k, title, x, align) in columns {
            let active = self.sort.filter(|(s, _)| *s == k);
            let color = if active.is_some() { ACCENT } else { MUTED };
            let tr = p.text(pos2(x, hr.center().y), align, title, bold(13.0), color);
            let zone = tr.expand2(vec2(6.0, 6.0)).with_max_x(tr.right() + 18.0);
            let resp = ui.interact(zone, ui.scope_id().with(("sort", title)), Sense::click());
            if let Some((_, asc)) = active {
                sort_arrow(&p, pos2(tr.right() + 9.0, hr.center().y), asc, ACCENT);
            } else if resp.hovered() {
                sort_arrow(&p, pos2(tr.right() + 9.0, hr.center().y), false, Color32::from_rgb(209, 213, 219));
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.on_hover_text(t!("Cliquez pour trier")).clicked() {
                self.set_sort(k);
            }
        }
        p.text(pos2(hr.right() - 12.0, hr.center().y), Align2::RIGHT_CENTER, t!("Chances"), bold(13.0), MUTED);
        p.line_segment([pos2(hr.left(), hr.bottom()), pos2(hr.right(), hr.bottom())], Stroke::new(1.0, BORDER));

        if visible.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(t!("Aucun fichier ne correspond à ces filtres.")).font(regular(14.5)).color(MUTED));
            });
        }
        let row_h = 54.0;
        let mut clicked: Option<usize> = None;
        let mut toggled: Option<usize> = None;
        egui::ScrollArea::vertical().id_salt("files").auto_shrink([false, false]).show_rows(ui, row_h, visible.len(), |ui, range| {
            for vi in range {
                let idx = visible[vi];
                let f = &self.files[idx];
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::click());
                let p = ui.painter();
                let focused = self.focus == Some(idx);
                if focused {
                    p.rect_filled(r.shrink2(vec2(0.0, 2.0)), CornerRadius::same(10), ACCENT_SOFT);
                } else if resp.hovered() {
                    p.rect_filled(r.shrink2(vec2(0.0, 2.0)), CornerRadius::same(10), Color32::from_rgb(249, 250, 251));
                }
                checkbox_at(p, pos2(r.left() + 22.0, r.center().y), f.checked, false);
                let (icon, color, soft) = cat_style(f.category);
                let ir = Rect::from_center_size(pos2(r.left() + 68.0, r.center().y), vec2(36.0, 36.0));
                p.rect_filled(ir, CornerRadius::same(9), soft);
                icons::draw(p, icon, ir.center(), 20.0, color);
                let name_w = ((r.width() - 420.0) / 8.0).max(12.0) as usize;
                p.text(pos2(r.left() + 100.0, r.top() + 18.0), Align2::LEFT_CENTER, elide(&f.name, name_w), bold(15.0), TEXT);
                let sub = if f.ext.is_empty() || f.ext == "dossier" {
                    f.place.clone()
                } else {
                    format!("{} · {}", f.ext.to_uppercase(), f.place)
                };
                p.text(pos2(r.left() + 100.0, r.top() + 37.0), Align2::LEFT_CENTER, elide(&sub, name_w + 10), regular(12.5), MUTED);
                p.text(
                    pos2(r.right() - 300.0, r.center().y),
                    Align2::LEFT_CENTER,
                    f.date.clone().unwrap_or_else(|| "—".into()),
                    regular(14.0),
                    TEXT,
                );
                p.text(pos2(r.right() - 120.0, r.center().y), Align2::RIGHT_CENTER, human_size(f.size), regular(14.0), TEXT);
                let (txt, fg, bg) = match f.chance {
                    Chance::Excellent => (t!("Excellentes"), GREEN, GREEN_SOFT),
                    Chance::Good => (t!("Bonnes"), ACCENT, ACCENT_SOFT),
                    Chance::Low => (t!("Faibles"), AMBER, Color32::from_rgb(254, 243, 199)),
                };
                let br = Rect::from_min_max(pos2(r.right() - 96.0, r.center().y - 12.0), pos2(r.right() - 6.0, r.center().y + 12.0));
                p.rect_filled(br, CornerRadius::same(255), bg);
                p.text(br.center(), Align2::CENTER_CENTER, txt, bold(12.0), fg);
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    // the box ticks; the rest of the row only shows the file
                    let x = resp.interact_pointer_pos().map_or(f32::MAX, |p| p.x);
                    if x < r.left() + 44.0 {
                        toggled = Some(idx);
                    } else {
                        clicked = Some(idx);
                    }
                }
            }
        });
        if let Some(idx) = toggled {
            self.files[idx].checked = !self.files[idx].checked;
        }
        if let Some(idx) = clicked
            && self.focus != Some(idx)
        {
            self.focus = Some(idx);
            self.stop_sound();
        }
        self.view = visible;
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        let w = ui.available_width();
        Frame::new()
            .fill(Color32::from_rgb(249, 250, 251))
            .corner_radius(CornerRadius::same(14))
            .stroke(Stroke::new(1.0, BORDER))
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                ui.set_width(w - 28.0);
                ui.set_min_height(ui.available_height() - 28.0);
                let Some(idx) = self.focus.filter(|&i| i < self.files.len()) else {
                    ui.add_space(60.0);
                    ui.vertical_centered(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
                        icons::draw(ui.painter(), Icon::Pointer, r.center(), 34.0, MUTED);
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(t!("Aperçu")).font(bold(17.0)).color(TEXT));
                        ui.label(egui::RichText::new(t!("Cliquez sur un fichier pour le voir ici.")).font(regular(14.0)).color(MUTED));
                    });
                    return;
                };
                let f = self.files[idx].clone();
                let h = ui.available_height();
                if ui.available_width() >= 420.0 {
                    // wide: the picture takes the whole height, the details sit beside it
                    let details_w = 200.0;
                    let gap = 14.0;
                    let pic_w = ui.available_width() - details_w - gap;
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let (r, _) = ui.allocate_exact_size(vec2(pic_w, h.max(160.0)), Sense::hover());
                        self.preview_area(ui, &f, r);
                        ui.add_space(gap);
                        ui.allocate_ui_with_layout(vec2(details_w, h), Layout::top_down(Align::Min), |ui| {
                            ui.set_width(details_w);
                            ui.spacing_mut().item_spacing.x = 8.0;
                            // a low window scrolls the details rather than cutting the buttons
                            egui::ScrollArea::vertical().id_salt("preview-details").auto_shrink([false, true]).show(ui, |ui| {
                                ui.set_width(details_w);
                                self.preview_details(ui, idx, &f, true);
                            });
                        });
                    });
                } else {
                    // narrow: the preview shrinks first so the details and the buttons stay visible
                    let area = vec2(ui.available_width(), (h - 262.0).clamp(70.0, 300.0));
                    let (r, _) = ui.allocate_exact_size(area, Sense::hover());
                    self.preview_area(ui, &f, r);
                    ui.add_space(8.0);
                    self.preview_details(ui, idx, &f, false);
                }
            });
    }

    /// The picture, the first lines or the first bytes of `f`, fitted in `r`.
    fn preview_area(&mut self, ui: &mut egui::Ui, f: &Found, r: Rect) {
        ui.painter().rect_filled(r, CornerRadius::same(10), CARD);
        let mut drawn = false;
        let expert = self.settings.expert;
        if preview::can_preview(f) || expert {
            match &self.preview {
                Some((id, Shown::Picture(tex))) if *id == f.id => {
                    let s = tex.size_vec2();
                    let inner = r.shrink(6.0);
                    // small thumbnails may grow a little, never blurry beyond twice their size
                    let k = (inner.width() / s.x).min(inner.height() / s.y).min(2.0);
                    let ir = Rect::from_center_size(r.center(), s * k);
                    ui.painter().image(tex.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    drawn = true;
                }
                Some((id, Shown::Text(t))) if *id == f.id => {
                    let t = t.clone();
                    ui.scope_builder(egui::UiBuilder::new().max_rect(r.shrink(8.0)), |ui| {
                        egui::ScrollArea::both().id_salt(("text-preview", f.id)).auto_shrink([false, false]).show(ui, |ui| {
                            ui.add(egui::Label::new(egui::RichText::new(t).font(FontId::monospace(12.0)).color(TEXT)).extend());
                        });
                    });
                    drawn = true;
                }
                Some((id, Shown::Nothing)) if *id == f.id => {}
                _ => {
                    if self.preview_wanted != Some(f.id) {
                        self.preview_wanted = Some(f.id);
                        preview::load(f.clone(), expert, ui.ctx().clone(), self.preview_slot.clone());
                    }
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, t!("Chargement de l'aperçu…"), regular(13.0), MUTED);
                    drawn = true;
                }
            }
        }
        if !drawn {
            let (icon, color, soft) = cat_style(f.category);
            let c = r.center() - vec2(0.0, if r.height() > 120.0 { 10.0 } else { 0.0 });
            ui.painter().circle_filled(c, 40.0, soft);
            icons::draw(ui.painter(), icon, c, 38.0, color);
            if r.height() > 120.0 {
                ui.painter().text(
                    pos2(r.center().x, c.y + 62.0),
                    Align2::CENTER_CENTER,
                    t!("Pas d'aperçu pour ce fichier"),
                    regular(12.5),
                    MUTED,
                );
            }
        }
    }

    /// Name, type, size, date and origin of the focused file, and what can be done with it.
    /// `column`: laid out in a narrow column beside the picture.
    fn preview_details(&mut self, ui: &mut egui::Ui, idx: usize, f: &Found, column: bool) {
        let kind = if f.ext == "dossier" {
            t!("dossier").to_string()
        } else if f.ext.is_empty() {
            f.category.label().to_string()
        } else {
            format!("{} · {}", f.category.label(), f.ext.to_uppercase())
        };
        let date_label = if matches!(f.origin, Origin::Recycled { .. }) { t!("Supprimé le") } else { t!("Date") };
        let place = match &f.origin {
            Origin::Recycled { .. } => t!("Corbeille"),
            Origin::Raw { .. } => t!("Espace libre du disque"),
            Origin::Staged { .. } => t!("Analyse approfondie"),
            Origin::Runs { .. } | Origin::Bytes(_) => t!("Analyse rapide"),
        };
        let facts = [
            (t!("Type"), kind),
            (t!("Taille"), human_size(f.size)),
            (date_label, f.date.clone().unwrap_or_else(|| t!("inconnue").into())),
            (t!("Trouvé dans"), place.to_string()),
        ];
        if column {
            ui.add(egui::Label::new(egui::RichText::new(&f.name).font(bold(16.0)).color(TEXT)).wrap()).on_hover_text(&f.place);
            ui.add_space(6.0);
            for (k, v) in facts {
                ui.horizontal_top(|ui| {
                    ui.label(egui::RichText::new(k).font(regular(12.5)).color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(v).font(regular(13.0)).color(TEXT)).wrap());
                    });
                });
            }
            ui.add_space(8.0);
        } else {
            ui.label(egui::RichText::new(elide(&f.name, 34)).font(bold(16.0)).color(TEXT));
            ui.add_space(6.0);
            for (k, v) in facts {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(k).font(regular(13.0)).color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(egui::RichText::new(v).font(regular(13.0)).color(TEXT));
                    });
                });
            }
            ui.add_space(8.0);
        }
        // listen / open, when the file sits somewhere other programs can read it
        if let Some(path) = preview::local_path(f).map(|p| p.to_path_buf()) {
            ui.horizontal(|ui| {
                let half = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
                let mut open_w = ui.available_width();
                if preview::is_sound(f) && sys::Player::AVAILABLE {
                    open_w = half;
                    let on = self.playing == Some(f.id);
                    let label = if on { t!("Arrêter") } else { t!("Écouter") };
                    let icon = if on { Icon::Stop } else { Icon::Play };
                    if icon_button(ui, vec2(half, 34.0), Some(icon), label, false, true).clicked() {
                        if on {
                            self.stop_sound();
                        } else if self.player.play(&path) {
                            self.playing = Some(f.id);
                        } else {
                            self.notice = Some(t!("Ce son ne peut pas être lu ici : essayez « Ouvrir ».").into());
                        }
                    }
                }
                if icon_button(ui, vec2(open_w, 34.0), None, t!("Ouvrir"), false, true)
                    .on_hover_text(t!("Ouvre le fichier dans le programme habituel, sans le récupérer."))
                    .clicked()
                {
                    sys::open_path(&path);
                }
            });
            ui.add_space(4.0);
        }
        // ticking stays a separate, deliberate choice
        let (cr, cresp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
        checkbox_at(ui.painter(), pos2(cr.left() + 11.0, cr.center().y), f.checked, false);
        let ctext = if f.checked { t!("Coché pour la récupération") } else { t!("Cocher pour la récupération") };
        ui.painter().text(pos2(cr.left() + 30.0, cr.center().y), Align2::LEFT_CENTER, ctext, bold(13.5), TEXT);
        if cresp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if cresp.clicked() {
            self.files[idx].checked = !f.checked;
        }
        ui.add_space(4.0);
        let busy = self.copying();
        if pill_button(ui, vec2(ui.available_width(), 38.0), t!("Récupérer ce fichier"), false, !busy).clicked() {
            self.ask_destination(vec![f.clone()], ui.ctx());
        }
    }

    fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        let checked: Vec<&Found> = self.files.iter().filter(|f| f.checked).collect();
        let n = checked.len();
        let bytes: u64 = checked.iter().map(|f| f.size).sum();
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 14.0, ACCENT);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, "3", bold(15.0), Color32::WHITE);
            ui.add_space(4.0);
            ui.vertical(|ui| {
                let copy = self.copy.as_ref().map(|c| {
                    let g = c.lock().unwrap();
                    (g.done_files, g.total_files, g.failed, g.finished, g.folder.clone())
                });
                match copy {
                    Some((done, total, _, false, _)) => {
                        ui.label(egui::RichText::new(t!("Récupération… {} / {}", done, total)).font(bold(17.0)).color(TEXT));
                        self.progress_bar(ui, 320.0, done as f32 / total.max(1) as f32, true);
                    }
                    Some((done, _, failed, true, folder)) => {
                        let ok = done - failed;
                        let done_txt = plural(ok, t!("fichier récupéré"), t!("fichiers récupérés"));
                        let txt = if failed == 0 {
                            t!("{} — c'est fait !", done_txt)
                        } else {
                            t!("{} · {} impossible(s) à lire", done_txt, failed)
                        };
                        ui.horizontal(|ui| {
                            icon_label(ui, Icon::Check, 18.0, GREEN, egui::RichText::new(txt).font(bold(17.0)).color(GREEN));
                            if ui.link(egui::RichText::new(t!("Ouvrir le dossier")).font(bold(14.0)).color(ACCENT)).clicked() {
                                sys::open_path(&folder);
                            }
                        });
                        ui.label(egui::RichText::new(folder.to_string_lossy()).font(regular(12.5)).color(MUTED));
                    }
                    None => {
                        let head = if n == 0 {
                            t!("Aucun fichier coché").to_string()
                        } else {
                            format!("{} · {}", plural(n, t!("fichier coché"), t!("fichiers cochés")), human_size(bytes))
                        };
                        ui.label(egui::RichText::new(head).font(bold(17.0)).color(TEXT));
                        let tip = self
                            .notice
                            .clone()
                            .unwrap_or_else(|| t!("Conseil : enregistrez-les sur un autre disque (une clé USB par exemple).").into());
                        ui.label(egui::RichText::new(tip).font(regular(13.0)).color(MUTED));
                    }
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let busy = self.copying();
                if pill_button(ui, vec2(270.0, 52.0), t!("Récupérer la sélection"), true, n > 0 && !busy).clicked() {
                    let files: Vec<Found> = self.files.iter().filter(|f| f.checked).cloned().collect();
                    self.ask_destination(files, ui.ctx());
                }
                ui.add_space(10.0);
                if pill_button(ui, vec2(150.0, 52.0), t!("Tout décocher"), false, n > 0 && !busy)
                    .on_hover_text(t!("Décoche tous les fichiers, même ceux masqués par les filtres."))
                    .clicked()
                {
                    for f in &mut self.files {
                        f.checked = false;
                    }
                }
                let can_all = self.view.iter().any(|&i| !self.files[i].checked);
                if pill_button(ui, vec2(150.0, 52.0), t!("Tout cocher"), false, can_all && !busy)
                    .on_hover_text(t!("Coche tous les fichiers affichés dans la liste."))
                    .clicked()
                {
                    for &i in &self.view {
                        self.files[i].checked = true;
                    }
                }
            });
        });
    }

    fn confirm_modal(&mut self, ctx: &egui::Context) {
        let Some((dest, files)) = self.confirm_dest.clone() else { return };
        let mut close = false;
        let mut go = false;
        let mut other = false;
        egui::Modal::new(egui::Id::unique("same-disk")).show(ctx, |ui| {
            ui.set_width(440.0);
            icon_label(
                ui,
                Icon::Warning,
                20.0,
                AMBER,
                egui::RichText::new(t!("Même disque que les fichiers perdus")).font(bold(18.0)).color(AMBER),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(t!(
                    "Enregistrer ici peut écraser d'autres fichiers encore récupérables. Il vaut mieux choisir une clé USB ou un autre disque."
                ))
                .font(regular(14.5))
                .color(TEXT),
            );
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if pill_button(ui, vec2(210.0, 42.0), t!("Choisir un autre endroit"), true, true).clicked() {
                    other = true;
                }
                if pill_button(ui, vec2(170.0, 42.0), t!("Continuer quand même"), false, true).clicked() {
                    go = true;
                }
            });
            ui.add_space(4.0);
            if ui.link(egui::RichText::new(t!("Annuler")).color(MUTED)).clicked() {
                close = true;
            }
        });
        if go {
            self.copy = Some(engine::start_recovery(files, dest, ctx.clone()));
            self.confirm_dest = None;
        } else if other {
            self.confirm_dest = None;
            self.ask_destination(files, ctx);
        } else if close {
            self.confirm_dest = None;
        }
    }

    /// Explains the two kinds of search and offers the deep one, at start and on demand.
    fn deep_modal(&mut self, ctx: &egui::Context) {
        if !self.ask_deep || self.admin {
            return;
        }
        let mut go = false;
        let mut stay = false;
        egui::Modal::new(egui::Id::unique("deep-search")).show(ctx, |ui| {
            ui.set_width(500.0);
            icon_label(ui, Icon::Shield, 22.0, ACCENT, egui::RichText::new(t!("Jusqu'où chercher ?")).font(bold(20.0)).color(TEXT));
            ui.add_space(10.0);
            let item = |ui: &mut egui::Ui, title: &str, body: &str| {
                Frame::new()
                    .fill(Color32::from_rgb(249, 250, 251))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new(title).font(bold(15.0)).color(TEXT));
                        ui.label(egui::RichText::new(body).font(regular(14.0)).color(MUTED));
                    });
            };
            item(
                ui,
                t!("Recherche rapide (maintenant)"),
                t!("Ne regarde que dans la corbeille. Quelques secondes, aucune autorisation demandée. \
                    Si la corbeille a été vidée, les fichiers ne seront pas retrouvés."),
            );
            ui.add_space(6.0);
            item(
                ui,
                t!("Recherche approfondie (conseillée)"),
                &t!(
                    "Lit tout le disque pour retrouver aussi les fichiers effacés depuis longtemps ou après avoir vidé \
                     la corbeille. Plus long : de quelques minutes à plus d'une heure selon le disque. \
                     {} vous demandera d'autoriser l'application ({}).",
                    system(),
                    sys::admin_word()
                ),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if icon_button(ui, vec2(290.0, 44.0), Some(Icon::Shield), t!("Passer en recherche approfondie"), true, true).clicked() {
                    go = true;
                }
                if pill_button(ui, vec2(190.0, 44.0), t!("Rester en rapide"), false, true).clicked() {
                    stay = true;
                }
            });
            ui.add_space(8.0);
            let mut never = self.settings.never_ask_deep;
            if ui
                .checkbox(&mut never, egui::RichText::new(t!("Ne plus me le proposer au démarrage")).font(regular(13.5)).color(MUTED))
                .changed()
            {
                self.settings.never_ask_deep = never;
                self.settings.save();
            }
        });
        if go {
            if sys::relaunch_as_admin() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.notice = Some(t!("{} n'a pas donné l'autorisation : la recherche reste rapide.", system()));
            }
            self.ask_deep = false;
        } else if stay {
            self.ask_deep = false;
        }
    }

    /// Expert mode: every step of the search, PhotoRec's own lines included, as they come.
    fn journal(&mut self, ui: &mut egui::Ui) {
        let light = Color32::from_rgb(229, 231, 235);
        let dim = Color32::from_rgb(156, 163, 175);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(t!("Journal de la recherche")).font(bold(14.0)).color(light));
            ui.label(egui::RichText::new(plural(self.log.len(), t!("ligne"), t!("lignes"))).font(regular(12.5)).color(dim));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let link = |s: &str| egui::RichText::new(s).font(bold(13.0)).color(Color32::from_rgb(165, 180, 252));
                if ui.link(link(t!("Effacer"))).clicked() {
                    self.log.clear();
                }
                ui.add_space(8.0);
                if ui.link(link(t!("Copier tout"))).clicked() {
                    ui.ctx().copy_text(self.log.join("\n"));
                }
            });
        });
        ui.add_space(4.0);
        if self.log.is_empty() {
            ui.label(
                egui::RichText::new(t!(
                    "Le journal se remplit pendant la recherche : chaque étape, et en recherche approfondie les lignes de PhotoRec."
                ))
                .font(FontId::monospace(12.0))
                .color(dim),
            );
            return;
        }
        let row_h = 17.0;
        egui::ScrollArea::vertical().id_salt("journal").auto_shrink([false, false]).stick_to_bottom(true).show_rows(
            ui,
            row_h,
            self.log.len(),
            |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let (dropped, kept) = (t!("écarté"), t!("gardé"));
                for line in &self.log[range] {
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::hover());
                    let text = line.split_once("] ").map_or("", |x| x.1);
                    let color = if text.starts_with(dropped) {
                        Color32::from_rgb(107, 114, 128)
                    } else if text.starts_with(kept) {
                        Color32::from_rgb(134, 239, 172)
                    } else if text.starts_with("» ") {
                        Color32::from_rgb(147, 197, 253)
                    } else {
                        light
                    };
                    let max = (r.width() / 7.3) as usize;
                    ui.painter().text(r.left_center(), Align2::LEFT_CENTER, elide(line, max.max(20)), FontId::monospace(12.0), color);
                }
            },
        );
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        self.frames += 1;
        ctx.request_repaint();
        // show the preview too: the first file found is opened
        if self.focus.is_none()
            && let Some(i) = self.files.iter().position(|f| f.ext != "dossier")
        {
            self.focus = Some(i);
        }
        if self.shot_done.load(Ordering::Relaxed) || self.frames > 900 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.frames == 60 {
            self.apply_scene();
        }
        if self.frames >= 120 && !self.shot_requested {
            self.shot_requested = true;
            let done = self.shot_done.clone();
            let c = ctx.clone();
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let raw: Vec<u8> = img.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, raw) {
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    if let Err(e) = buf.save(&path) {
                        eprintln!("screenshot: {e}");
                    }
                }
                done.store(true, Ordering::Relaxed);
                c.request_repaint();
            });
        }
    }
}

impl Drop for App {
    /// Stops PhotoRec and empties its scratch folder when the window closes.
    fn drop(&mut self) {
        if let Some(job) = self.scan.take() {
            job.cancel.store(true, Ordering::Relaxed);
            let t = std::time::Instant::now();
            while !job.shared.lock().unwrap().done && t.elapsed().as_secs() < 5 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            clear_stage(&job);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.handle_screenshot(&ctx);
        // closing while the search (or a copy) runs only puts the window away
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quitting
            && self.screenshot.is_none()
            && (self.scanning() || self.copying())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.go_background(&ctx);
        }

        egui::Panel::top("top")
            .frame(Frame::new().fill(CARD).inner_margin(Margin::symmetric(24, 12)).stroke(Stroke::new(1.0, BORDER)))
            .show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("bottom")
            .frame(Frame::new().fill(CARD).inner_margin(Margin::symmetric(24, 14)).stroke(Stroke::new(1.0, BORDER)))
            .show(ui, |ui| self.bottom_bar(ui));
        if self.settings.expert {
            egui::Panel::bottom("journal")
                .resizable(true)
                .default_size(200.0)
                .min_size(90.0)
                .max_size(520.0)
                .frame(Frame::new().fill(Color32::from_rgb(17, 24, 39)).inner_margin(Margin::symmetric(20, 10)))
                .show(ui, |ui| self.journal(ui));
        }
        egui::CentralPanel::default().frame(Frame::new().fill(BG).inner_margin(Margin::same(20))).show(ui, |ui| {
            self.disk_section(ui);
            ui.add_space(16.0);
            self.results_section(ui);
        });
        self.confirm_modal(&ctx);
        self.deep_modal(&ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }
}
