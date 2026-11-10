//! The colour themes: a light and a dark one of our own, and popular editor palettes.
//! Every colour of the interface comes from the current theme's palette.

use eframe::egui::{self, Color32, Stroke};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Every colour the interface draws with.
pub struct Palette {
    /// The window behind the cards.
    pub bg: Color32,
    pub card: Color32,
    /// A slightly raised surface inside a card: the preview, a hovered row.
    pub raised: Color32,
    /// A hovered chip, a neutral badge.
    pub hover: Color32,
    pub text: Color32,
    pub muted: Color32,
    /// The used part of a drive's bar when it isn't selected.
    pub faint: Color32,
    pub border: Color32,
    /// Outlines that must show: an empty tick box, a dashed card.
    pub border_strong: Color32,
    /// The empty part of a bar.
    pub track: Color32,
    pub accent: Color32,
    /// The accent under the mouse.
    pub accent_hover: Color32,
    /// The background of what's selected.
    pub accent_soft: Color32,
    pub accent_mid: Color32,
    /// A disabled main button, a secondary button's outline.
    pub accent_muted: Color32,
    /// Text drawn on the accent.
    pub on_accent: Color32,
    /// Text on a disabled main button.
    pub on_accent_muted: Color32,
    pub green: Color32,
    pub green_soft: Color32,
    pub amber: Color32,
    pub amber_soft: Color32,
    pub red: Color32,
    pub red_soft: Color32,
    /// The box of a warning.
    pub warn_bg: Color32,
    /// The colours of the kinds of files: photos, videos, music, documents, archives, others.
    pub cats: [Color32; 6],
    /// The search journal, always dark like a terminal.
    pub log_bg: Color32,
    pub log_text: Color32,
    pub log_dim: Color32,
    pub log_faint: Color32,
    pub log_link: Color32,
    pub log_kept: Color32,
    pub log_info: Color32,
    pub log_grip: Color32,
}

