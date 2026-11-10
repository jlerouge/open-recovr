//! Everything that talks to the operating system. What is the same everywhere lives
//! here; the Windows calls are in `sys/windows.rs`, Linux and macOS in `sys/unix.rs`.

use std::path::{Path, PathBuf};

#[cfg(windows)]
#[path = "sys/windows.rs"]
mod os;
#[cfg(unix)]
#[path = "sys/unix.rs"]
mod os;

pub use os::{Player, Tray, free_space, is_admin, list_disks, relaunch_as_admin, thumbnail, trash_items, ui_language, volume_bitmap};

/// What sort of drive it is, for its picture and its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Hdd,
    /// a solid-state drive whose connection isn't known
    Ssd,
    SataSsd,
    Nvme,
    Optical,
    SdCard,
    MicroSd,
    CompactFlash,
    UsbStick,
    UsbDrive,
    /// a phone (or camera) plugged in by USB: listed, but its memory can't be read raw
    Phone,
    /// a disk image file (.img, .dd…) made with ddrescue, dd or a cloning tool
    Image,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Hdd => t!("Disque dur"),
            Kind::Ssd => t!("SSD"),
            Kind::SataSsd => t!("SSD SATA"),
            Kind::Nvme => t!("SSD NVMe"),
            Kind::Optical => t!("CD / DVD"),
            Kind::SdCard => t!("Carte SD"),
            Kind::MicroSd => t!("Carte microSD"),
            Kind::CompactFlash => t!("Carte CompactFlash"),
            Kind::UsbStick => t!("Clé USB"),
            Kind::UsbDrive => t!("Disque externe USB"),
            Kind::Phone => t!("Téléphone"),
            Kind::Image => t!("Image disque"),
        }
    }

    /// The name of its picture in `assets/`.
    pub fn art(self) -> &'static str {
        match self {
            Kind::Hdd => "hdd",
            Kind::Ssd | Kind::SataSsd => "ssd",
            Kind::Nvme => "nvme",
            Kind::Optical => "optical",
            Kind::SdCard => "sd",
            Kind::MicroSd => "microsd",
            Kind::CompactFlash => "cf",
            Kind::UsbStick => "usb",
            Kind::UsbDrive => "external",
            Kind::Phone => "phone",
            Kind::Image => "image",
        }
    }

    /// Inside the computer, not plugged in.
    pub fn internal(self) -> bool {
        matches!(self, Kind::Hdd | Kind::Ssd | Kind::SataSsd | Kind::Nvme)
    }
}

/// How a drive is connected, as the system reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bus {
    /// older IDE / ATA (only Windows tells it apart from SATA)
    #[cfg_attr(not(windows), allow(dead_code))]
    Ata,
    Sata,
    Nvme,
    Usb,
    /// a built-in SD / MMC reader
    Sd,
    Other,
}

/// The kind of drive from what the system says about it. `model` is the maker's name for
/// the device ("Generic STORAGE DEVICE", "SDXC Card Reader"…), which often tells a card
/// reader apart from a USB stick.
pub fn classify(bus: Bus, ssd: Option<bool>, removable: bool, optical: bool, model: &str) -> Kind {
    if optical {
        return Kind::Optical;
    }
    let up = model.to_uppercase();
    let words: Vec<&str> = up.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let has = |w: &str| words.contains(&w);
    let card = if has("MICROSD") || has("TF") || up.contains("MICRO SD") {
        Some(Kind::MicroSd)
    } else if has("CF") || has("COMPACTFLASH") || up.contains("COMPACT FLASH") {
        Some(Kind::CompactFlash)
    } else if ["SD", "SDHC", "SDXC", "MMC", "READER", "CARD", "MS", "XD"].iter().any(|w| has(w)) {
        Some(Kind::SdCard)
    } else {
        None
    };
    match bus {
        Bus::Nvme => Kind::Nvme,
        Bus::Sd => {
            if card == Some(Kind::MicroSd) {
                Kind::MicroSd
            } else {
                Kind::SdCard
            }
        }
        Bus::Usb => card.unwrap_or(if removable { Kind::UsbStick } else { Kind::UsbDrive }),
        _ if removable && card.is_some() => card.unwrap_or(Kind::SdCard),
        Bus::Ata | Bus::Sata => {
            if ssd == Some(true) {
                Kind::SataSsd
            } else {
                Kind::Hdd
            }
        }
        Bus::Other => match ssd {
            Some(true) => Kind::Ssd,
            _ if removable => Kind::UsbStick,
            _ => Kind::Hdd,
        },
    }
}

#[derive(Clone, Debug)]
pub struct Disk {
    /// What the person knows the drive by: `C:` on Windows, the mount point elsewhere.
    pub id: String,
    /// Where its files start (`C:\`, `/media/usb`); empty for a phone.
    pub root: PathBuf,
    /// The raw device read by the deep search (`\\.\C:`, `/dev/sdb1`).
    pub device: String,
    pub label: String,
    pub fs: String,
    pub total: u64,
    pub free: u64,
    pub removable: bool,
    pub cluster: u64,
    pub kind: Kind,
    /// The maker's name for the device, when known.
    pub model: String,
}

