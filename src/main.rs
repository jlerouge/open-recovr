#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod i18n;
mod app;
mod art;
mod bench;
mod carve;
mod engine;
mod icons;
mod ntfs;
mod photorec;
mod preview;
mod sys;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::path::PathBuf;
use std::sync::Arc;

/// The system's own font when it's there (Segoe UI on Windows), egui's otherwise (they also carry the icons).
fn setup_fonts(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let fallback = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut bold_family = fallback.clone();
    let (regular, bold) = sys::ui_fonts();
    let first = |paths: Vec<PathBuf>| paths.iter().find_map(|p| std::fs::read(p).ok());
    if let Some(bytes) = first(regular) {
        defs.font_data.insert("system".into(), Arc::new(FontData::from_owned(bytes)));
        defs.families.entry(FontFamily::Proportional).or_default().insert(0, "system".into());
    }
    if let Some(bytes) = first(bold) {
        defs.font_data.insert("system-bold".into(), Arc::new(FontData::from_owned(bytes)));
        bold_family.insert(0, "system-bold".into());
    }
    defs.families.insert(FontFamily::Name("bold".into()), bold_family);
    ctx.set_fonts(defs);

    ctx.all_styles_mut(|s| {
        s.visuals = egui::Visuals::light();
        s.visuals.panel_fill = app::BG;
        s.visuals.window_fill = app::CARD;
        s.visuals.selection.bg_fill = app::ACCENT;
        s.visuals.hyperlink_color = app::ACCENT;
        s.text_styles = [
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Body, FontId::proportional(15.0)),
            (TextStyle::Button, FontId::proportional(15.0)),
            (TextStyle::Heading, FontId::proportional(22.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        s.spacing.item_spacing = egui::vec2(8.0, 4.0);
    });
}

fn main() -> eframe::Result {
    let mut screenshot = None;
    let mut search_now = false;
    let mut expert = false;
    let mut disk: Option<String> = None;
    let mut lang: Option<String> = None;
    let mut show: Option<PathBuf> = None;
    let mut scene: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--screenshot" => screenshot = args.next().map(PathBuf::from),
            // starts the search on the chosen drive right away
            "--search" => search_now = true,
            // shows the search journal for this run, without changing the saved choice
            "--expert" => expert = true,
            // a drive letter ("D") or a mount point ("/media/usb")
            "--disk" => disk = args.next(),
            // the interface language for this run ("en", "fr", "es", "de")
            "--lang" => lang = args.next(),
            // lists this file as found, to see its preview
            "--show" => show = args.next().map(PathBuf::from),
            // with --screenshot: the moment to draw ("home", "scanning", "selected", "recovered")
            "--scene" => scene = args.next(),
            // measures each way of searching on a disk image built for the purpose
            "--bench" => {
                bench::run(&args.next().map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("open-recovr-bench")));
                return Ok(());
            }
            "--photorec-test" => {
                let (image, stage) = (args.next().unwrap_or_default(), args.next().unwrap_or_default());
                photorec::self_test(&image, &stage, args.next().as_deref());
                return Ok(());
            }
            _ => {}
        }
    }
    app::choose_language(lang.as_deref());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(app::window_title())
            .with_inner_size([1240.0, 860.0])
            .with_min_inner_size([1000.0, 680.0])
            .with_icon(Arc::new(egui::IconData { rgba: art::app_icon(128), width: 128, height: 128 })),
        ..Default::default()
    };
    eframe::run_native(
        "Open Recovr",
        options,
        Box::new(move |cc| {
            setup_fonts(&cc.egui_ctx);
            let mut app = app::App::new(screenshot);
            // the native window, for the icon by the clock
            #[cfg(windows)]
            {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                if let Ok(h) = cc.window_handle()
                    && let RawWindowHandle::Win32(w) = h.as_raw()
                {
                    app.attach_tray(w.hwnd.get(), &cc.egui_ctx);
                }
            }
            #[cfg(not(windows))]
            app.attach_tray(0, &cc.egui_ctx);
            if expert {
                app.show_journal();
            }
            if let Some(d) = &disk {
                app.select_disk(d);
            }
            if search_now {
                app.start_scan(&cc.egui_ctx);
            }
            if let Some(s) = &scene {
                app.set_scene(s);
            }
            if let Some(p) = &show {
                app.show_file(p);
            }
            Ok(Box::new(app))
        }),
    )
}
