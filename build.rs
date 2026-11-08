//! On Windows, gives the .exe its icon and its name in the file's properties.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.svg");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = out.join("open-recovr.ico");
    std::fs::write(&ico, icon_file(&std::fs::read_to_string("assets/icon.svg").unwrap())).unwrap();

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico.to_str().unwrap())
        .set("ProductName", "Open Recovr")
        .set("FileDescription", "Open Recovr")
        .set("LegalCopyright", "MIT License");
    res.compile().expect("Windows resources");
}

/// An .ico with the picture at the sizes Windows asks for, each as a 32-bit bitmap.
fn icon_file(svg: &str) -> Vec<u8> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
    let sides = [16u32, 24, 32, 48, 64, 128, 256];
    let images: Vec<Vec<u8>> = sides.iter().map(|&s| bitmap(&tree, s)).collect();

    let mut f = Vec::new();
    f.extend_from_slice(&[0, 0, 1, 0]);
    f.extend_from_slice(&(sides.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sides.len() as u32;
    for (s, img) in sides.iter().zip(&images) {
        let byte = if *s >= 256 { 0 } else { *s as u8 }; // 0 stands for 256
        f.extend_from_slice(&[byte, byte, 0, 0]);
        f.extend_from_slice(&1u16.to_le_bytes()); // planes
        f.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        f.extend_from_slice(&(img.len() as u32).to_le_bytes());
        f.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        f.extend_from_slice(&img);
    }
    f
}

/// The picture `side` pixels square, as an icon bitmap: header, BGRA rows bottom up, mask.
fn bitmap(tree: &resvg::usvg::Tree, side: u32) -> Vec<u8> {
    let mut pm = resvg::tiny_skia::Pixmap::new(side, side).unwrap();
    let size = tree.size();
    let k = side as f32 / size.width().max(size.height());
    resvg::render(tree, resvg::tiny_skia::Transform::from_scale(k, k), &mut pm.as_mut());
    let px = pm.take();

    let mut b = Vec::new();
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(side as i32).to_le_bytes());
    b.extend_from_slice(&(2 * side as i32).to_le_bytes()); // colours + mask
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(&[0; 24]);
    for y in (0..side).rev() {
        for x in 0..side {
            let i = ((y * side + x) * 4) as usize;
            let a = px[i + 3] as u32;
            // tiny-skia gives premultiplied colours, icons want them straight
            let c = |v: u8| if a == 0 { 0 } else { ((v as u32 * 255 + a / 2) / a).min(255) as u8 };
            b.extend_from_slice(&[c(px[i + 2]), c(px[i + 1]), c(px[i]), a as u8]);
        }
    }
    // the mask is unused with 32-bit colours but must be there: rows padded to 4 bytes
    let row = side.div_ceil(32) * 4;
    b.resize(b.len() + (row * side) as usize, 0);
    b
}