impl Disk {
    pub fn title(&self) -> String {
        if matches!(self.kind, Kind::Phone | Kind::Image) {
            return self.label.clone();
        }
        let label = self.label.trim();
        if !label.is_empty() {
            format!("{label} ({})", self.id)
        } else if self.kind.internal() {
            t!("Disque local ({})", self.id)
        } else {
            format!("{} ({})", self.kind.label(), self.id)
        }
    }

    /// Whether its memory can be searched (a phone's can't, through USB).
    pub fn searchable(&self) -> bool {
        self.kind != Kind::Phone
    }

    /// Whether files can be written there (not on a phone, a CD or a DVD).
    pub fn writable(&self) -> bool {
        !matches!(self.kind, Kind::Phone | Kind::Optical | Kind::Image)
    }
}

/// Something the system's own bin still holds.
pub struct TrashItem {
    pub data: PathBuf,
    pub original: PathBuf,
    /// Seconds since 1970, 0 when unknown.
    pub deleted: u64,
    /// Size as noted when it was deleted, 0 when unknown.
    pub size: u64,
}

/// Bytes, pixels left to right and top to bottom, 4 bytes (RGBA) each.
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// What the system calls the rights needed for the deep search.
pub fn admin_word() -> &'static str {
    if cfg!(windows) { t!("administrateur") } else { t!("superutilisateur (root)") }
}

/// The disk holding `path`: the one with the longest matching root.
pub fn disk_of<'a>(path: &Path, disks: &'a [Disk]) -> Option<&'a Disk> {
    let p = path.to_string_lossy().to_lowercase();
    disks
        .iter()
        .filter(|d| {
            let r = d.root.to_string_lossy().to_lowercase();
            !r.is_empty() && p.starts_with(&r) && (r.ends_with(['/', '\\']) || p.len() == r.len() || p[r.len()..].starts_with(['/', '\\']))
        })
        .max_by_key(|d| d.root.as_os_str().len())
}

/// A disk image file (made with ddrescue, dd, a cloning tool…) listed like a drive: no
/// administrator rights needed to read it, and the damaged drive is left alone.
pub fn open_image(path: &Path) -> Option<Disk> {
    use std::io::Read;
    let meta = std::fs::metadata(path).ok().filter(|m| m.is_file() && m.len() >= 512)?;
    let mut boot = [0u8; 512];
    std::fs::File::open(path).ok()?.read_exact(&mut boot).ok()?;
    let (fs, cluster) = image_fs(&boot);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Some(Disk {
        id: path.to_string_lossy().into_owned(),
        root: PathBuf::new(),
        device: path.to_string_lossy().into_owned(),
        label: name,
        fs: fs.into(),
        total: meta.len(),
        free: 0,
        removable: false,
        cluster,
        kind: Kind::Image,
        model: String::new(),
    })
}

