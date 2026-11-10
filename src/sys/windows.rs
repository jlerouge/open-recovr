//! The Windows side: drives, administrator rights, the Recycle Bin, the free-space
//! map of a volume, Explorer's thumbnails and sound playback.

use super::{Bus, Disk, Kind, Picture, TrashItem, classify};
use std::ffi::c_void;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// What the drive behind `X:` says about itself: (connection, solid-state?, removable medium?, model).
fn storage_info(letter: char) -> (Bus, Option<bool>, bool, String) {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Ioctl::IOCTL_STORAGE_QUERY_PROPERTY;
    let mut info = (Bus::Other, None, false, String::new());
    let path = wide(&format!(r"\\.\{letter}:"));
    // no read or write rights asked: only questions about the device
    let h = unsafe { CreateFileW(path.as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE, null(), OPEN_EXISTING, 0, null_mut()) };
    if h == INVALID_HANDLE_VALUE {
        return info;
    }
    let query = |property: u32, out: &mut [u8]| -> bool {
        let q = [property, 0u32, 0u32]; // the property, "standard query", nothing else
        let mut got = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                h,
                IOCTL_STORAGE_QUERY_PROPERTY,
                q.as_ptr() as _,
                12,
                out.as_mut_ptr() as _,
                out.len() as u32,
                &mut got,
                null_mut(),
            )
        };
        ok != 0 && got >= 9
    };
    let mut d = vec![0u8; 1024];
    if query(0, &mut d) {
        // STORAGE_DEVICE_DESCRIPTOR
        let u32_at = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap()) as usize;
        let text_at = |o: usize| -> String {
            if o == 0 || o >= d.len() {
                return String::new();
            }
            let end = d[o..].iter().position(|&b| b == 0).map_or(d.len(), |e| o + e);
            String::from_utf8_lossy(&d[o..end]).trim().to_string()
        };
        info.2 = d[10] != 0;
        info.3 = [text_at(u32_at(12)), text_at(u32_at(16))].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ");
        info.0 = match u32_at(28) {
            3 => Bus::Ata,
            7 => Bus::Usb,
            11 => Bus::Sata,
            12 | 13 => Bus::Sd,
            17 => Bus::Nvme,
            _ => Bus::Other,
        };
    }
    let mut p = [0u8; 16];
    if query(7, &mut p) {
        // DEVICE_SEEK_PENALTY_DESCRIPTOR: spinning disks pay for moving the head
        info.1 = Some(p[8] == 0);
    }
    unsafe { CloseHandle(h) };
    info
}

