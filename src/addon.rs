//! PhotoRec as an optional add-on. Open Recovr does not ship it: on Windows, when the user asks
//! for it, the official archive is fetched from cgsecurity.org, the few files the classic engine
//! needs are kept in the user's data folder, and every one of them is checked against a known
//! SHA-256, so a changed upstream archive is refused instead of run. Elsewhere, PhotoRec comes
//! from the system's own packages (testdisk).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub const URL: &str = "https://www.cgsecurity.org/testdisk-7.2.win64.zip";

/// path inside the kept folder -> SHA-256 (the same list as scripts/fetch-photorec.ps1)
const EXPECTED: &[(&str, &str)] = &[
    ("photorec_win.exe", "d75b9a6ff60f91479085a578aca518abb1cd7405f24506a61afa2f7e222147bf"),
    ("cygewf-2.dll", "e904b5a3f55572fec2d08d222a6c1a54dd768bbb72e666f8eebffa0c68c19214"),
    ("cyggcc_s-seh-1.dll", "38425ee608500b6808bcfe4569ea7f6387a436e8b779979c9c3c045201b87260"),
    ("cygiconv-2.dll", "b5674bb002302ab874521499ca07edc30ace5a6beded599591743a2d9656a9cf"),
    ("cygjpeg-8.dll", "49c02ebeeb3d02ae5ef7760aba80b07f070e27100b7ac48fd5cd3fc77323df04"),
    ("cygncursesw-10.dll", "b2f0f7b56356244a4530934d95de622c58772b66bd356963ab345741a21d41a9"),
    ("cygwin1.dll", "52062be7ef8f14445a81a5dff3b3ac3c7b74466159fffd542b22a83af8ac6b86"),
    ("cygz.dll", "78d499f1d956edf00dd988c95f7f76174425c0fb47e3ddb56ba6af447f18f4f1"),
    ("63/cygwin", "1a9b75481dba09d8f73b9b13f39d52db026bfc7ae8eb4de1d250388756445dbc"),
    ("COPYING.txt", "9f2e250993c6206fac643824e05e5a0d7d3e0895d9e09a5ce4b12bc2610afc11"),
    ("AUTHORS.txt", "24f8f773d2f79b50b15acb86c2960a63eb87236bb9498358540289a116688c90"),
    ("THANKS.txt", "12b2bd649cdb8df8a649ffed716af77570982194b96a1fffabc18d830153ccc5"),
    ("NEWS.txt", "6cd6b0d6c2ac3a413ccba78533105b3987df50017569dc0481fa72311c9432d8"),
    ("VERSION.txt", "81bdb4615fad4dd3953406c3505ed2bea8cd550274bd16cbe9576f45a786a385"),
    ("INFO", "45c3a355cd86b60f4647c715dc9c6a5fd8216047416f04b32cf273b1216ed442"),
    ("readme.txt", "68af43175eeba5e1104074600f807492391f2928da944786ea1acc507dc5ce02"),
];

#[derive(Clone, PartialEq)]
pub enum State {
    Idle,
    Working,
    Done(PathBuf),
    Failed(String),
}

/// Where the add-on lives: next to the settings, in the user's own data folder.
pub fn folder() -> Option<PathBuf> {
    // not in the roaming profile on Windows: it is a program, not a setting
    let local = std::env::var_os("LOCALAPPDATA").filter(|_| cfg!(windows)).map(|d| PathBuf::from(d).join("Open Recovr"));
    local.or_else(|| crate::sys::settings_file().and_then(|p| p.parent().map(PathBuf::from))).map(|d| d.join("testdisk-7.2"))
}

/// Can the add-on be fetched by Open Recovr here? Elsewhere the system's packages provide it.
pub fn installable() -> bool {
    cfg!(windows)
}

/// Fetches, checks and keeps PhotoRec, in the background.
pub fn install(shared: Arc<Mutex<State>>, ctx: eframe::egui::Context) {
    *shared.lock().unwrap() = State::Working;
    std::thread::spawn(move || {
        let r = run();
        *shared.lock().unwrap() = match r {
            Ok(p) => State::Done(p),
            Err(e) => State::Failed(e),
        };
        ctx.request_repaint();
    });
}

