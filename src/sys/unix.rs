//! The Linux and macOS side. It covers the basics (drives, rights, the bin,
//! free space) so the app runs there too; thumbnails and sound are not done yet.

use super::{Bus, Disk, Kind, Picture, TrashItem, classify, home_dir};
use std::fs::File;
use std::path::{Path, PathBuf};

fn statvfs(path: &Path) -> Option<libc::statvfs> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    (unsafe { libc::statvfs(c.as_ptr(), &mut st) } == 0).then_some(st)
}

/// Mount tables write a space as `\040`.
fn unescape_mount(s: &str) -> String {
    s.replace("\\040", " ").replace("\\011", "\t").replace("\\134", "\\")
}

/// (device, mount point, file system) of every mounted disk.
fn mounts() -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    if let Ok(t) = std::fs::read_to_string("/proc/mounts") {
        for line in t.lines() {
            let mut it = line.split(' ');
            if let (Some(d), Some(m), Some(f)) = (it.next(), it.next(), it.next()) {
                out.push((d.to_string(), unescape_mount(m), f.to_string()));
            }
        }
    } else if let Ok(o) = std::process::Command::new("mount").output() {
        // macOS: "/dev/disk3s1 on / (apfs, local, journaled)"
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let Some((d, rest)) = line.split_once(" on ") else { continue };
            let Some((m, opts)) = rest.rsplit_once(" (") else { continue };
            let fs = opts.split(',').next().unwrap_or("").trim_end_matches(')');
            out.push((d.to_string(), m.to_string(), fs.to_string()));
        }
    }
    out
}

/// Volume names, from /dev/disk/by-label on Linux.
fn label_of(device: &str, mount: &str) -> String {
    if let Ok(rd) = std::fs::read_dir("/dev/disk/by-label") {
        let dev = std::fs::canonicalize(device).ok();
        for e in rd.flatten() {
            if dev.is_some() && std::fs::canonicalize(e.path()).ok() == dev {
                return e.file_name().to_string_lossy().replace("\\x20", " ");
            }
        }
    }
    if mount.starts_with("/Volumes/") || mount.starts_with("/media/") || mount.starts_with("/run/media/") {
        return Path::new(mount).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    }
    String::new()
}

/// The whole disk a partition is on: sdb1 → sdb, nvme0n1p2 → nvme0n1, mmcblk0p1 → mmcblk0.
#[cfg(not(target_os = "macos"))]
fn whole_disk(device: &str) -> &str {
    let base = device.trim_start_matches("/dev/");
    if base.starts_with("nvme") || base.starts_with("mmcblk") {
        base.rsplit_once('p').map_or(base, |(a, _)| a)
    } else if base.starts_with("sr") {
        base
    } else {
        base.trim_end_matches(|c: char| c.is_ascii_digit())
    }
}

fn mounted_as_removable(mount: &str) -> bool {
    mount.starts_with("/media/") || mount.starts_with("/run/media/") || mount.starts_with("/Volumes/")
}

/// (kind, model, removable) from what Linux tells in /sys/block.
#[cfg(not(target_os = "macos"))]
fn drive_kind(device: &str, mount: &str, fs: &str) -> (Kind, String, bool) {
    let whole = whole_disk(device);
    let dir = format!("/sys/block/{whole}");
    let read = |f: &str| std::fs::read_to_string(format!("{dir}/{f}")).map(|s| s.trim().to_string()).unwrap_or_default();
    // the device's place in the machine: …/usb2/2-1/… for anything plugged in by USB
    let place = std::fs::canonicalize(&dir).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let removable = read("removable") == "1" || mounted_as_removable(mount);
    let bus = if whole.starts_with("nvme") {
        Bus::Nvme
    } else if whole.starts_with("mmcblk") {
        Bus::Sd
    } else if place.contains("/usb") {
        Bus::Usb
    } else if whole.starts_with("sd") {
        Bus::Sata
    } else {
        Bus::Other
    };
    let ssd = match read("queue/rotational").as_str() {
        "0" => Some(true),
        "1" => Some(false),
        _ => None,
    };
    let model = format!("{} {}", read("device/vendor"), read("device/model")).trim().to_string();
    let optical = whole.starts_with("sr") || matches!(fs, "iso9660" | "udf");
    (classify(bus, ssd, removable, optical, &model), model, removable)
}