/// Phones and cameras plugged in by USB: Windows shows them as "portable devices", not as drives.
fn phones() -> Vec<String> {
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
    use windows_sys::core::GUID;
    const PORTABLE_DEVICES: GUID = GUID::from_u128(0xEEC5AD98_8080_425F_922A_DABF3DE3F69A);
    let mut out = Vec::new();
    let set = unsafe { SetupDiGetClassDevsW(&PORTABLE_DEVICES, null(), null_mut(), DIGCF_PRESENT) };
    if set as isize == -1 {
        return out;
    }
    let text = |data: &SP_DEVINFO_DATA, prop: u32| -> String {
        let mut buf = [0u16; 256];
        let ok = unsafe {
            SetupDiGetDeviceRegistryPropertyW(set, data, prop, null_mut(), buf.as_mut_ptr() as _, (buf.len() * 2) as u32, null_mut())
        };
        if ok != 0 { from_wide(&buf) } else { String::new() }
    };
    for i in 0..64 {
        let mut data: SP_DEVINFO_DATA = unsafe { std::mem::zeroed() };
        data.cbSize = size_of::<SP_DEVINFO_DATA>() as u32;
        if unsafe { SetupDiEnumDeviceInfo(set, i, &mut data) } == 0 {
            break;
        }
        let service = text(&data, SPDRP_SERVICE).to_lowercase();
        let mut name = text(&data, SPDRP_FRIENDLYNAME);
        if name.is_empty() {
            name = text(&data, SPDRP_DEVICEDESC);
        }
        let apple = ["iphone", "ipad"].iter().any(|w| name.to_lowercase().contains(w));
        // USB sticks show up here too, through the "file system" driver: those are listed as drives already
        if (service.contains("mtp") || service.contains("ptp") || apple) && !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    unsafe { SetupDiDestroyDeviceInfoList(set) };
    out
}

/// The language of Windows' menus ("fr", "en"…).
pub fn ui_language() -> Option<String> {
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff;
    Some(
        match id {
            0x0C => "fr",
            0x09 => "en",
            0x0A => "es",
            0x07 => "de",
            _ => return None,
        }
        .to_string(),
    )
}

pub fn list_disks() -> Vec<Disk> {
    use windows_sys::Win32::Storage::FileSystem::*;
    let mut out = Vec::new();
    let mask = unsafe { GetLogicalDrives() };
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root = wide(&format!("{letter}:\\"));
        let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
        // 2 = removable, 3 = fixed, 5 = CD / DVD. Network and RAM drives are left out.
        if kind != 2 && kind != 3 && kind != 5 {
            continue;
        }
        let mut name = [0u16; 261];
        let mut fsn = [0u16; 261];
        let ok = unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                name.as_mut_ptr(),
                name.len() as u32,
                null_mut(),
                null_mut(),
                null_mut(),
                fsn.as_mut_ptr(),
                fsn.len() as u32,
            )
        };
        if ok == 0 {
            continue; // empty card reader or disc drive, and the like
        }
        let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
        unsafe { GetDiskFreeSpaceExW(root.as_ptr(), &mut avail, &mut total, &mut free) };
        let (mut spc, mut bps, mut nf, mut nt) = (0u32, 0u32, 0u32, 0u32);
        unsafe { GetDiskFreeSpaceW(root.as_ptr(), &mut spc, &mut bps, &mut nf, &mut nt) };
        let (bus, ssd, removable_medium, model) = storage_info(letter);
        let removable = kind == 2 || kind == 5 || removable_medium;
        out.push(Disk {
            id: format!("{letter}:"),
            root: PathBuf::from(format!("{letter}:\\")),
            device: format!(r"\\.\{letter}:"),
            label: from_wide(&name),
            fs: from_wide(&fsn),
            total,
            free,
            removable,
            cluster: (spc as u64 * bps as u64).max(512),
            kind: classify(bus, ssd, removable, kind == 5, &model),
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
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

/// Starts this program again with administrator rights. Returns true when Windows accepted.
pub fn relaunch_as_admin() -> bool {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let Ok(exe) = std::env::current_exe() else { return false };
    let exe = wide(&exe.to_string_lossy());
    let verb = wide("runas");
    // the elevated copy starts with a fresh environment: carry a pretended version over
    let v = crate::update::current();
    let params = (v != env!("CARGO_PKG_VERSION")).then(|| wide(&format!("--pretend-version {v}")));
    let params = params.as_ref().map_or(null(), |p| p.as_ptr());
    let r = unsafe { ShellExecuteW(null_mut(), verb.as_ptr(), exe.as_ptr(), params, null(), 1) };
    r as isize > 32
}

/// The volume's cluster map: one bit per cluster, 1 = in use. Only meaningful on NTFS,
/// where cluster 0 starts at the beginning of the volume.
pub fn volume_bitmap(file: &File) -> Option<Vec<u8>> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Ioctl::FSCTL_GET_VOLUME_BITMAP;
    let handle = file.as_raw_handle();
    let start: i64 = 0;
    let mut cap: usize = 1 << 16;
    for _ in 0..6 {
        let mut out = vec![0u8; cap];
        let mut got = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle as _,
                FSCTL_GET_VOLUME_BITMAP,
                &start as *const i64 as _,
                8,
                out.as_mut_ptr() as _,
                cap as u32,
                &mut got,
                null_mut(),
            )
        };
        let bits = i64::from_le_bytes(out[8..16].try_into().ok()?).max(0) as usize;
        let need = 16 + bits.div_ceil(8);
        if ok != 0 {
            let end = need.min(got as usize).max(16);
            return Some(out[16..end].to_vec());
        }
        let err = unsafe { GetLastError() };
        if err != 234 {
            return None; // anything but ERROR_MORE_DATA
        }
        cap = if bits > 0 { need + 64 } else { cap * 8 };
    }
    None
}