fn run() -> Result<PathBuf, String> {
    let dest = folder().ok_or("no data folder")?;
    let work = std::env::temp_dir().join(format!("open-recovr-photorec-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let zip_path = work.join("testdisk.zip");
    let r = (|| {
        crate::update::download(URL, &zip_path)?;
        let file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        unpack(&mut zip, &dest)
    })();
    let _ = std::fs::remove_dir_all(&work);
    if r.is_err() {
        let _ = std::fs::remove_dir_all(&dest);
    }
    r
}

/// Keeps the expected files of the archive, wherever they sit in it, and checks each one.
fn unpack<R: std::io::Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, dest: &std::path::Path) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    let names: Vec<String> = zip.file_names().filter_map(|n| n.ok().map(|n| n.to_string())).collect();
    let _ = std::fs::remove_dir_all(dest);
    for (rel, sum) in EXPECTED {
        let tail = format!("/{rel}");
        let name = names
            .iter()
            .find(|n| n.replace('\\', "/").ends_with(&tail) || n.as_str() == *rel)
            .ok_or_else(|| format!("{rel} is not in the archive"))?;
        let mut entry = zip.by_name(name).map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut data).map_err(|e| e.to_string())?;
        let got: String = Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect();
        if got != *sum {
            return Err(format!("{rel}: unexpected SHA-256, the archive on cgsecurity.org changed"));
        }
        let to = dest.join(rel);
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&to, &data).map_err(|e| e.to_string())?;
    }
    Ok(dest.join("photorec_win.exe"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The files fetched for development (scripts/fetch-photorec.ps1), zipped as cgsecurity does,
    /// come out whole; skipped when they were not fetched.
    #[test]
    fn the_official_files_are_kept() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/testdisk-7.2");
        if !src.join("photorec_win.exe").is_file() {
            return;
        }
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("testdisk-7.2/fidentify_win.exe", o).unwrap();
            for (rel, _) in EXPECTED {
                let inside =
                    if *rel == "63/cygwin" { "testdisk-7.2/terminfo/63/cygwin".to_string() } else { format!("testdisk-7.2/{rel}") };
                w.start_file(inside, o).unwrap();
                std::io::Write::write_all(&mut w, &std::fs::read(src.join(rel)).unwrap()).unwrap();
            }
            w.finish().unwrap();
        }
        let dest = std::env::temp_dir().join(format!("open-recovr-addon-ok-{}", std::process::id()));
        let mut zip = zip::ZipArchive::new(buf).unwrap();
        let exe = unpack(&mut zip, &dest).unwrap();
        assert!(exe.is_file() && dest.join("63/cygwin").is_file() && !dest.join("fidentify_win.exe").exists());
        let _ = std::fs::remove_dir_all(&dest);
    }

    /// The real thing: fetches the official archive from cgsecurity.org (27 MB) into the data
    /// folder, and the classic engine then finds it there. Run by hand, with LOCALAPPDATA pointing
    /// at a scratch folder: `cargo test real_download -- --ignored`.
    #[test]
    #[ignore]
    fn real_download() {
        let exe = run().unwrap();
        assert!(exe.is_file(), "{}", exe.display());
        assert_eq!(crate::sys::find_photorec(), Some(exe));
    }

    #[test]
    fn a_tampered_archive_is_refused() {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("testdisk-7.2/photorec_win.exe", o).unwrap();
            std::io::Write::write_all(&mut w, b"not photorec").unwrap();
            w.finish().unwrap();
        }
        let dest = std::env::temp_dir().join(format!("open-recovr-addon-test-{}", std::process::id()));
        let mut zip = zip::ZipArchive::new(buf).unwrap();
        let err = unpack(&mut zip, &dest).unwrap_err();
        assert!(err.contains("SHA-256"), "{err}");
        let _ = std::fs::remove_dir_all(&dest);
    }
}
