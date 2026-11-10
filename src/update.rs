//! Looks on GitHub for a newer version and installs it: through the installer when Open Recovr was
//! installed, by replacing its own files when it runs from a folder (a USB stick), and elsewhere by
//! opening the download page.

use eframe::egui;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const REPO: &str = "jlerouge/open-recovr";
/// The key that signs each release's SHA256SUMS.txt (see keys/README.md). Its secret half never
/// goes to GitHub, so taking over the repository is not enough to push an update.
const SIGNING_KEY: &str = include_str!("../keys/open-recovr.pub");
pub const RELEASES_PAGE: &str = "https://github.com/jlerouge/open-recovr/releases/latest";

#[derive(Clone, PartialEq, Debug)]
pub enum State {
    /// Not looked yet.
    Unknown,
    Checking,
    UpToDate,
    /// A newer version, like "0.3.0".
    Available(String),
    /// Fetching it: the version and how much has come, from 0 to 1.
    Downloading(String, f32),
    /// The new version is starting: this one closes.
    Restart,
    /// Why it couldn't look or update, for a tooltip.
    Failed(String),
}

pub type Shared = Arc<Mutex<State>>;

pub fn shared() -> Shared {
    Arc::new(Mutex::new(State::Unknown))
}

static PRETEND: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// `--pretend-version x.y.z`: see [`current`].
pub fn pretend(v: String) {
    let _ = PRETEND.set(v);
}

/// The running version. A debug build can pretend to be older, to try an update for real
/// (`--pretend-version` or `OPEN_RECOVR_PRETEND_VERSION`).
pub fn current() -> String {
    if cfg!(debug_assertions) {
        if let Some(v) = PRETEND.get() {
            return v.clone();
        }
        if let Ok(v) = std::env::var("OPEN_RECOVR_PRETEND_VERSION") {
            return v;
        }
    }
    env!("CARGO_PKG_VERSION").to_string()
}

fn parse(v: &str) -> Option<(u32, u32, u32)> {
    let mut it = v.trim().trim_start_matches('v').split('.').map(|s| s.parse::<u32>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

/// `latest` comes after `current`; anything that isn't a plain x.y.z never does.
pub fn newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(a), Some(b)) if a > b)
}

/// How this copy gets its updates.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    /// Installed with the installer: the new installer replaces it.
    Installer,
    /// Unpacked from the zip: it replaces its own files.
    Portable,
    /// Other systems: the download page opens.
    Page,
}

pub fn kind() -> Kind {
    if !cfg!(all(windows, target_arch = "x86_64")) {
        return Kind::Page;
    }
    match std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        Some(dir) if dir.join("unins000.exe").exists() => Kind::Installer,
        Some(_) => Kind::Portable,
        None => Kind::Page,
    }
}

/// The file of the release this copy needs.
fn asset(kind: Kind, version: &str) -> Option<String> {
    match kind {
        Kind::Installer => Some(format!("open-recovr-v{version}-windows-x64-setup.exe")),
        Kind::Portable => Some(format!("open-recovr-v{version}-windows-x64.zip")),
        Kind::Page => None,
    }
}

/// curl, which ships with Windows 10 and later, macOS and nearly every Linux: no TLS library to carry.
fn curl() -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let mut c = Command::new(root.join("System32").join("curl.exe"));
        c.creation_flags(0x0800_0000); // no console window
        c
    }
    #[cfg(not(windows))]
    Command::new("curl")
}

fn fetch_text(url: &str) -> Result<String, String> {
    let out = curl()
        .args(["-fsSL", "--max-time", "20", "-A", "open-recovr-updater", "-H", "Accept: application/vnd.github+json", url])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

/// The text after `"key":` at or after `from`, with the position just past it.
fn json_value<'a>(json: &'a str, key: &str, from: usize) -> Option<(&'a str, usize)> {
    let k = format!("\"{key}\"");
    let start = from + json.get(from..)?.find(&k)? + k.len();
    let rest = json[start..].trim_start().strip_prefix(':')?.trim_start();
    let at = json.len() - rest.len();
    if let Some(s) = rest.strip_prefix('"') {
        let end = s.find('"')?;
        Some((&s[..end], at + end + 2))
    } else {
        let end = rest.find([',', '}', '\n']).unwrap_or(rest.len());
        Some((rest[..end].trim(), at + end))
    }
}

fn latest_tag(json: &str) -> Option<String> {
    json_value(json, "tag_name", 0).map(|(v, _)| v.trim_start_matches('v').to_string())
}