pub struct Theme {
    /// Saved in the settings.
    pub id: &'static str,
    /// Shown in the menu: translated for ours, the palette's own name for the others.
    pub name: &'static str,
    pub dark: bool,
    pub p: Palette,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// `a` moved towards `b` by `t` 256ths.
const fn mix(a: Color32, b: Color32, t: u32) -> Color32 {
    const fn m(x: u8, y: u8, t: u32) -> u8 {
        ((x as u32 * (256 - t) + y as u32 * t) / 256) as u8
    }
    Color32::from_rgb(m(a.r(), b.r(), t), m(a.g(), b.g(), t), m(a.b(), b.b(), t))
}

const fn luma(c: Color32) -> u32 {
    (c.r() as u32 * 299 + c.g() as u32 * 587 + c.b() as u32 * 114) / 1000
}

const WHITE: Color32 = Color32::WHITE;
const BLACK: Color32 = Color32::BLACK;

/// The colours a theme chooses; the others are worked out from them.
struct Base {
    dark: bool,
    bg: u32,
    card: u32,
    text: u32,
    muted: u32,
    border: u32,
    accent: u32,
    green: u32,
    amber: u32,
    red: u32,
    cats: [u32; 6],
    log_bg: u32,
}

const fn build(b: Base) -> Palette {
    let (bg, card, text, muted, border) = (rgb(b.bg), rgb(b.card), rgb(b.text), rgb(b.muted), rgb(b.border));
    let (accent, green, amber, red) = (rgb(b.accent), rgb(b.green), rgb(b.amber), rgb(b.red));
    // tints are stronger on a dark card, or they don't show
    let soft = if b.dark { 46 } else { 26 };
    let ink = if b.dark { bg } else { rgb(0x111827) };
    let on_accent = if luma(accent) > 140 { ink } else { WHITE };
    let log_bg = rgb(b.log_bg);
    let log_text = if b.dark { text } else { rgb(0xe5e7eb) };
    let c = b.cats;
    Palette {
        bg,
        card,
        raised: mix(card, text, if b.dark { 10 } else { 4 }),
        hover: mix(card, text, if b.dark { 20 } else { 10 }),
        text,
        muted,
        faint: mix(muted, card, 80),
        border,
        border_strong: mix(border, text, if b.dark { 50 } else { 40 }),
        track: mix(card, text, if b.dark { 30 } else { 26 }),
        accent,
        accent_hover: if b.dark { mix(accent, WHITE, 36) } else { mix(accent, BLACK, 36) },
        accent_soft: mix(card, accent, soft),
        accent_mid: mix(card, accent, soft * 2),
        accent_muted: mix(card, accent, if b.dark { 100 } else { 72 }),
        on_accent,
        on_accent_muted: if b.dark { mix(card, text, 120) } else { WHITE },
        green,
        green_soft: mix(card, green, soft + 8),
        amber,
        amber_soft: mix(card, amber, soft + 8),
        red,
        red_soft: mix(card, red, soft + 8),
        warn_bg: mix(card, amber, soft / 2),
        cats: [rgb(c[0]), rgb(c[1]), rgb(c[2]), rgb(c[3]), rgb(c[4]), rgb(c[5])],
        log_bg,
        log_text,
        log_dim: mix(log_bg, log_text, 150),
        log_faint: mix(log_bg, log_text, 110),
        log_link: if b.dark { accent } else { mix(accent, WHITE, 140) },
        log_kept: mix(green, WHITE, 90),
        log_info: mix(rgb(c[3]), WHITE, 90),
        log_grip: mix(log_bg, log_text, 70),
    }
}

/// The first two are the ones the day/night button switches between.
pub const THEMES: &[Theme] = &[
    Theme {
        id: "clair",
        name: "Clair",
        dark: false,
        // the original look, kept exactly where the worked-out colours drift
        p: Palette {
            raised: rgb(0xf9fafb),
            hover: rgb(0xf3f4f6),
            faint: rgb(0x9ca3af),
            border_strong: rgb(0xd1d5db),
            track: rgb(0xe5e7eb),
            accent_hover: rgb(0x4338ca),
            accent_soft: rgb(0xeef0ff),
            accent_mid: rgb(0xe0e7ff),
            accent_muted: rgb(0xc7caee),
            green_soft: rgb(0xdcfce7),
            amber_soft: rgb(0xfef3c7),
            red_soft: rgb(0xfee2e2),
            warn_bg: rgb(0xfffbeb),
            log_link: rgb(0xa5b4fc),
            log_kept: rgb(0x86efac),
            log_info: rgb(0x93c5fd),
            log_dim: rgb(0x9ca3af),
            log_faint: rgb(0x6b7280),
            log_grip: rgb(0x4b5563),
            ..build(Base {
                dark: false,
                bg: 0xf4f6fb,
                card: 0xffffff,
                text: 0x111827,
                muted: 0x6b7280,
                border: 0xe5e7eb,
                accent: 0x4f46e5,
                green: 0x16a34a,
                amber: 0xb45309,
                red: 0xb91c1c,
                cats: [0x7c3aed, 0xdc2626, 0xea580c, 0x2563eb, 0xa16207, 0x4b5563],
                log_bg: 0x111827,
            })
        },
    },
    Theme {
        id: "sombre",
        name: "Sombre",
        dark: true,
        p: build(Base {
            dark: true,
            bg: 0x0f131c,
            card: 0x1a202c,
            text: 0xe5e7eb,
            muted: 0x9ca3af,
            border: 0x2d3544,
            accent: 0x818cf8,
            green: 0x4ade80,
            amber: 0xfbbf24,
            red: 0xf87171,
            cats: [0xa78bfa, 0xf87171, 0xfb923c, 0x60a5fa, 0xfacc15, 0x9ca3af],
            log_bg: 0x0a0d14,
        }),
    },
    Theme {
        id: "nord",
        name: "Nord",
        dark: true,
        p: build(Base {
            dark: true,
            bg: 0x2e3440,
            card: 0x3b4252,
            text: 0xeceff4,
            muted: 0xb4bccc,
            border: 0x4c566a,
            accent: 0x88c0d0,
            green: 0xa3be8c,
            amber: 0xebcb8b,
            red: 0xbf616a,
            cats: [0xb48ead, 0xbf616a, 0xd08770, 0x81a1c1, 0xebcb8b, 0xd8dee9],
            log_bg: 0x242933,
        }),
    },
    Theme {
        id: "catppuccin-mocha",
        name: "Catppuccin Mocha",
        dark: true,
        p: build(Base {
            dark: true,
            bg: 0x181825,
            card: 0x1e1e2e,
            text: 0xcdd6f4,
            muted: 0xa6adc8,
            border: 0x313244,
            accent: 0xcba6f7,
            green: 0xa6e3a1,
            amber: 0xf9e2af,
            red: 0xf38ba8,
            cats: [0xcba6f7, 0xf38ba8, 0xfab387, 0x89b4fa, 0xf9e2af, 0xa6adc8],
            log_bg: 0x11111b,
        }),
    },
    Theme {
        id: "catppuccin-latte",
        name: "Catppuccin Latte",
        dark: false,
        p: build(Base {
            dark: false,
            bg: 0xe6e9ef,
            card: 0xeff1f5,
            text: 0x4c4f69,
            muted: 0x6c6f85,
            border: 0xccd0da,
            accent: 0x8839ef,
            green: 0x40a02b,
            amber: 0xdf8e1d,
            red: 0xd20f39,
            cats: [0x8839ef, 0xd20f39, 0xfe640b, 0x1e66f5, 0xdf8e1d, 0x6c6f85],
            log_bg: 0x303446,
        }),
    },
    Theme {
        id: "tokyo-night",
        name: "Tokyo Night",
        dark: true,
        p: build(Base {
            dark: true,
            bg: 0x16161e,
            card: 0x1f2335,
            text: 0xc0caf5,
            muted: 0x8c94c0,
            border: 0x2f3549,
            accent: 0x7aa2f7,
            green: 0x9ece6a,
            amber: 0xe0af68,
            red: 0xf7768e,
            cats: [0xbb9af7, 0xf7768e, 0xff9e64, 0x7aa2f7, 0xe0af68, 0xa9b1d6],
            log_bg: 0x101014,
        }),
    },
    Theme {
        id: "solarized-dark",
        name: "Solarized Dark",
        dark: true,
        p: build(Base {
            dark: true,
            bg: 0x002b36,
            card: 0x073642,
            text: 0xeee8d5,
            muted: 0x93a1a1,
            border: 0x1c4b57,
            accent: 0x268bd2,
            green: 0x859900,
            amber: 0xb58900,
            red: 0xdc322f,
            cats: [0x6c71c4, 0xdc322f, 0xcb4b16, 0x268bd2, 0xb58900, 0x93a1a1],
            log_bg: 0x00212b,
        }),
    },
    Theme {
        id: "solarized-light",
        name: "Solarized Light",
        dark: false,
        p: build(Base {
            dark: false,
            bg: 0xeee8d5,
            card: 0xfdf6e3,
            text: 0x073642,
            muted: 0x657b83,
            border: 0xe1dbc5,
            accent: 0x268bd2,
            green: 0x859900,
            amber: 0xb58900,
            red: 0xdc322f,
            cats: [0x6c71c4, 0xdc322f, 0xcb4b16, 0x268bd2, 0xb58900, 0x657b83],
            log_bg: 0x073642,
        }),
    },
    Theme {
        id: "monokai",
        name: "Monokai",
        dark: true,
        p: build(Base {
            dark: true,
            bg: 0x1e1f1c,
            card: 0x272822,
            text: 0xf8f8f2,
            muted: 0xa59f85,
            border: 0x3e3d32,
            accent: 0xf92672,
            green: 0xa6e22e,
            amber: 0xfd971f,
            red: 0xff5f5f,
            cats: [0xae81ff, 0xf92672, 0xfd971f, 0x66d9ef, 0xe6db74, 0xa59f85],
            log_bg: 0x171814,
        }),
    },
    Theme {
        id: "gruvbox",
        name: "Gruvbox",
        dark: true,
        p: build(Base {
            dark: true,
            bg: 0x1d2021,
            card: 0x282828,
            text: 0xebdbb2,
            muted: 0xa89984,
            border: 0x3c3836,
            accent: 0xfabd2f,
            green: 0xb8bb26,
            amber: 0xfe8019,
            red: 0xfb4934,
            cats: [0xd3869b, 0xfb4934, 0xfe8019, 0x83a598, 0xfabd2f, 0xa89984],
            log_bg: 0x141617,
        }),
    },
];

static CURRENT: AtomicUsize = AtomicUsize::new(0);

pub fn current() -> usize {
    CURRENT.load(Ordering::Relaxed)
}

pub fn theme() -> &'static Theme {
    &THEMES[current()]
}