/// Free bytes on the drive holding `path`.
pub fn free_space(path: &Path) -> Option<u64> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let p = wide(&path.to_string_lossy());
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    let ok = unsafe { GetDiskFreeSpaceExW(p.as_ptr(), &mut avail, &mut total, &mut free) };
    (ok != 0).then_some(avail)
}

/// What the Recycle Bin of this drive holds: each `$I…` file describes the `$R…` next to it.
pub fn trash_items(disk: &Disk) -> Vec<TrashItem> {
    let mut out = Vec::new();
    let Ok(users) = std::fs::read_dir(disk.root.join("$Recycle.Bin")) else { return out };
    for user in users.flatten() {
        let Ok(items) = std::fs::read_dir(user.path()) else { continue };
        for item in items.flatten() {
            let fname = item.file_name().to_string_lossy().to_string();
            let Some(rest) = fname.strip_prefix("$I") else { continue };
            let Ok(info) = std::fs::read(item.path()) else { continue };
            if info.len() < 28 {
                continue;
            }
            let version = u64::from_le_bytes(info[0..8].try_into().unwrap());
            let size = u64::from_le_bytes(info[8..16].try_into().unwrap());
            let when = u64::from_le_bytes(info[16..24].try_into().unwrap());
            let path_u16: Vec<u16> = if version >= 2 {
                let n = u32::from_le_bytes(info[24..28].try_into().unwrap()) as usize;
                info[28..].as_chunks::<2>().0.iter().take(n).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
            } else {
                info[24..].as_chunks::<2>().0.iter().take(260).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
            };
            out.push(TrashItem {
                data: user.path().join(format!("$R{rest}")),
                original: PathBuf::from(from_wide(&path_u16)),
                // FILETIME counts 100 ns steps from 1601
                deleted: (when / 10_000_000).saturating_sub(11_644_473_600),
                size,
            });
        }
    }
    out
}

/// The picture Explorer shows for a file (first frame of a video, first page of a PDF…),
/// when Windows has a thumbnail maker for that kind of file.
pub fn thumbnail(path: &Path, size: i32) -> Option<Picture> {
    use windows_sys::Win32::Foundation::SIZE;
    use windows_sys::Win32::Graphics::Gdi::{DeleteObject, HBITMAP};
    use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize};
    use windows_sys::Win32::UI::Shell::SHCreateItemFromParsingName;
    use windows_sys::core::GUID;

    /// IShellItemImageFactory: IUnknown's three entries, then GetImage.
    #[repr(C)]
    struct Vtbl {
        _query_interface: usize,
        _add_ref: usize,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        get_image: unsafe extern "system" fn(*mut c_void, SIZE, i32, *mut HBITMAP) -> i32,
    }
    const IID_IMAGE_FACTORY: GUID = GUID::from_u128(0xbcc18b79_ba16_442f_80c4_8a59c30c463b);
    const BIGGER_SIZE_OK: i32 = 0x1;
    const THUMBNAIL_ONLY: i32 = 0x8; // no plain file-type icon

    let p = wide(&path.to_string_lossy());
    unsafe {
        let init = CoInitializeEx(null(), 2); // apartment-threaded
        let mut obj: *mut c_void = null_mut();
        let mut out = None;
        if SHCreateItemFromParsingName(p.as_ptr(), null_mut(), &IID_IMAGE_FACTORY, &mut obj) >= 0 && !obj.is_null() {
            let vtbl = *(obj as *const *const Vtbl);
            let mut hbm: HBITMAP = null_mut();
            if ((*vtbl).get_image)(obj, SIZE { cx: size, cy: size }, BIGGER_SIZE_OK | THUMBNAIL_ONLY, &mut hbm) >= 0 && !hbm.is_null() {
                out = bitmap_pixels(hbm);
                DeleteObject(hbm);
            }
            ((*vtbl).release)(obj);
        }
        if init >= 0 {
            CoUninitialize();
        }
        out
    }
}