/// (kind, model, removable) from `diskutil info`.
#[cfg(target_os = "macos")]
fn drive_kind(device: &str, mount: &str, fs: &str) -> (Kind, String, bool) {
    let text = std::process::Command::new("diskutil")
        .args(["info", device])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(name).and_then(|r| r.strip_prefix(':')).map(|v| v.trim().to_string()))
            .unwrap_or_default()
    };
    let protocol = field("Protocol");
    let bus = match protocol.as_str() {
        "PCI-Express" | "Apple Fabric" | "PCI" => Bus::Nvme,
        "SATA" => Bus::Sata,
        "USB" | "Thunderbolt" => Bus::Usb,
        "Secure Digital" => Bus::Sd,
        _ => Bus::Other,
    };
    let ssd = match field("Solid State").as_str() {
        "Yes" => Some(true),
        "No" => Some(false),
        _ => None,
    };
    let removable = field("Removable Media") == "Removable" || field("Ejectable") == "Yes" || mounted_as_removable(mount);
    let model = field("Device / Media Name");
    let optical = !field("Optical Drive Type").is_empty() || matches!(fs, "cd9660" | "udf");
    (classify(bus, ssd, removable, optical, &model), model, removable)
}

/// Phones and cameras the desktop has opened (GNOME and friends put them under gvfs).
fn phones() -> Vec<String> {
    let uid = unsafe { libc::getuid() };
    let Ok(rd) = std::fs::read_dir(format!("/run/user/{uid}/gvfs")) else { return Vec::new() };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        let Some(host) = n.strip_prefix("mtp:host=").or_else(|| n.strip_prefix("gphoto2:host=")) else { continue };
        // "SAMSUNG_SAMSUNG_Android_R58M12345" → "SAMSUNG Android"
        let decoded = percent_decode(host);
        let mut words: Vec<&str> = decoded.split(['_', ' ']).filter(|w| !w.is_empty()).collect();
        if words.len() > 1 && words.last().is_some_and(|w| w.len() >= 8 && w.chars().any(|c| c.is_ascii_digit())) {
            words.pop();
        }
        words.dedup();
        let name = words.join(" ");
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// The language the desktop is set to ("fr_FR.UTF-8"…).
pub fn ui_language() -> Option<String> {
    for k in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(v) = std::env::var(k)
            && !v.is_empty()
            && v != "C"
            && v != "POSIX"
        {
            return Some(v);
        }
    }
    if cfg!(target_os = "macos")
        && let Ok(o) = std::process::Command::new("defaults").args(["read", "-g", "AppleLocale"]).output()
    {
        let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    None
}

// statvfs field widths differ between Linux (u64) and macOS (u32): the casts are needed there.
#[allow(clippy::unnecessary_cast)]
pub fn list_disks() -> Vec<Disk> {
    let mut out: Vec<Disk> = Vec::new();
    for (device, mount, fs) in mounts() {
        if !device.starts_with("/dev/") || device.starts_with("/dev/loop") {
            continue;
        }
        if matches!(fs.as_str(), "squashfs" | "devfs" | "autofs") || mount.starts_with("/System/Volumes/") {
            continue;
        }
        if mount.starts_with("/boot") || mount.starts_with("/snap/") {
            continue;
        }
        if let Some(prev) = out.iter_mut().find(|d| d.device == device) {
            // the same partition mounted twice: keep the shorter path
            if mount.len() < prev.id.len() {
                prev.id = mount.clone();
                prev.root = PathBuf::from(&mount);
            }
            continue;
        }
        let Some(st) = statvfs(Path::new(&mount)) else { continue };
        let block = (st.f_frsize as u64).max(512);
        let (kind, model, removable) = drive_kind(&device, &mount, &fs);
        out.push(Disk {
            id: mount.clone(),
            root: PathBuf::from(&mount),
            label: label_of(&device, &mount),
            removable,
            device,
            fs: fs.to_uppercase(),
            total: st.f_blocks as u64 * block,
            free: st.f_bavail as u64 * block,
            cluster: block,
            kind,
            model,
        });
    }
    for (n, name) in phones().into_iter().enumerate() {
        out.push(Disk {
            id: format!("phone:{}", n + 1),
            root: PathBuf::new(),
            device: String::new(),
            label: name,
            fs: String::new(),
            total: 0,
            free: 0,
            removable: true,
            cluster: 512,
            kind: Kind::Phone,
            model: String::new(),
        });
    }
    out
}

pub fn is_admin() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Starts this program again as root (pkexec on Linux, the password prompt on macOS).
pub fn relaunch_as_admin() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let started = if cfg!(target_os = "macos") {
        let script = format!("do shell script quoted form of \"{}\" with administrator privileges", exe.display());
        std::process::Command::new("osascript").args(["-e", &script]).spawn()
    } else {
        // pkexec clears the environment: hand the display over explicitly
        let mut c = std::process::Command::new("pkexec");
        c.arg("env");
        for k in ["DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY", "XDG_RUNTIME_DIR"] {
            if let Ok(v) = std::env::var(k) {
                c.arg(format!("{k}={v}"));
            }
        }
        c.arg(exe).spawn()
    };
    started.is_ok()
}