/// The current theme's colours.
pub fn pal() -> &'static Palette {
    &theme().p
}

/// Switches to the theme saved as `id`; false when there's none.
pub fn set(id: &str) -> bool {
    match THEMES.iter().position(|t| t.id == id) {
        Some(i) => {
            CURRENT.store(i, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// The light theme from a dark one, the dark theme from a light one.
pub fn day_night() -> &'static str {
    if theme().dark { THEMES[0].id } else { THEMES[1].id }
}

/// Readable text on `c`: white on a deep colour, the theme's ink on a pale one.
pub fn ink_on(c: Color32) -> Color32 {
    if luma(c) > 140 { if theme().dark { pal().bg } else { rgb(0x111827) } } else { WHITE }
}

/// The pale background behind a coloured icon.
pub fn soft(c: Color32) -> Color32 {
    mix(pal().card, c, if theme().dark { 46 } else { 30 })
}

/// Hands the theme's colours to egui's own widgets: menus, tooltips, scroll bars, text fields.
pub fn apply(ctx: &egui::Context) {
    let t = theme();
    let p = &t.p;
    ctx.all_styles_mut(|s| {
        let v = &mut s.visuals;
        *v = if t.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        v.panel_fill = p.bg;
        v.window_fill = p.card;
        v.faint_bg_color = p.raised;
        v.extreme_bg_color = if t.dark { p.bg } else { WHITE };
        v.code_bg_color = p.raised;
        v.window_stroke = Stroke::new(1.0, p.border);
        v.selection.bg_fill = p.accent;
        // the chosen item of a menu: readable on the accent
        v.selection.stroke = Stroke::new(1.0, p.on_accent);
        v.hyperlink_color = p.accent;
        v.warn_fg_color = p.amber;
        v.error_fg_color = p.red;
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = p.card;
        w.noninteractive.weak_bg_fill = p.card;
        w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
        for (state, fill) in
            [(&mut w.inactive, p.raised), (&mut w.hovered, p.hover), (&mut w.active, p.accent_soft), (&mut w.open, p.hover)]
        {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.fg_stroke = Stroke::new(1.0, p.text);
        }
        w.inactive.bg_stroke = Stroke::new(1.0, p.border);
        w.hovered.bg_stroke = Stroke::new(1.0, p.border_strong);
        w.active.bg_stroke = Stroke::new(1.0, p.accent);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG contrast ratio between two colours.
    fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let ch = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
        };
        let (x, y) = (lum(a) + 0.05, lum(b) + 0.05);
        x.max(y) / x.min(y)
    }

    #[test]
    fn every_theme_is_readable() {
        let mut ids = std::collections::HashSet::new();
        for t in THEMES {
            assert!(ids.insert(t.id), "{} twice", t.id);
            let p = &t.p;
            assert!(contrast(p.text, p.card) >= 7.0, "{}: text on cards", t.id);
            assert!(contrast(p.muted, p.card) >= 3.5, "{}: muted text on cards", t.id);
            assert!(contrast(p.on_accent, p.accent) >= 3.0, "{}: text on the accent", t.id);
            assert!(contrast(p.log_text, p.log_bg) >= 7.0, "{}: journal", t.id);
        }
        assert!(!THEMES[0].dark && THEMES[1].dark);
    }
}
