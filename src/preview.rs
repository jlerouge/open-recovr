//! What the preview panel can show of a found file: the picture itself, the first lines
//! of a text or of an Office document, or the thumbnail the system makes for videos and PDFs.

use crate::engine::{Category, FORMATS, Found, Origin, read_found};
use eframe::egui::ColorImage;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub enum Preview {
    Image(ColorImage),
    Text(String),
    Nothing,
}

pub type Slot = Arc<Mutex<Option<(usize, Preview)>>>;

const PICTURES: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp"];
const OFFICE: &[&str] = &["docx", "xlsx", "pptx", "odt", "ods", "odp"];
const TEXTS: &[&str] =
    &["txt", "md", "markdown", "csv", "log", "ini", "cfg", "conf", "yaml", "yml", "json", "xml", "html", "htm", "tex", "rtf"];

fn is_text(ext: &str) -> bool {
    // the label is the French key; `name()` would be the translated one
    TEXTS.contains(&ext) || FORMATS.iter().any(|f| f.label == "Code source" && f.exts.contains(&ext))
}

/// The file as it sits on disk, when there is one we can hand to other programs.
pub fn local_path(f: &Found) -> Option<&Path> {
    match &f.origin {
        Origin::Recycled { data, is_dir: false } => Some(data),
        Origin::Staged { path } => Some(path),
        _ => None,
    }
}

pub fn can_preview(f: &Found) -> bool {
    let e = f.ext.as_str();
    (PICTURES.contains(&e) && f.size <= 60 << 20)
        || is_text(e)
        || (OFFICE.contains(&e) && local_path(f).is_some())
        || (cfg!(windows) && local_path(f).is_some() && f.category != Category::Archives)
}

pub fn is_sound(f: &Found) -> bool {
    f.category == Category::Music && local_path(f).is_some()
}

/// The first `max` bytes of the file, wherever it was found.
fn read_head(f: &Found, max: u64) -> Option<Vec<u8>> {
    if let Some(p) = local_path(f) {
        let mut v = Vec::new();
        std::fs::File::open(p).ok()?.take(max).read_to_end(&mut v).ok()?;
        return Some(v);
    }
    let mut v = Vec::new();
    read_found(f, max, |b| {
        v.extend_from_slice(b);
        true
    })
    .ok()?;
    Some(v)
}

/// The first bytes as an editor would show them, for the expert mode.
pub fn hex_dump(f: &Found) -> Option<String> {
    let b = read_head(f, 1024)?;
    let mut out = String::new();
    for (i, row) in b.chunks(16).enumerate() {
        let hex: Vec<String> = row.iter().map(|x| format!("{x:02X}")).collect();
        let txt: String = row.iter().map(|&c| if (0x20..0x7f).contains(&c) { c as char } else { '.' }).collect();
        out.push_str(&format!("{:04X}  {:<47}  {txt}\n", i * 16, hex.join(" ")));
    }
    Some(out.trim_end().to_string())
}

/// Bytes to text: UTF-8 or UTF-16 with a mark, else Windows' western code page.
/// Nothing when it looks like a binary file.
fn decode(b: &[u8]) -> Option<String> {
    if let Some(rest) = b.strip_prefix(&[0xFF, 0xFE]) {
        let u: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return Some(String::from_utf16_lossy(&u));
    }
    if let Some(rest) = b.strip_prefix(&[0xFE, 0xFF]) {
        let u: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        return Some(String::from_utf16_lossy(&u));
    }
    let b = b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b);
    if b[..b.len().min(4096)].contains(&0) {
        return None;
    }
    match std::str::from_utf8(b) {
        Ok(s) => Some(s.to_string()),
        // cut in the middle of a character at the very end: still UTF-8
        Err(e) if e.valid_up_to() + 4 >= b.len() => Some(String::from_utf8_lossy(&b[..e.valid_up_to()]).into_owned()),
        Err(_) => Some(b.iter().map(|&c| c as char).collect()),
    }
}

/// Keeps what fits in the panel: the first lines, not too wide.
fn excerpt(s: &str) -> String {
    let mut out = String::new();
    for line in s.lines().take(80) {
        let line = line.replace('\t', "    ");
        let mut l: String = line.chars().filter(|c| !c.is_control()).take(160).collect();
        if line.chars().count() > 160 {
            l.push('…');
        }
        out.push_str(l.trim_end());
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// The words of an XML part, one line per paragraph.
fn xml_text(xml: &str, breaks: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('>') else { break };
        let tag = &rest[i + 1..i + j];
        if breaks.iter().any(|b| tag.starts_with(b)) {
            out.push('\n');
        } else if tag.ends_with(":tab/") || tag.starts_with("text:tab") {
            out.push(' ');
        }
        rest = &rest[i + j + 1..];
    }
    out.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn office_text(path: &Path, ext: &str) -> Option<String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut part = |name: &str| -> Option<String> {
        let mut s = String::new();
        zip.by_name(name).ok()?.take(8 << 20).read_to_string(&mut s).ok()?;
        Some(s)
    };
    let text = match ext {
        "docx" => xml_text(&part("word/document.xml")?, &["/w:p>", "w:br", "w:cr"]),
        "xlsx" => xml_text(&part("xl/sharedStrings.xml")?, &["/si>"]),
        "pptx" => {
            let mut all = String::new();
            for n in 1..=30 {
                let Some(x) = part(&format!("ppt/slides/slide{n}.xml")) else { break };
                if n > 1 {
                    all.push_str("\n———\n");
                }
                all.push_str(&xml_text(&x, &["/a:p>"]));
            }
            all
        }
        _ => xml_text(&part("content.xml")?, &["/text:p>", "/text:h>", "text:line-break", "/table:table-row>"]),
    };
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn picture(img: image::DynamicImage) -> ColorImage {
    let t = img.thumbnail(1024, 1024).to_rgba8();
    ColorImage::from_rgba_unmultiplied([t.width() as usize, t.height() as usize], t.as_raw())
}

fn build(f: &Found) -> Preview {
    let e = f.ext.as_str();
    if PICTURES.contains(&e)
        && let Some(img) = read_head(f, 60 << 20).and_then(|b| image::load_from_memory(&b).ok())
    {
        return Preview::Image(picture(img));
    }
    if is_text(e)
        && let Some(s) = read_head(f, 64 << 10).as_deref().and_then(decode)
    {
        let s = excerpt(&s);
        return if s.is_empty() { Preview::Text(t!("(fichier vide)").into()) } else { Preview::Text(s) };
    }
    if let Some(p) = local_path(f) {
        if OFFICE.contains(&e)
            && let Some(s) = office_text(p, e)
        {
            return Preview::Text(excerpt(&s));
        }
        if let Some(pic) = crate::sys::thumbnail(p, 640) {
            return Preview::Image(ColorImage::from_rgba_unmultiplied([pic.width, pic.height], &pic.rgba));
        }
    }
    Preview::Nothing
}

/// Prepares the preview off the interface thread; it lands in `slot` when ready.
/// In expert mode a file with nothing else to show gets its first bytes in hexadecimal.
pub fn load(f: Found, expert: bool, ctx: eframe::egui::Context, slot: Slot) {
    std::thread::spawn(move || {
        let mut p = if can_preview(&f) { build(&f) } else { Preview::Nothing };
        if expert
            && matches!(p, Preview::Nothing)
            && let Some(hex) = hex_dump(&f)
        {
            p = Preview::Text(hex);
        }
        *slot.lock().unwrap() = Some((f.id, p));
        ctx.request_repaint();
    });
}