unsafe fn bitmap_pixels(hbm: windows_sys::Win32::Graphics::Gdi::HBITMAP) -> Option<Picture> {
    use windows_sys::Win32::Graphics::Gdi::*;
    unsafe {
        let mut bm: BITMAP = std::mem::zeroed();
        if GetObjectW(hbm, size_of::<BITMAP>() as i32, &mut bm as *mut BITMAP as *mut c_void) == 0 {
            return None;
        }
        let (w, h) = (bm.bmWidth, bm.bmHeight.abs());
        if w <= 0 || h <= 0 || w > 4096 || h > 4096 {
            return None;
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // top row first
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let dc = GetDC(null_mut());
        let got = GetDIBits(dc, hbm, 0, h as u32, buf.as_mut_ptr() as *mut c_void, &mut info, DIB_RGB_COLORS);
        ReleaseDC(null_mut(), dc);
        if got == 0 {
            return None;
        }
        let has_alpha = buf.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
        for px in buf.as_chunks_mut::<4>().0 {
            px.swap(0, 2); // BGRA to RGBA
            if !has_alpha {
                px[3] = 255;
            }
        }
        Some(Picture { width: w as usize, height: h as usize, rgba: buf })
    }
}

fn mci(cmd: &str) -> Option<String> {
    use windows_sys::Win32::Media::Multimedia::mciSendStringW;
    let c = wide(cmd);
    let mut buf = [0u16; 128];
    let err = unsafe { mciSendStringW(c.as_ptr(), buf.as_mut_ptr(), buf.len() as u32, null_mut()) };
    (err == 0).then(|| from_wide(&buf))
}

/// Plays a sound file with Windows' own media layer, without any window.
#[derive(Default)]
pub struct Player {
    open: bool,
}

impl Player {
    pub const AVAILABLE: bool = true;

    pub fn play(&mut self, path: &Path) -> bool {
        self.stop();
        let opened = mci(&format!("open \"{}\" type mpegvideo alias openrecovr", path.display())).is_some();
        self.open = opened;
        opened && mci("play openrecovr").is_some()
    }

    pub fn stop(&mut self) {
        if self.open {
            mci("stop openrecovr");
            mci("close openrecovr");
            self.open = false;
        }
    }

    pub fn playing(&mut self) -> bool {
        self.open && mci("status openrecovr mode").is_some_and(|m| m == "playing")
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The icon next to the clock while the window is hidden and the search goes on.
/// It lives on its own thread with a hidden window that receives the clicks.
pub struct Tray {
    own: usize,
}

struct TrayState {
    main: usize,
    own: usize,
    icon: usize,
    status: String,
    shown: bool,
    quit: bool,
    wake: std::sync::Arc<dyn Fn() + Send + Sync>,
}

static TRAY: std::sync::Mutex<Option<TrayState>> = std::sync::Mutex::new(None);

const WM_TRAY: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 1;
const CMD_SHOW: usize = 1;
const CMD_QUIT: usize = 2;

/// An icon handle from 32×32 RGBA pixels, in the layout of an .ico entry.
fn make_icon(rgba: &[u8], size: i32) -> usize {
    use windows_sys::Win32::UI::WindowsAndMessaging::CreateIconFromResourceEx;
    let n = size as usize;
    let mut b = Vec::with_capacity(40 + n * n * 4 + n * n.div_ceil(32) * 4);
    for v in [40u32, size as u32, 2 * size as u32] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(&[0; 24]);
    for y in (0..n).rev() {
        for px in rgba[y * n * 4..(y + 1) * n * 4].as_chunks::<4>().0 {
            b.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    b.resize(b.len() + n * n.div_ceil(32) * 4, 0);
    unsafe { CreateIconFromResourceEx(b.as_ptr(), b.len() as u32, 1, 0x0003_0000, size, size, 0) as usize }
}

fn notify_data(own: usize) -> windows_sys::Win32::UI::Shell::NOTIFYICONDATAW {
    let mut d: windows_sys::Win32::UI::Shell::NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    d.cbSize = std::mem::size_of_val(&d) as u32;
    d.hWnd = own as _;
    d.uID = 1;
    d
}

fn copy_wide(dst: &mut [u16], s: &str) {
    let w: Vec<u16> = s.encode_utf16().take(dst.len() - 1).collect();
    dst[..w.len()].copy_from_slice(&w);
    dst[w.len()] = 0;
}

/// Puts the window back and removes the icon.
fn restore() {
    use windows_sys::Win32::UI::Shell::{NIM_DELETE, Shell_NotifyIconW};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let Some((main, own)) = TRAY.lock().unwrap().as_mut().map(|t| {
        t.shown = true;
        (t.main, t.own)
    }) else {
        return;
    };
    unsafe {
        let d = notify_data(own);
        Shell_NotifyIconW(NIM_DELETE, &d);
        ShowWindow(main as _, if IsIconic(main as _) != 0 { SW_RESTORE } else { SW_SHOW });
        SetForegroundWindow(main as _);
    }
    let wake = TRAY.lock().unwrap().as_ref().map(|t| t.wake.clone());
    if let Some(wake) = wake {
        wake();
    }
}

unsafe extern "system" fn tray_proc(hwnd: *mut c_void, msg: u32, wp: usize, lp: isize) -> isize {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::Shell::NIN_BALLOONUSERCLICK;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        match msg {
            WM_TRAY => match lp as u32 {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_BALLOONUSERCLICK => restore(),
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    let status = TRAY.lock().unwrap().as_ref().map(|t| t.status.clone()).unwrap_or_default();
                    let menu = CreatePopupMenu();
                    if !status.is_empty() {
                        AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, wide(&status).as_ptr());
                        AppendMenuW(menu, MF_SEPARATOR, 0, null());
                    }
                    AppendMenuW(menu, MF_STRING, CMD_SHOW, wide(t!("Afficher la fenêtre")).as_ptr());
                    AppendMenuW(menu, MF_STRING, CMD_QUIT, wide(t!("Arrêter et quitter")).as_ptr());
                    SetMenuDefaultItem(menu, CMD_SHOW as u32, 0);
                    let mut pt = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut pt);
                    SetForegroundWindow(hwnd);
                    let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, null());
                    PostMessageW(hwnd, WM_NULL, 0, 0);
                    DestroyMenu(menu);
                    if cmd as usize == CMD_QUIT
                        && let Some(t) = TRAY.lock().unwrap().as_mut()
                    {
                        t.quit = true;
                    }
                    if cmd as usize == CMD_SHOW || cmd as usize == CMD_QUIT {
                        restore();
                    }
                }
                _ => {}
            },
            WM_DESTROY => PostQuitMessage(0),
            _ => return DefWindowProcW(hwnd, msg, wp, lp),
        }
        0
    }
}