/// The file system of an image holding a single volume, from its first sector (nothing
/// for the image of a whole disk, read from end to end).
fn image_fs(b: &[u8; 512]) -> (&'static str, u64) {
    let cluster = u16::from_le_bytes([b[11], b[12]]) as u64 * b[13] as u64;
    let cluster = if cluster.is_power_of_two() && (512..=2 << 20).contains(&cluster) { cluster } else { 512 };
    if &b[3..11] == b"NTFS    " {
        ("NTFS", cluster)
    } else if &b[3..11] == b"EXFAT   " {
        ("exFAT", 512)
    } else if &b[82..90] == b"FAT32   " {
        ("FAT32", cluster)
    } else if &b[54..59] == b"FAT16" || &b[54..59] == b"FAT12" {
        ("FAT", cluster)
    } else {
        ("", 512)
    }
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// A scratch folder for the deep search, never on the drive being searched:
/// writing there could overwrite the very files we are looking for.
pub fn pick_stage(scanned: &Disk, disks: &[Disk]) -> Option<PathBuf> {
    const MIN_FREE: u64 = 2 << 30;
    let tag = format!("Open Recovr - analyse {}", std::process::id());
    let temp = std::env::temp_dir();
    if disk_of(&temp, disks).is_some_and(|d| d.id != scanned.id) && free_space(&temp).is_some_and(|f| f > MIN_FREE) {
        return Some(temp.join(tag));
    }
    disks
        .iter()
        .filter(|d| d.id != scanned.id && d.writable() && d.free > MIN_FREE)
        .max_by_key(|d| (!d.removable, d.free))
        .map(|d| d.root.join(tag))
}

/// The PhotoRec engine: the copy shipped next to the app first, then an installed one.
pub fn find_photorec() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["photorec_win.exe", "photorec.exe"] } else { &["photorec"] };
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1).take(4) {
            roots.push(dir.to_path_buf());
            roots.push(dir.join("vendor"));
        }
    }
    roots.push(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor")));
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if cfg!(unix) {
        // often installed where only administrators look
        dirs.extend(["/usr/sbin", "/usr/bin", "/usr/local/bin", "/opt/homebrew/bin", "/sbin"].map(PathBuf::from));
    }
    for dir in dirs {
        for n in names {
            let p = dir.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    if cfg!(windows) {
        roots.extend([r"C:\Program Files", r"C:\Program Files (x86)", r"C:\"].map(PathBuf::from));
    }
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if !name.starts_with("testdisk") {
                continue;
            }
            for n in names {
                let p = e.path().join(n);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Opens a folder in the file manager, or a file in the program that usually opens it.
pub fn open_path(path: &Path) {
    let tool = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(tool).arg(path).spawn();
}

/// Where the app keeps its few settings.
pub fn settings_file() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home_dir().map(|h| h.join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home_dir().map(|h| h.join(".config")))
    };
    base.map(|b| b.join("Open Recovr").join("reglages.txt"))
}

/// The journal of the last search, kept next to the settings, to send along with a bug report.
pub fn log_file() -> Option<PathBuf> {
    settings_file().map(|p| p.with_file_name("journal.txt"))
}

/// Fonts for the interface: the system's own when they can be found.
pub fn ui_fonts() -> (Vec<PathBuf>, Vec<PathBuf>) {
    if cfg!(windows) {
        let f = std::env::var("WINDIR").map(|w| PathBuf::from(w).join("Fonts")).unwrap_or_default();
        (vec![f.join("segoeui.ttf")], vec![f.join("seguisb.ttf"), f.join("segoeuib.ttf")])
    } else if cfg!(target_os = "macos") {
        (
            vec![PathBuf::from("/System/Library/Fonts/Supplemental/Arial.ttf")],
            vec![PathBuf::from("/System/Library/Fonts/Supplemental/Arial Bold.ttf")],
        )
    } else {
        let p = |s: &str| PathBuf::from(s);
        (
            vec![
                p("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf"),
                p("/usr/share/fonts/noto/NotoSans-Regular.ttf"),
                p("/usr/share/fonts/google-noto/NotoSans-Regular.ttf"),
                p("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
                p("/usr/share/fonts/TTF/DejaVuSans.ttf"),
            ],
            vec![
                p("/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf"),
                p("/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf"),
                p("/usr/share/fonts/noto/NotoSans-Bold.ttf"),
                p("/usr/share/fonts/google-noto/NotoSans-Bold.ttf"),
                p("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
                p("/usr/share/fonts/TTF/DejaVuSans-Bold.ttf"),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_of_drive() {
        assert_eq!(classify(Bus::Nvme, Some(true), false, false, "Samsung SSD 980 PRO"), Kind::Nvme);
        assert_eq!(classify(Bus::Sata, Some(true), false, false, "CT500MX500SSD1"), Kind::SataSsd);
        assert_eq!(classify(Bus::Sata, Some(false), false, false, "WDC WD20EZRZ"), Kind::Hdd);
        assert_eq!(classify(Bus::Ata, None, false, false, ""), Kind::Hdd);
        assert_eq!(classify(Bus::Other, Some(true), false, false, "Msft Virtual Disk"), Kind::Ssd);
        assert_eq!(classify(Bus::Usb, None, true, false, "SanDisk Cruzer Blade"), Kind::UsbStick);
        assert_eq!(classify(Bus::Usb, None, false, false, "WD Elements 25A2"), Kind::UsbDrive);
        assert_eq!(classify(Bus::Usb, None, true, false, "Generic SD/MMC"), Kind::SdCard);
        assert_eq!(classify(Bus::Usb, None, true, false, "Mass Storage Device MicroSD"), Kind::MicroSd);
        assert_eq!(classify(Bus::Usb, None, true, false, "Generic- Compact Flash"), Kind::CompactFlash);
        assert_eq!(classify(Bus::Usb, None, true, false, "Generic-CF"), Kind::CompactFlash);
        assert_eq!(classify(Bus::Sd, None, true, false, "SD64G"), Kind::SdCard);
        assert_eq!(classify(Bus::Usb, None, true, true, "HL-DT-ST DVDRAM"), Kind::Optical);
        // "MS" is a card word, but not inside a longer one
        assert_eq!(classify(Bus::Usb, None, true, false, "Kingston DataTraveler MSFT"), Kind::UsbStick);
    }

    #[test]
    fn phones_have_no_folder() {
        let phone = Disk {
            id: "phone:1".into(),
            root: PathBuf::new(),
            device: String::new(),
            label: "Pixel 7".into(),
            fs: String::new(),
            total: 0,
            free: 0,
            removable: true,
            cluster: 512,
            kind: Kind::Phone,
            model: String::new(),
        };
        let disks = [phone];
        assert!(disk_of(Path::new("C:\\Users"), &disks).is_none());
        assert!(disk_of(Path::new("/home"), &disks).is_none());
        assert_eq!(disks[0].title(), "Pixel 7");
        assert!(!disks[0].searchable());
    }
}