/// Not read on these systems yet: the built-in search then goes through the whole disk.
pub fn volume_bitmap(_file: &File) -> Option<Vec<u8>> {
    None
}

#[allow(clippy::unnecessary_cast)] // see list_disks
pub fn free_space(path: &Path) -> Option<u64> {
    let mut p = path;
    // the scratch folder may not exist yet: ask about its parent
    while !p.exists() {
        p = p.parent()?;
    }
    statvfs(p).map(|st| st.f_bavail as u64 * (st.f_frsize as u64).max(512))
}

/// `%20` and friends in a .trashinfo path.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// "2024-05-17T10:42:03" to seconds since 1970 (local time taken as UTC: close enough for sorting).
fn parse_deletion_date(s: &str) -> u64 {
    let n = |a: usize, b: usize| s.get(a..b).and_then(|t| t.parse::<i64>().ok());
    let (Some(y), Some(m), Some(d)) = (n(0, 4), n(5, 7), n(8, 10)) else { return 0 };
    let (hh, mm, ss) = (n(11, 13).unwrap_or(0), n(14, 16).unwrap_or(0), n(17, 19).unwrap_or(0));
    // days from civil (H. Hinnant)
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    (days * 86_400 + hh * 3600 + mm * 60 + ss).max(0) as u64
}

/// Freedesktop bins (`files/` + `info/*.trashinfo`), and macOS' `.Trash` / `.Trashes`.
pub fn trash_items(disk: &Disk) -> Vec<TrashItem> {
    let uid = unsafe { libc::getuid() };
    let mut bins: Vec<PathBuf> = Vec::new();
    let home = home_dir();
    let on_this_disk = |p: &Path| super::disk_of(p, std::slice::from_ref(disk)).is_some();
    if let Some(h) = &home {
        let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| h.join(".local/share"));
        bins.push(data.join("Trash"));
        bins.push(h.join(".Trash"));
    }
    bins.push(disk.root.join(format!(".Trash-{uid}")));
    bins.push(disk.root.join(".Trash").join(uid.to_string()));
    bins.push(disk.root.join(".Trashes").join(uid.to_string()));
    let mut out = Vec::new();
    for bin in bins {
        if !on_this_disk(&bin) || !bin.is_dir() {
            continue;
        }
        let files = bin.join("files");
        if files.is_dir() {
            let Ok(rd) = std::fs::read_dir(&files) else { continue };
            for e in rd.flatten() {
                let name = e.file_name();
                let info = bin.join("info").join(format!("{}.trashinfo", name.to_string_lossy()));
                let text = std::fs::read_to_string(info).unwrap_or_default();
                let mut original = PathBuf::from(&name);
                let mut deleted = 0;
                for l in text.lines() {
                    if let Some(p) = l.strip_prefix("Path=") {
                        original = PathBuf::from(percent_decode(p));
                    } else if let Some(d) = l.strip_prefix("DeletionDate=") {
                        deleted = parse_deletion_date(d);
                    }
                }
                out.push(TrashItem { data: e.path(), original, deleted, size: 0 });
            }
        } else if let Ok(rd) = std::fs::read_dir(&bin) {
            // macOS keeps no record of where things came from
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let deleted = e.metadata().ok().and_then(|m| m.modified().ok()).map(crate::engine::unix_secs).unwrap_or(0);
                out.push(TrashItem { data: e.path(), original: PathBuf::from(e.file_name()), deleted, size: 0 });
            }
        }
    }
    out
}

pub fn thumbnail(_path: &Path, _size: i32) -> Option<Picture> {
    None
}

/// Sound playback is not done on these systems yet: the "Open" button is offered instead.
#[derive(Default)]
pub struct Player;

impl Player {
    pub const AVAILABLE: bool = false;

    pub fn play(&mut self, _path: &Path) -> bool {
        false
    }

    pub fn stop(&mut self) {}

    pub fn playing(&mut self) -> bool {
        false
    }
}

/// No icon by the clock on these systems yet: the window is minimized instead.
pub struct Tray;

impl Tray {
    pub const AVAILABLE: bool = false;

    pub fn start(_window: isize, _icon: &[u8], _wake: impl Fn() + Send + Sync + 'static) -> Option<Tray> {
        None
    }

    pub fn hide_window(&self) -> bool {
        false
    }

    pub fn is_hidden(&self) -> bool {
        false
    }

    pub fn set_status(&self, _text: &str) {}

    pub fn notify(&self, _title: &str, _text: &str) {}

    pub fn take_quit(&self) -> bool {
        false
    }
}