/// The size GitHub gives for a file of the release, to show the download's progress.
fn asset_size(json: &str, name: &str) -> Option<u64> {
    let mut from = 0;
    while let Some((v, next)) = json_value(json, "name", from) {
        if v == name {
            return json_value(json, "size", next).and_then(|(s, _)| s.parse().ok());
        }
        from = next;
    }
    None
}

/// Checks that `sums` was signed with `key` for `version`: the trusted comment, signed too,
/// names the version, so the checksums of one release can't pass for another's.
fn check_signature(sums: &str, minisig: &str, key: &str, version: &str) -> Result<(), String> {
    let key = minisign_verify::PublicKey::decode(key).map_err(|_| "this build has no signing key".to_string())?;
    let sig = minisign_verify::Signature::decode(minisig).map_err(|e| format!("SHA256SUMS.txt.minisig: {e}"))?;
    key.verify(sums.as_bytes(), &sig, false).map_err(|_| "SHA256SUMS.txt is not signed by the Open Recovr key".to_string())?;
    let tag = format!("v{version}");
    if !sig.trusted_comment().split_whitespace().any(|w| w == tag) {
        return Err(format!("the signature is not for {tag}"));
    }
    Ok(())
}

/// The SHA-256 written for `name` in the release's SHA256SUMS.txt.
fn expected_digest(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (hash, file) = l.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| hash.to_ascii_lowercase())
    })
}

fn digest(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn set(shared: &Shared, ctx: &egui::Context, s: State) {
    *shared.lock().unwrap() = s;
    ctx.request_repaint();
}

/// Asks GitHub for the latest version, in the background.
pub fn check(shared: Shared, ctx: egui::Context) {
    set(&shared, &ctx, State::Checking);
    std::thread::spawn(move || {
        let result = fetch_text(&format!("https://api.github.com/repos/{REPO}/releases/latest"))
            .and_then(|json| latest_tag(&json).ok_or_else(|| "no version in GitHub's answer".to_string()));
        let state = match result {
            Ok(v) if newer(&v, &current()) => State::Available(v),
            Ok(_) => State::UpToDate,
            Err(e) => State::Failed(e),
        };
        set(&shared, &ctx, state);
    });
}

/// Fetches `version`, checks it against the release's signed checksums, and starts it.
pub fn install(version: String, shared: Shared, ctx: egui::Context) {
    let kind = kind();
    let Some(name) = asset(kind, &version) else {
        ctx.open_url(egui::OpenUrl::new_tab(RELEASES_PAGE));
        return;
    };
    set(&shared, &ctx, State::Downloading(version.clone(), 0.0));
    std::thread::spawn(move || {
        let state = match fetch_and_start(kind, &version, &name, &shared, &ctx) {
            Ok(()) => State::Restart,
            Err(e) => State::Failed(e),
        };
        set(&shared, &ctx, state);
    });
}

fn fetch_and_start(kind: Kind, version: &str, name: &str, shared: &Shared, ctx: &egui::Context) -> Result<(), String> {
    let base = format!("https://github.com/{REPO}/releases/download/v{version}");
    let json = fetch_text(&format!("https://api.github.com/repos/{REPO}/releases/tags/v{version}"))?;
    let size = asset_size(&json, name);
    let sums = fetch_text(&format!("{base}/SHA256SUMS.txt"))?;
    let minisig = fetch_text(&format!("{base}/SHA256SUMS.txt.minisig")).map_err(|e| format!("SHA256SUMS.txt.minisig: {e}"))?;
    check_signature(&sums, &minisig, SIGNING_KEY, version)?;
    let want = expected_digest(&sums, name).ok_or_else(|| format!("{name} is not in SHA256SUMS.txt"))?;

    let dir = work_dir();
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(name);
    let mut child = curl()
        .args(["-fsSL", "--max-time", "1800", "-A", "open-recovr-updater", "-o"])
        .arg(&file)
        .arg(format!("{base}/{name}"))
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl: {e}"))?;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                let mut err = String::new();
                if let Some(mut e) = child.stderr.take() {
                    let _ = std::io::Read::read_to_string(&mut e, &mut err);
                }
                return Err(err.trim().to_string());
            }
            break;
        }
        if let (Some(total), Ok(m)) = (size, std::fs::metadata(&file)) {
            set(shared, ctx, State::Downloading(version.to_string(), (m.len() as f32 / total.max(1) as f32).min(1.0)));
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    if digest(&file).map_err(|e| e.to_string())? != want {
        let _ = std::fs::remove_file(&file);
        return Err(format!("{name}: the checksum doesn't match"));
    }

    match kind {
        // the installer closes nothing and asks nothing, then starts the new version
        Kind::Installer => Command::new(&file)
            .args(["/SP-", "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"])
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string()),
        Kind::Portable => {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            replace_in_place(&file, &exe)?;
            Command::new(&exe).spawn().map(|_| ()).map_err(|e| e.to_string())
        }
        Kind::Page => Ok(()),
    }
}

