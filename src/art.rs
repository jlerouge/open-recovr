//! The coloured pictures: the app's icon and one picture per kind of drive, drawn as SVG and rendered here.

use eframe::egui::{self, ColorImage, Rect, TextureHandle, TextureOptions};
use std::cell::RefCell;
use std::collections::HashMap;

pub const ICON: &str = "icon";

const SOURCES: &[(&str, &str)] = &[
    (ICON, include_str!("../assets/icon.svg")),
    ("hdd", include_str!("../assets/hdd.svg")),
    ("ssd", include_str!("../assets/ssd.svg")),
    ("nvme", include_str!("../assets/nvme.svg")),
    ("optical", include_str!("../assets/optical.svg")),
    ("sd", include_str!("../assets/sd.svg")),
    ("microsd", include_str!("../assets/microsd.svg")),
    ("cf", include_str!("../assets/cf.svg")),
    ("usb", include_str!("../assets/usb.svg")),
    ("external", include_str!("../assets/external.svg")),
    ("phone", include_str!("../assets/phone.svg")),
];

/// The picture as premultiplied RGBA, `side` pixels square.
fn render(name: &str, side: u32) -> Option<Vec<u8>> {
    let src = SOURCES.iter().find(|(n, _)| *n == name)?.1;
    let tree = resvg::usvg::Tree::from_str(src, &resvg::usvg::Options::default()).ok()?;
    let mut pm = resvg::tiny_skia::Pixmap::new(side, side)?;
    let size = tree.size();
    let s = side as f32 / size.width().max(size.height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s, s), &mut pm.as_mut());
    Some(pm.take())
}

/// The app's icon as plain RGBA, for the window and the icon by the clock.
pub fn app_icon(side: u32) -> Vec<u8> {
    let mut px = render(ICON, side).unwrap_or_else(|| vec![0; (side * side * 4) as usize]);
    for p in px.as_chunks_mut::<4>().0 {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    px
}

/// Rendered pictures, kept per name and pixel size.
#[derive(Default)]
pub struct Art {
    cache: RefCell<HashMap<(&'static str, u32), Option<TextureHandle>>>,
}

impl Art {
    /// Draws the picture `name` to fill `rect`, rendered sharp for the screen's pixel density.
    pub fn paint(&self, ui: &egui::Ui, name: &'static str, rect: Rect) {
        let side = (rect.width().max(rect.height()) * ui.ctx().pixels_per_point()).round().max(1.0) as u32;
        let tex = self
            .cache
            .borrow_mut()
            .entry((name, side))
            .or_insert_with(|| {
                let px = render(name, side)?;
                let img = ColorImage::from_rgba_premultiplied([side as usize, side as usize], &px);
                Some(ui.ctx().load_texture(format!("art-{name}-{side}"), img, TextureOptions::LINEAR))
            })
            .clone();
        if let Some(tex) = tex {
            let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(tex.id(), rect, uv, egui::Color32::WHITE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_picture_renders() {
        for (name, _) in SOURCES {
            let px = render(name, 48).unwrap_or_else(|| panic!("{name} doesn't render"));
            assert!(px.iter().skip(3).step_by(4).any(|&a| a > 0), "{name} is empty");
        }
        let icon = app_icon(64);
        assert_eq!(icon.len(), 64 * 64 * 4);
    }
}