impl Tray {
    pub const AVAILABLE: bool = true;

    /// `window` is the application's own window; `wake` asks it to draw again.
    pub fn start(window: isize, icon: &[u8], wake: impl Fn() + Send + Sync + 'static) -> Option<Tray> {
        use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let (tx, rx) = std::sync::mpsc::channel::<usize>();
        let icon = icon.to_vec();
        std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || unsafe {
                let inst = GetModuleHandleW(null());
                let class = wide("OpenRecovrTray");
                let mut wc: WNDCLASSW = std::mem::zeroed();
                wc.lpfnWndProc = Some(tray_proc);
                wc.hInstance = inst;
                wc.lpszClassName = class.as_ptr();
                RegisterClassW(&wc);
                let own =
                    CreateWindowExW(0, class.as_ptr(), class.as_ptr(), WS_OVERLAPPED, 0, 0, 0, 0, null_mut(), null_mut(), inst, null());
                if own.is_null() {
                    let _ = tx.send(0);
                    return;
                }
                *TRAY.lock().unwrap() = Some(TrayState {
                    main: window as usize,
                    own: own as usize,
                    icon: make_icon(&icon, 32),
                    status: String::new(),
                    shown: true,
                    quit: false,
                    wake: std::sync::Arc::new(wake),
                });
                let _ = tx.send(own as usize);
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            })
            .ok()?;
        let own = rx.recv().ok().filter(|&h| h != 0)?;
        Some(Tray { own })
    }

    /// Hides the window from the screen and the taskbar; the icon appears by the clock.
    /// Nothing is hidden when the icon could not be put there, so the window can't get lost.
    pub fn hide_window(&self) -> bool {
        use windows_sys::Win32::UI::Shell::*;
        use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
        let Some((main, icon, status)) = TRAY.lock().unwrap().as_ref().map(|t| (t.main, t.icon, t.status.clone())) else {
            return false;
        };
        let mut d = notify_data(self.own);
        d.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        d.uCallbackMessage = WM_TRAY;
        d.hIcon = icon as _;
        copy_wide(&mut d.szTip, if status.is_empty() { "Open Recovr" } else { &status });
        if unsafe { Shell_NotifyIconW(NIM_ADD, &d) } == 0 {
            return false;
        }
        if let Some(t) = TRAY.lock().unwrap().as_mut() {
            t.shown = false;
        }
        unsafe { ShowWindow(main as _, SW_HIDE) };
        true
    }

    pub fn is_hidden(&self) -> bool {
        TRAY.lock().unwrap().as_ref().is_some_and(|t| !t.shown)
    }

    /// The line shown when the mouse rests on the icon, and at the top of its menu.
    pub fn set_status(&self, text: &str) {
        use windows_sys::Win32::UI::Shell::*;
        let hidden = match TRAY.lock().unwrap().as_mut() {
            Some(t) => {
                t.status = text.to_string();
                !t.shown
            }
            None => return,
        };
        if hidden {
            let mut d = notify_data(self.own);
            d.uFlags = NIF_TIP;
            copy_wide(&mut d.szTip, text);
            unsafe { Shell_NotifyIconW(NIM_MODIFY, &d) };
        }
    }

    /// A message bubble over the icon; clicking it brings the window back.
    pub fn notify(&self, title: &str, text: &str) {
        use windows_sys::Win32::UI::Shell::*;
        if !self.is_hidden() {
            return;
        }
        let mut d = notify_data(self.own);
        d.uFlags = NIF_INFO;
        d.dwInfoFlags = NIIF_INFO;
        copy_wide(&mut d.szInfoTitle, title);
        copy_wide(&mut d.szInfo, text);
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &d) };
    }

    /// True once, after "Arrêter et quitter" was chosen in the icon's menu.
    pub fn take_quit(&self) -> bool {
        TRAY.lock().unwrap().as_mut().is_some_and(|t| std::mem::take(&mut t.quit))
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        use windows_sys::Win32::UI::Shell::{NIM_DELETE, Shell_NotifyIconW};
        use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &notify_data(self.own));
            PostMessageW(self.own as _, WM_CLOSE, 0, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_icon_comes_and_goes() {
        assert_ne!(make_icon(&crate::art::app_icon(32), 32), 0);
        let tray = Tray::start(0, &crate::art::app_icon(32), || {}).expect("tray thread");
        tray.set_status("Recherche 42 % · 12 fichiers trouvés");
        assert!(tray.hide_window(), "the icon was not added");
        assert!(tray.is_hidden());
        tray.notify("Open Recovr", "test");
        drop(tray);
    }
}