fn work_dir() -> PathBuf {
    std::env::temp_dir().join("open-recovr-update")
}

/// The program set aside during an update, removed at the next start.
fn set_aside(exe: &Path) -> PathBuf {
    exe.with_extension("exe.old")
}

/// Unpacks the release's zip over the folder holding `exe`. Windows lets a running program be
/// renamed but not overwritten: it is moved aside first, and put back if anything fails.
fn replace_in_place(zip_file: &Path, exe: &Path) -> Result<(), String> {
    let dir = exe.parent().ok_or("no folder")?;
    let old = set_aside(exe);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old).map_err(|e| format!("{}: {e}", dir.display()))?;
    let result = unpack(zip_file, dir);
    if result.is_err() && !exe.exists() {
        let _ = std::fs::rename(&old, exe);
    }
    result
}

fn unpack(zip_file: &Path, dir: &Path) -> Result<(), String> {
    let f = std::fs::File::open(zip_file).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(path) = entry.enclosed_name() else { continue };
        // the zip holds one folder named after the version: its content goes where the program is
        let rel: PathBuf = path.components().skip(1).collect();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = dir.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut w = std::fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            std::io::copy(&mut entry, &mut w).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Removes what the last update left: the old program and the downloaded files.
pub fn tidy() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(set_aside(&exe));
    }
    let _ = std::fs::remove_dir_all(work_dir());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("0.3.0", "0.2.2"));
        assert!(newer("v0.10.0", "0.9.9"));
        assert!(newer("1.0.0", "0.99.0"));
        assert!(!newer("0.2.2", "0.2.2"));
        assert!(!newer("0.2.1", "0.2.2"));
        assert!(!newer("0.3.0-beta", "0.2.2"));
        assert!(!newer("", "0.2.2"));
    }

    #[test]
    fn reads_githubs_answer() {
        let json = r#"{
  "url": "https://api.github.com/repos/x/y/releases/1",
  "tag_name": "v0.3.0",
  "name": "Open Recovr v0.3.0",
  "assets": [
    { "name": "open-recovr-v0.3.0-linux-x64.tar.gz", "uploader": { "login": "a", "id": 3 }, "size": 111 },
    { "name": "open-recovr-v0.3.0-windows-x64.zip", "label": "", "uploader": { "login": "a" }, "size": 4567890, "digest": "sha256:ab" }
  ]
}"#;
        assert_eq!(latest_tag(json).as_deref(), Some("0.3.0"));
        assert_eq!(asset_size(json, "open-recovr-v0.3.0-windows-x64.zip"), Some(4567890));
        assert_eq!(asset_size(json, "open-recovr-v0.3.0-linux-x64.tar.gz"), Some(111));
        assert_eq!(asset_size(json, "missing.zip"), None);
        let compact = r#"{"tag_name":"v1.2.3","assets":[{"name":"a.zip","size":42}]}"#;
        assert_eq!(latest_tag(compact).as_deref(), Some("1.2.3"));
        assert_eq!(asset_size(compact, "a.zip"), Some(42));
    }

    #[test]
    fn finds_the_checksum_of_a_file() {
        let sums = "AB12  open-recovr-v0.3.0-windows-x64.zip\ncd34 *open-recovr-v0.3.0-windows-x64-setup.exe\n";
        assert_eq!(expected_digest(sums, "open-recovr-v0.3.0-windows-x64.zip").as_deref(), Some("ab12"));
        assert_eq!(expected_digest(sums, "open-recovr-v0.3.0-windows-x64-setup.exe").as_deref(), Some("cd34"));
        assert_eq!(expected_digest(sums, "other"), None);
    }

    /// A key pair made for the test, and `sums` signed with it like scripts/sign-release.ps1 does.
    fn signed(sums: &str, comment: &str) -> (String, String) {
        let kp = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let sig = minisign::sign(Some(&kp.pk), &kp.sk, sums.as_bytes(), Some(comment), None).unwrap();
        (kp.pk.to_box().unwrap().into_string(), sig.into_string())
    }

    #[test]
    fn the_build_carries_a_signing_key() {
        assert!(minisign_verify::PublicKey::decode(SIGNING_KEY).is_ok(), "keys/open-recovr.pub holds no key");
    }

    #[test]
    fn checks_a_signature_made_by_the_minisign_tool() {
        // made by scripts/sign-release.ps1 with the real key, for checksums that match no file
        let sums = "aaaa  open-recovr-v0.2.2-windows-x64.zip\nbbbb  open-recovr-v0.2.2-windows-x64-setup.exe\n";
        let sig = "untrusted comment: signature from minisign secret key\n\
RURpu6g2vBvmlx+VUhfRaBycoUrbS/ahVPvtSJlSR96R5WbeN30w2OClobCZ2JfNMRa6hmfWx5pfdpygIRjNKMCS50BjuHmtAAE=\n\
trusted comment: open-recovr v0.2.2\n\
s58e8Fhm+VLesDZEX4NMy31FBB3BE7akNEBIG9YCZCmf1IbqqpilledRQmOZBbj9KniuTDC01boYpPqkhN6dCQ==\n";
        assert_eq!(check_signature(sums, sig, SIGNING_KEY, "0.2.2"), Ok(()));
        assert!(check_signature(sums, sig, SIGNING_KEY, "0.3.0").is_err());
    }

    #[test]
    fn only_signed_checksums_pass() {
        let sums = "ab12  open-recovr-v0.3.0-windows-x64.zip
";
        let (key, sig) = signed(sums, "open-recovr v0.3.0");
        assert_eq!(check_signature(sums, &sig, &key, "0.3.0"), Ok(()));
        // changed checksums
        assert!(
            check_signature(
                "ff12  open-recovr-v0.3.0-windows-x64.zip
",
                &sig,
                &key,
                "0.3.0"
            )
            .is_err()
        );
        // signed for another version
        assert!(check_signature(sums, &sig, &key, "0.4.0").is_err());
        // signed with another key
        let (other_key, other_sig) = signed(sums, "open-recovr v0.3.0");
        assert!(check_signature(sums, &other_sig, &key, "0.3.0").is_err());
        assert!(check_signature(sums, &sig, &other_key, "0.3.0").is_err());
        // a trusted comment edited by hand breaks the signature
        let edited = sig.replace("open-recovr v0.3.0", "open-recovr v0.4.0");
        assert!(check_signature(sums, &edited, &key, "0.4.0").is_err());
        // no signature, or no key in the build
        assert!(check_signature(sums, "", &key, "0.3.0").is_err());
        assert_eq!(
            check_signature(
                sums,
                &sig,
                "untrusted comment: placeholder
",
                "0.3.0"
            ),
            Err("this build has no signing key".into())
        );
    }

    #[test]
    fn sha256_of_a_file() {
        let p = std::env::temp_dir().join("open-recovr-digest-test.txt");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(digest(&p).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn unpacks_a_release_over_the_folder() {
        use std::io::Write;
        let root = std::env::temp_dir().join("open-recovr-unpack-test");
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("app");
        std::fs::create_dir_all(dir.join("vendor")).unwrap();
        std::fs::write(dir.join("open-recovr.exe"), b"old program").unwrap();
        std::fs::write(dir.join("vendor").join("keep.txt"), b"mine").unwrap();
        let zip_path = root.join("release.zip");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        z.add_directory("open-recovr-v9.0.0-windows-x64/", o).unwrap();
        z.start_file("open-recovr-v9.0.0-windows-x64/open-recovr.exe", o).unwrap();
        z.write_all(b"new program").unwrap();
        z.start_file("open-recovr-v9.0.0-windows-x64/vendor/photorec.txt", o).unwrap();
        z.write_all(b"pr").unwrap();
        z.finish().unwrap();

        let exe = dir.join("open-recovr.exe");
        replace_in_place(&zip_path, &exe).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new program");
        assert_eq!(std::fs::read(set_aside(&exe)).unwrap(), b"old program");
        assert_eq!(std::fs::read(dir.join("vendor").join("photorec.txt")).unwrap(), b"pr");
        assert_eq!(std::fs::read(dir.join("vendor").join("keep.txt")).unwrap(), b"mine");
        let _ = std::fs::remove_dir_all(&root);
    }
}
