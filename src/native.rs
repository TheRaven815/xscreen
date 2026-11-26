use crate::{
    capture::Region,
    i18n::{Lang, Strings},
};
use eframe::egui;
use std::{path::PathBuf, sync::mpsc, thread};
use windows::{
    Win32::{
        Foundation::{self, *},
        Graphics::{Dwm::*, Gdi::*},
        System::SystemInformation::{GetLocalTime, GetSystemTime},
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
                OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
            },
            LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW},
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        },
        UI::{HiDpi::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
    },
    core::{Result, w},
};

pub fn fit_overlay(title: &str, bounds: Region, precision_cursor: bool, scale: f32) -> Result<()> {
    let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let hwnd = FindWindowW(None, windows::core::PCWSTR(title.as_ptr()))?;
        let mut owner = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut owner));
        if owner != std::process::id() {
            return Err(windows::core::Error::from_win32());
        }
        let _ = SetWindowDisplayAffinity(hwnd, WDA_NONE);
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect)?;
        let transitions_disabled: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &transitions_disabled as *const i32 as *const core::ffi::c_void,
            size_of::<i32>() as u32,
        );
        let aligned = rect.left == bounds.x && rect.top == bounds.y;
        let covers_monitor = rect.right - rect.left >= bounds.width as i32
            && rect.bottom - rect.top >= bounds.height as i32;
        if !aligned || !covers_monitor {
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                bounds.x,
                bounds.y,
                bounds.width as i32 + 1,
                bounds.height as i32 + 1,
                SWP_NOACTIVATE,
            )?;
        }
        let hcur_bits = if precision_cursor {
            precision_hcursor_bits(scale)
        } else {
            0
        };
        let _ = SetWindowSubclass(hwnd, Some(overlay_subclass_proc), 1, hcur_bits);
        if hcur_bits != 0
            && let Some((cx, cy)) = cursor_pos()
            && bounds.x <= cx
            && cx < bounds.x + bounds.width as i32
            && bounds.y <= cy
            && cy < bounds.y + bounds.height as i32
        {
            let _ = SetCursor(Some(HCURSOR(hcur_bits as *mut _)));
        }
        Ok(())
    }
}

unsafe extern "system" fn overlay_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    uid: usize,
    dw_ref_data: usize,
) -> LRESULT {
    unsafe {
        if msg == WM_SETCURSOR && (lparam.0 as u32 & 0xffff) == HTCLIENT && dw_ref_data != 0 {
            let _ = SetCursor(Some(HCURSOR(dw_ref_data as *mut _)));
            return LRESULT(1);
        }
        if msg == WM_NCDESTROY {
            let _ = RemoveWindowSubclass(hwnd, Some(overlay_subclass_proc), uid);
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }
}

/// Generate a crisp 32-bit BGRA precision crosshair cursor (`(dim, center, bgra)`).
/// Features a 1px white core, 1px dark graphite halo for universal contrast on any wallpaper,
/// a clean transparent target gap around the center, and a 1px center pinpoint dot.
pub(crate) fn precision_crosshair_bgra(scale: f32) -> (u32, u32, Vec<u8>) {
    let (dim, gap, arm_end): (i32, i32, i32) = if scale <= 1.15 {
        (29, 5, 12)
    } else if scale <= 1.65 {
        (37, 6, 16)
    } else {
        (49, 8, 21)
    };
    let c = dim / 2;
    let mut bgra = vec![0u8; (dim * dim * 4) as usize];
    let mut put = |x: i32, y: i32, px: [u8; 4]| {
        if x >= 0 && x < dim && y >= 0 && y < dim {
            let i = ((y * dim + x) * 4) as usize;
            bgra[i..i + 4].copy_from_slice(&px);
        }
    };

    let halo = [22, 18, 15, 220]; // Dark graphite outline
    let core = [255, 255, 255, 252]; // Crisp white core

    // Pass 1: 1px dark graphite halo around all 4 crosshair arms (including end caps)
    for d in gap..=arm_end {
        for (ax, ay) in [(c - d, c), (c + d, c), (c, c - d), (c, c + d)] {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    put(ax + dx, ay + dy, halo);
                }
            }
        }
    }

    // Pass 2: 1px crisp white core along all 4 arms
    for d in gap..=arm_end {
        put(c - d, c, core);
        put(c + d, c, core);
        put(c, c - d, core);
        put(c, c + d, core);
    }

    // Center pinpoint dot + subtle 4-neighbor contrast ring (leaves transparent ring before `gap`)
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        put(c + dx, c + dy, [22, 18, 15, 185]);
    }
    put(c, c, core);

    (dim as u32, c as u32, bgra)
}

fn precision_hcursor_bits(scale: f32) -> usize {
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<Vec<(u32, usize)>>> = OnceLock::new();
    let bucket = if scale <= 1.15 {
        100
    } else if scale <= 1.65 {
        150
    } else {
        200
    };
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(guard) = cache.lock()
        && let Some(&(_, bits)) = guard.iter().find(|&&(b, _)| b == bucket)
    {
        return bits;
    }

    let (dim, hotspot, bgra) = precision_crosshair_bgra(scale);
    let created = unsafe { create_alpha_cursor(dim, hotspot, &bgra) }.unwrap_or(0);
    if created != 0
        && let Ok(mut guard) = cache.lock()
    {
        guard.push((bucket, created));
    }
    created
}

unsafe fn create_alpha_cursor(dim: u32, hotspot: u32, bgra: &[u8]) -> Option<usize> {
    unsafe {
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = dim as i32;
        bmi.bmiHeader.biHeight = -(dim as i32); // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;

        let mut bits_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let hbm_color =
            CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0).ok()?;
        if hbm_color.is_invalid() || bits_ptr.is_null() {
            return None;
        }
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits_ptr.cast::<u8>(), bgra.len());

        let mask_stride = (dim as usize).div_ceil(16) * 2;
        let mask_bytes = vec![0u8; mask_stride * dim as usize];
        let hbm_mask = CreateBitmap(
            dim as i32,
            dim as i32,
            1,
            1,
            Some(mask_bytes.as_ptr().cast()),
        );
        if hbm_mask.is_invalid() {
            let _ = DeleteObject(hbm_color.into());
            return None;
        }

        let info = ICONINFO {
            fIcon: Foundation::FALSE,
            xHotspot: hotspot,
            yHotspot: hotspot,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };
        let hicon = CreateIconIndirect(&info).ok();
        let _ = DeleteObject(hbm_mask.into());
        let _ = DeleteObject(hbm_color.into());
        hicon.map(|ic| ic.0 as usize)
    }
}

/// Windows'un ortak denetimlerini ve tepsi menüsünü uygulama temasına çeker.
/// `SetPreferredAppMode`: 2 = ForceDark, 3 = ForceLight. Eski Windows'ta yok sayılır.
unsafe fn set_app_theme(light: bool) {
    unsafe {
        let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) else {
            return;
        };
        let Some(proc) = GetProcAddress(uxtheme, windows::core::PCSTR(135usize as *const u8))
        else {
            return;
        };
        let set_mode: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(proc);
        let _ = set_mode(if light { 3 } else { 2 });
        if let Some(proc) = GetProcAddress(uxtheme, windows::core::PCSTR(136usize as *const u8)) {
            let flush: unsafe extern "system" fn() = std::mem::transmute(proc);
            flush();
        }
    }
}

pub fn monitor_scale(bounds: Region) -> f32 {
    unsafe {
        let monitor = MonitorFromPoint(
            POINT {
                x: bounds.x,
                y: bounds.y,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        let (mut x, mut y) = (96, 96);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y);
        x as f32 / 96.0
    }
}

const CF_DIB: u32 = 8;
const CF_DIBV5: u32 = 17;

pub fn copy_to_clipboard(frame: &crate::capture::Frame, png: bool) -> Result<()> {
    let width = frame.bounds.width as usize;
    let height = frame.bounds.height as usize;
    if width == 0 || height == 0 || frame.bgra.len() < width * height * 4 {
        return Ok(());
    }
    let png = png
        .then(|| encode_png(width as u32, height as u32, &frame.bgra).ok())
        .flatten();
    let dib = dibv5_bytes(width, height, &frame.bgra);
    write_clipboard(dib, png)
}

/// Read an image from the Windows clipboard (`PNG`, `CF_DIBV5`, or `CF_DIB`) as top-down BGRA.
pub fn read_clipboard_bgra(
    lang: crate::i18n::Lang,
) -> std::result::Result<(Vec<u8>, [u32; 2]), String> {
    let s = crate::i18n::Strings::new(lang);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(read_clipboard_sta(lang));
    });
    rx.recv().map_err(|_| s.err_clipboard_busy().to_owned())?
}

fn read_clipboard_sta(lang: crate::i18n::Lang) -> std::result::Result<(Vec<u8>, [u32; 2]), String> {
    let s = crate::i18n::Strings::new(lang);
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.0 < 0 {
            return Err(s.err_clipboard_busy().into());
        }
        let res = read_clipboard_inner(lang);
        if hr.0 == 0 {
            CoUninitialize();
        }
        res
    }
}

unsafe fn read_clipboard_inner(
    lang: crate::i18n::Lang,
) -> std::result::Result<(Vec<u8>, [u32; 2]), String> {
    let s = crate::i18n::Strings::new(lang);
    if !open_clipboard() {
        return Err(s.err_clipboard_busy().into());
    }
    let result = unsafe {
        let png_fmt = RegisterClipboardFormatW(w!("PNG"));
        let from_png = if png_fmt != 0 && IsClipboardFormatAvailable(png_fmt).is_ok() {
            read_clipboard_bytes(png_fmt).and_then(|bytes| decode_png_bytes_to_bgra(&bytes))
        } else {
            None
        };
        if let Some(decoded) = from_png {
            Ok(decoded)
        } else if IsClipboardFormatAvailable(CF_DIBV5).is_ok() {
            read_clipboard_bytes(CF_DIBV5)
                .and_then(|bytes| parse_dib_to_bgra(&bytes))
                .ok_or_else(|| s.err_clipboard_unsupported().to_owned())
        } else if IsClipboardFormatAvailable(CF_DIB).is_ok() {
            read_clipboard_bytes(CF_DIB)
                .and_then(|bytes| parse_dib_to_bgra(&bytes))
                .ok_or_else(|| s.err_clipboard_unsupported().to_owned())
        } else {
            Err(s.err_clipboard_no_image().to_owned())
        }
    };
    unsafe {
        let _ = CloseClipboard();
    }
    result
}

unsafe fn read_clipboard_bytes(format: u32) -> Option<Vec<u8>> {
    unsafe {
        let handle = GetClipboardData(format).ok()?;
        let hmem = Foundation::HGLOBAL(handle.0);
        let len = GlobalSize(hmem);
        if len == 0 || len > 256 * 1024 * 1024 {
            return None;
        }
        let ptr = GlobalLock(hmem) as *const u8;
        if ptr.is_null() {
            return None;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let out = slice.to_vec();
        let _ = GlobalUnlock(hmem);
        Some(out)
    }
}

fn decode_png_bytes_to_bgra(bytes: &[u8]) -> Option<(Vec<u8>, [u32; 2])> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let buf_size = reader.output_buffer_size()?;
    let w = reader.info().width;
    let h = reader.info().height;
    let pixels = u64::from(w) * u64::from(h);
    if w == 0 || h == 0 || pixels > 40_000_000 || buf_size > 256 * 1024 * 1024 {
        return None;
    }
    let mut buf = vec![0u8; buf_size];
    let info = reader.next_frame(&mut buf).ok()?;
    let raw = &buf[..info.buffer_size()];
    let mut bgra = vec![0u8; pixels as usize * 4];
    match info.color_type {
        png::ColorType::Rgba => {
            for (src, dst) in raw
                .as_chunks::<4>()
                .0
                .iter()
                .zip(bgra.as_chunks_mut::<4>().0)
            {
                *dst = [src[2], src[1], src[0], src[3]];
            }
        }
        png::ColorType::Rgb => {
            for (src, dst) in raw
                .as_chunks::<3>()
                .0
                .iter()
                .zip(bgra.as_chunks_mut::<4>().0)
            {
                *dst = [src[2], src[1], src[0], 255];
            }
        }
        png::ColorType::Grayscale => {
            for (&v, dst) in raw.iter().zip(bgra.as_chunks_mut::<4>().0) {
                *dst = [v, v, v, 255];
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for (src, dst) in raw
                .as_chunks::<2>()
                .0
                .iter()
                .zip(bgra.as_chunks_mut::<4>().0)
            {
                *dst = [src[0], src[0], src[0], src[1]];
            }
        }
        _ => return None,
    }
    Some((bgra, [w, h]))
}

fn parse_dib_to_bgra(bytes: &[u8]) -> Option<(Vec<u8>, [u32; 2])> {
    if bytes.len() < size_of::<BITMAPINFOHEADER>() {
        return None;
    }
    let header_size = u32::from_le_bytes(bytes[0..4].try_into().ok()?) as usize;
    if header_size < size_of::<BITMAPINFOHEADER>() || header_size > bytes.len() {
        return None;
    }
    let width = i32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let height_raw = i32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let bpp = u16::from_le_bytes(bytes[14..16].try_into().ok()?);
    let compression = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
    if width <= 0 || height_raw == 0 || height_raw == i32::MIN {
        return None;
    }
    let top_down = height_raw < 0;
    let w = width as usize;
    let h = height_raw.unsigned_abs() as usize;
    if w * h > 40_000_000 {
        return None;
    }
    let mut offset = header_size;
    // BI_BITFIELDS (3) with 40-byte BITMAPINFOHEADER stores 3 color masks right after header.
    if compression == BI_BITFIELDS.0 && header_size == size_of::<BITMAPINFOHEADER>() {
        offset = offset.checked_add(12)?;
    } else if compression != BI_RGB.0 && compression != BI_BITFIELDS.0 {
        return None;
    }
    if offset > bytes.len() {
        return None;
    }
    let data = &bytes[offset..];
    let mut bgra = vec![0u8; w * h * 4];
    match bpp {
        32 => {
            let stride = w * 4;
            if data.len() < stride * h {
                return None;
            }
            for row in 0..h {
                let src_row = if top_down { row } else { h - 1 - row };
                let src = &data[src_row * stride..(src_row + 1) * stride];
                let dst = &mut bgra[row * stride..(row + 1) * stride];
                for (s, d) in src
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(dst.as_chunks_mut::<4>().0)
                {
                    *d = [s[0], s[1], s[2], 255];
                }
            }
        }
        24 => {
            let stride = (w * 3).div_ceil(4) * 4;
            if data.len() < stride * h {
                return None;
            }
            for row in 0..h {
                let src_row = if top_down { row } else { h - 1 - row };
                let src = &data[src_row * stride..src_row * stride + w * 3];
                let dst = &mut bgra[row * w * 4..(row + 1) * w * 4];
                for (s, d) in src
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .zip(dst.as_chunks_mut::<4>().0)
                {
                    *d = [s[0], s[1], s[2], 255];
                }
            }
        }
        _ => return None,
    }
    Some((bgra, [w as u32, h as u32]))
}

/// Pano STA ister. Encode çağıran iş parçacığında değil, burada bir kez başlat.
fn write_clipboard(dib: Vec<u8>, png: Option<Vec<u8>>) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(write_clipboard_sta(&dib, png.as_deref()));
    });
    rx.recv().unwrap_or_else(|_| {
        Err(windows::core::Error::from_hresult(
            windows::core::HRESULT(0x8000_4005u32 as i32), // E_FAIL
        ))
    })
}

fn write_clipboard_sta(dib: &[u8], png: Option<&[u8]>) -> Result<()> {
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.0 < 0 {
            return Err(windows::core::Error::from_hresult(hr));
        }
        let result = set_clipboard(dib, png);
        if hr.0 == 0 {
            CoUninitialize();
        }
        result
    }
}

fn set_clipboard(dib: &[u8], png: Option<&[u8]>) -> Result<()> {
    unsafe {
        let dib_mem = moveable(dib)?;
        let png_mem = png.map(moveable).transpose()?;
        if !open_clipboard() {
            let _ = Foundation::GlobalFree(Some(dib_mem));
            if let Some(png_mem) = png_mem {
                let _ = Foundation::GlobalFree(Some(png_mem));
            }
            return Err(windows::core::Error::from_win32());
        }
        let _ = EmptyClipboard();
        if let Err(error) = SetClipboardData(CF_DIBV5, Some(HANDLE(dib_mem.0))) {
            let _ = CloseClipboard();
            let _ = Foundation::GlobalFree(Some(dib_mem));
            if let Some(png_mem) = png_mem {
                let _ = Foundation::GlobalFree(Some(png_mem));
            }
            return Err(error);
        }
        if let Some(png_mem) = png_mem {
            let format = RegisterClipboardFormatW(w!("PNG"));
            if format == 0 || SetClipboardData(format, Some(HANDLE(png_mem.0))).is_err() {
                let _ = Foundation::GlobalFree(Some(png_mem));
            }
        }
        let _ = CloseClipboard();
        Ok(())
    }
}

fn moveable(bytes: &[u8]) -> Result<windows::Win32::Foundation::HGLOBAL> {
    unsafe {
        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes.len())?;
        let ptr = GlobalLock(hmem) as *mut u8;
        if ptr.is_null() {
            let _ = Foundation::GlobalFree(Some(hmem));
            return Err(windows::core::Error::from_win32());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(hmem);
        Ok(hmem)
    }
}

fn open_clipboard() -> bool {
    for _ in 0..5 {
        if unsafe { OpenClipboard(None) }.is_ok() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    false
}

fn dibv5_bytes(width: usize, height: usize, bgra: &[u8]) -> Vec<u8> {
    let header_size = size_of::<BITMAPV5HEADER>();
    let image_size = width * height * 4;
    let mut out = vec![0u8; header_size + image_size];
    let header = BITMAPV5HEADER {
        bV5Size: header_size as u32,
        bV5Width: width as i32,
        bV5Height: height as i32,
        bV5Planes: 1,
        bV5BitCount: 32,
        bV5Compression: BI_BITFIELDS,
        bV5SizeImage: image_size as u32,
        bV5RedMask: 0x00ff_0000,
        bV5GreenMask: 0x0000_ff00,
        bV5BlueMask: 0x0000_00ff,
        bV5AlphaMask: 0xff00_0000,
        bV5CSType: 0x5769_6e20, // 'Win '
        ..Default::default()
    };
    out[..header_size].copy_from_slice(unsafe {
        std::slice::from_raw_parts(&header as *const _ as *const u8, header_size)
    });
    let stride = width * 4;
    for row in 0..height {
        let src = &bgra[row * stride..(row + 1) * stride];
        let dst = header_size + (height - 1 - row) * stride;
        out[dst..dst + stride].copy_from_slice(src);
    }
    out
}

pub fn encode_png(width: u32, height: u32, bgra: &[u8]) -> std::io::Result<Vec<u8>> {
    let total = (width as usize) * (height as usize) * 4;
    let src = &bgra[..bgra.len().min(total)];
    let mut rgba = vec![0u8; src.len()];
    for (px, dst) in src
        .as_chunks::<4>()
        .0
        .iter()
        .zip(rgba.as_chunks_mut::<4>().0)
    {
        *dst = [px[2], px[1], px[0], 255];
    }
    let mut out = Vec::with_capacity(src.len() / 3);
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder
        .write_header()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    writer
        .write_image_data(&rgba)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    writer
        .finish()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(out)
}

pub fn pictures_dir() -> Option<PathBuf> {
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Pictures, KF_FLAG_DEFAULT, None).ok()?;
        let folder = PathBuf::from(path.to_string().unwrap_or_default()).join("xscreen");
        CoTaskMemFree(Some(path.0.cast()));
        Some(folder)
    }
}

/// Atomically write PNG bytes to an exact target path (overwriting if it exists).
pub fn save_png_at(bytes: &[u8], path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("png.part");
    std::fs::write(&tmp, bytes)?;
    if std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp, path)?;
    }
    Ok(())
}

/// `YYYY-MM-DD_HH-mm-ss.png`, çakışırsa `_2`. Üzerine yazmaz. `folder` boşsa Resimler/xscreen.
pub fn save_png(bytes: &[u8], folder: Option<&std::path::Path>) -> std::io::Result<PathBuf> {
    let owned;
    let folder = if let Some(folder) = folder {
        folder
    } else {
        owned = pictures_dir().ok_or_else(|| std::io::Error::other("Pictures folder missing"))?;
        &owned
    };
    std::fs::create_dir_all(folder)?;
    let stamp = stamp_now();
    let path = unique_path(folder, &stamp);
    save_png_at(bytes, &path)?;
    Ok(path)
}

fn stamp_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_local(secs as i64)
}

fn format_local(unix: i64) -> String {
    let shifted = unix + i64::from(local_offset_secs());
    let days = shifted.div_euclid(86_400);
    let tod = shifted.rem_euclid(86_400) as u32;
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    format!("{year:04}-{month:02}-{day:02}_{hour:02}-{minute:02}-{second:02}")
}

fn local_offset_secs() -> i32 {
    unsafe {
        let utc = GetSystemTime();
        let local = GetLocalTime();
        let u = systemtime_to_unix(&utc);
        let l = systemtime_to_unix(&local);
        (l - u) as i32
    }
}

fn systemtime_to_unix(t: &Foundation::SYSTEMTIME) -> i64 {
    let days = days_from_civil(i32::from(t.wYear), u32::from(t.wMonth), u32::from(t.wDay));
    days * 86_400 + i64::from(t.wHour) * 3600 + i64::from(t.wMinute) * 60 + i64::from(t.wSecond)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let y = year as i64 - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (month as i32 + i32::from(month <= 2) * 12 - 3) + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u32;
    era * 146_097 + i64::from(doe) - 719_468
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + i64::from(month <= 2);
    (year as i32, month, day)
}

pub fn unique_path(folder: &std::path::Path, stamp: &str) -> PathBuf {
    let first = folder.join(format!("{stamp}.png"));
    if !first.exists() {
        return first;
    }
    let mut n = 2u32;
    loop {
        let path = folder.join(format!("{stamp}_{n}.png"));
        if !path.exists() {
            return path;
        }
        n = n.saturating_add(1);
        if n == 0 {
            return path;
        }
    }
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut pt = POINT::default();
    unsafe {
        if GetCursorPos(&mut pt).is_ok() {
            Some((pt.x, pt.y))
        } else {
            None
        }
    }
}

pub fn window_at(point: (i32, i32)) -> Option<Region> {
    struct Context {
        pt: POINT,
        found: Option<Region>,
        own_pid: u32,
    }
    let mut ctx = Context {
        pt: POINT {
            x: point.0,
            y: point.1,
        },
        found: None,
        own_pid: std::process::id(),
    };
    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        unsafe {
            let ctx = &mut *(lparam.0 as *mut Context);
            if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                return TRUE;
            }
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == ctx.own_pid {
                return TRUE;
            }
            let mut rect = RECT::default();
            let hr = DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut rect as *mut _ as *mut _,
                size_of::<RECT>() as u32,
            );
            if hr.is_err() {
                let _ = GetWindowRect(hwnd, &mut rect);
            }
            if rect.left <= ctx.pt.x
                && ctx.pt.x < rect.right
                && rect.top <= ctx.pt.y
                && ctx.pt.y < rect.bottom
            {
                let width = (rect.right - rect.left) as u32;
                let height = (rect.bottom - rect.top) as u32;
                if width >= 8 && height >= 8 {
                    ctx.found = Some(Region {
                        x: rect.left,
                        y: rect.top,
                        width,
                        height,
                    });
                    return FALSE;
                }
            }
            TRUE
        }
    }
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut _ as isize));
    }
    ctx.found
}

pub fn snap_value(val: i32, targets: &[i32], threshold: i32) -> i32 {
    let mut best = val;
    let mut min_dist = threshold + 1;
    for &target in targets {
        let dist = (target - val).abs();
        if dist <= threshold && dist < min_dist {
            min_dist = dist;
            best = target;
        }
    }
    best
}

const TRAY: u32 = WM_APP + 1;
const SET_HOTKEY: u32 = WM_APP + 2;
const SET_THEME: u32 = WM_APP + 6;
pub const NOTICE: u32 = WM_APP + 3;
pub const COPIED: u32 = WM_APP + 4;
pub const SAVED: u32 = WM_APP + 5;
const CAPTURE: u32 = 10;
const SETTINGS: u32 = 11;
const EXIT: u32 = 12;
const LAST: u32 = 13;
const FOLDER: u32 = 14;
const ALBUM: u32 = 15;
const EDITOR: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub modifiers: u32,
    pub key: u32,
}

impl Default for Hotkey {
    fn default() -> Self {
        Self {
            modifiers: 0,
            key: VK_SNAPSHOT.0 as u32,
        }
    }
}

impl Hotkey {
    pub fn label(self) -> String {
        let mut label = String::new();
        for (flag, name) in [
            (MOD_CONTROL.0, "Ctrl+"),
            (MOD_ALT.0, "Alt+"),
            (MOD_SHIFT.0, "Shift+"),
        ] {
            if self.modifiers & flag != 0 {
                label.push_str(name);
            }
        }
        if self.key == VK_SNAPSHOT.0 as u32 {
            label.push_str("Print Screen");
        } else {
            label.push_str(&format!("F{}", self.key - VK_F1.0 as u32 + 1));
        }
        label
    }
}

pub enum Event {
    Toggle,
    Cancel,
    Settings,
    Album,
    Editor,
    CaptureSaved,
    Quit,
    Hotkey(Option<Hotkey>),
    Error(String),
}

pub struct Native {
    hwnd: HWND,
    thread: Option<thread::JoinHandle<()>>,
}

struct State {
    events: mpsc::Sender<Event>,
    context: egui::Context,
    hotkey: Option<Hotkey>,
    hotkey_id: i32,
    icon: HICON,
    taskbar_created: u32,
}

impl State {
    fn send(&self, event: Event) {
        let _ = self.events.send(event);
        self.context.request_repaint();
    }

    unsafe fn register(&mut self, hwnd: HWND, hotkey: Hotkey) -> Result<()> {
        let next_id = if self.hotkey_id == 1 { 2 } else { 1 };
        unsafe {
            RegisterHotKey(
                Some(hwnd),
                next_id,
                HOT_KEY_MODIFIERS(hotkey.modifiers | MOD_NOREPEAT.0),
                hotkey.key,
            )?;
            if self.hotkey.is_some() {
                let _ = UnregisterHotKey(Some(hwnd), self.hotkey_id);
            }
        }
        self.hotkey_id = next_id;
        self.hotkey = Some(hotkey);
        self.send(Event::Hotkey(self.hotkey));
        Ok(())
    }
}

impl Native {
    #[cfg(test)]
    pub(crate) fn stub() -> Self {
        Self {
            hwnd: HWND::default(),
            thread: None,
        }
    }

    pub fn open_folder(&self, folder: Option<&std::path::Path>) {
        if let Some(dir) = folder
            .map(std::path::Path::to_path_buf)
            .or_else(|| crate::config::Config::load().save_folder())
        {
            unsafe {
                reveal_folder(self.hwnd, &dir);
            }
        }
    }

    pub fn open_path(&self, path: &std::path::Path) {
        unsafe {
            open_file(self.hwnd, path);
        }
    }

    pub fn start(
        context: egui::Context,
        events: mpsc::Sender<Event>,
    ) -> std::result::Result<Self, String> {
        let (ready, result) = mpsc::sync_channel(1);
        let thread = thread::spawn(move || unsafe {
            let run = || -> Result<()> {
                let instance = GetModuleHandleW(None)?;
                let dpi = GetDpiForSystem();
                let icon = LoadImageW(
                    Some(instance.into()),
                    windows::core::PCWSTR(1u16 as _),
                    IMAGE_ICON,
                    GetSystemMetricsForDpi(SM_CXSMICON, dpi),
                    GetSystemMetricsForDpi(SM_CYSMICON, dpi),
                    LR_SHARED,
                )
                .map(|handle| HICON(handle.0))
                .or_else(|_| LoadIconW(None, IDI_APPLICATION))?;
                let class = WNDCLASSW {
                    lpfnWndProc: Some(window_proc),
                    hInstance: instance.into(),
                    lpszClassName: w!("xscreen.tray"),
                    ..Default::default()
                };
                if RegisterClassW(&class) == 0 {
                    return Err(windows::core::Error::from_win32());
                }
                let taskbar_created = RegisterWindowMessageW(w!("TaskbarCreated"));
                let mut state = Box::new(State {
                    events,
                    context,
                    hotkey: None,
                    hotkey_id: 0,
                    icon,
                    taskbar_created,
                });
                let config = crate::config::Config::load();
                set_app_theme(config.light_theme);
                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("xscreen.tray"),
                    w!("xscreen"),
                    WINDOW_STYLE::default(),
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    Some(instance.into()),
                    Some((&mut *state as *mut State).cast()),
                )?;
                add_tray_icon(hwnd, icon);
                let hotkey = config.hotkey;
                if !config.greeted {
                    let mut marked = config;
                    marked.greeted = true;
                    if marked.save().is_ok() {
                        notify(
                            hwnd,
                            &format!("{} is active. Capture from the tray icon.", hotkey.label()),
                        );
                    }
                }
                let fallback = Hotkey {
                    modifiers: MOD_CONTROL.0,
                    ..Hotkey::default()
                };
                if state.register(hwnd, hotkey).is_err() {
                    if hotkey == fallback || state.register(hwnd, fallback).is_err() {
                        state.send(Event::Hotkey(None));
                        notify(
                            hwnd,
                            "Shortcuts are in use. Pick another from the tray Settings menu.",
                        );
                    } else {
                        notify(
                            hwnd,
                            "Shortcut in use. Ctrl+Print Screen is active. Exclusive fullscreen games need the tray menu.",
                        );
                    }
                }
                let _ = ready.send(Ok(hwnd.0 as usize));
                let mut message = MSG::default();
                loop {
                    let result = GetMessageW(&mut message, None, 0, 0).0;
                    if result <= 0 {
                        break;
                    }
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                let _ = Shell_NotifyIconW(NIM_DELETE, &tray_data(hwnd));
                if state.hotkey.is_some() {
                    let _ = UnregisterHotKey(Some(hwnd), state.hotkey_id);
                }
                if IsWindow(Some(hwnd)).as_bool() {
                    let _ = DestroyWindow(hwnd);
                }
                Ok(())
            };
            if let Err(error) = run() {
                let _ = ready.send(Err(error.to_string()));
            }
        });
        let hwnd = result.recv().map_err(|error| error.to_string())??;
        Ok(Self {
            hwnd: HWND(hwnd as *mut _),
            thread: Some(thread),
        })
    }

    pub fn set_hotkey(&self, hotkey: Hotkey) {
        unsafe {
            let _ = PostMessageW(
                Some(self.hwnd),
                SET_HOTKEY,
                WPARAM(hotkey.modifiers as usize),
                LPARAM(hotkey.key as isize),
            );
        }
    }

    pub fn set_theme(&self, light: bool) {
        unsafe {
            let _ = PostMessageW(
                Some(self.hwnd),
                SET_THEME,
                WPARAM(usize::from(light)),
                LPARAM(0),
            );
        }
    }

    pub fn error(&self) {
        Self::post(self.hwnd_bits(), NOTICE);
    }

    pub fn hwnd_bits(&self) -> usize {
        self.hwnd.0 as usize
    }

    pub fn post(hwnd: usize, message: u32) {
        unsafe {
            let _ = PostMessageW(Some(HWND(hwnd as *mut _)), message, WPARAM(0), LPARAM(0));
        }
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        unsafe {
            let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if state.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let state = &mut *state;
        match message {
            WM_COMMAND => match wparam.0 as u32 & 0xffff {
                CAPTURE => state.send(Event::Toggle),
                SETTINGS => state.send(Event::Settings),
                ALBUM => state.send(Event::Album),
                EDITOR => state.send(Event::Editor),
                EXIT => state.send(Event::Quit),
                _ => {}
            },
            WM_HOTKEY => state.send(Event::Toggle),
            WM_DISPLAYCHANGE => state.send(Event::Cancel),
            SET_HOTKEY => {
                let hotkey = Hotkey {
                    modifiers: wparam.0 as u32,
                    key: lparam.0 as u32,
                };
                if state.hotkey == Some(hotkey) {
                    state.send(Event::Hotkey(state.hotkey));
                } else if let Err(error) = state.register(hwnd, hotkey) {
                    tracing::error!(%error, "hotkey register failed");
                    let lang = crate::config::Config::load().lang;
                    let msg = if lang == Lang::Tr {
                        "Kısayol başka bir uygulama tarafından kullanılıyor."
                    } else {
                        "Shortcut in use. Previous shortcut kept; pick another."
                    };
                    state.send(Event::Error(msg.into()));
                }
            }
            SET_THEME => set_app_theme(wparam.0 != 0),
            NOTICE => {
                let lang = crate::config::Config::load().lang;
                let msg = if lang == Lang::Tr {
                    "Yakalama başarısız oldu. Ayrıntılar: %APPDATA%\\xscreen\\error.log"
                } else {
                    "Capture failed. Leave the lock screen and try again. Details: %APPDATA%\\xscreen\\error.log"
                };
                notify(hwnd, msg);
            }
            COPIED => {
                let s = Strings::new(crate::config::Config::load().lang);
                notify(hwnd, s.copied_notice());
            }
            SAVED => {
                let s = Strings::new(crate::config::Config::load().lang);
                notify(hwnd, s.saved_notice());
                state.send(Event::CaptureSaved);
            }
            TRAY => match lparam.0 as u32 {
                WM_LBUTTONDBLCLK => state.send(Event::Toggle),
                WM_RBUTTONUP | WM_CONTEXTMENU => menu(hwnd, state),
                _ => {}
            },
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
            }
            WM_DESTROY => PostQuitMessage(0),
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            other if state.taskbar_created != 0 && other == state.taskbar_created => {
                add_tray_icon(hwnd, state.icon);
            }
            _ => return DefWindowProcW(hwnd, message, wparam, lparam),
        }
        LRESULT(0)
    }
}

fn wide_into<const N: usize>(target: &mut [u16; N], text: &str) {
    target.fill(0);
    for (slot, ch) in target.iter_mut().take(N - 1).zip(text.encode_utf16()) {
        *slot = ch;
    }
}

fn tray_data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    }
}

unsafe fn add_tray_icon(hwnd: HWND, icon: HICON) {
    let mut data = tray_data(hwnd);
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.hIcon = icon;
    data.uCallbackMessage = TRAY;
    wide_into(&mut data.szTip, "xscreen");
    unsafe {
        if !Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
            tracing::warn!("Shell_NotifyIconW(NIM_ADD) deferred until TaskbarCreated");
        }
    }
}

unsafe fn notify(hwnd: HWND, text: &str) {
    let mut data = tray_data(hwnd);
    data.uFlags = NIF_INFO;
    data.dwInfoFlags = NIIF_INFO;
    wide_into(&mut data.szInfoTitle, "xscreen");
    wide_into(&mut data.szInfo, text);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
    }
}

unsafe fn menu(hwnd: HWND, state: &State) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let cfg = crate::config::Config::load();
        let s = Strings::new(cfg.lang);
        let recent = recent_files();
        let add = |id: u32, title: &str, flags: MENU_ITEM_FLAGS| {
            let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
            let _ = AppendMenuW(
                menu,
                flags,
                id as usize,
                windows::core::PCWSTR(title.as_ptr()),
            );
        };
        add(CAPTURE, s.tray_capture(), MF_STRING);
        add(ALBUM, s.tray_studio(), MF_STRING);
        add(EDITOR, s.tray_editor(), MF_STRING);
        add(
            LAST,
            s.tray_last(),
            if recent.is_empty() {
                MF_GRAYED
            } else {
                MF_STRING
            },
        );
        add(FOLDER, s.tray_folder(), MF_STRING);
        add(0, "", MF_SEPARATOR);
        add(SETTINGS, s.tray_settings(), MF_STRING);
        add(EXIT, s.tray_exit(), MF_STRING);
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(hwnd);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            None,
            hwnd,
            None,
        )
        .0 as u32;
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        match selected {
            CAPTURE => state.send(Event::Toggle),
            SETTINGS => state.send(Event::Settings),
            ALBUM => state.send(Event::Album),
            EDITOR => state.send(Event::Editor),
            EXIT => state.send(Event::Quit),
            LAST => {
                if let Some(path) = recent.first() {
                    open_file(hwnd, path);
                }
            }
            FOLDER => reveal_folder(hwnd, &cfg.save_folder().unwrap_or_default()),
            _ => {}
        }
    }
}

fn recent_files() -> Vec<PathBuf> {
    let Some(folder) = crate::config::Config::load().save_folder() else {
        return Vec::new();
    };
    let Ok(files) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut files: Vec<_> = files
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !entry.file_type().ok()?.is_file() || !path.extension()?.eq_ignore_ascii_case("png")
            {
                return None;
            }
            Some((entry.metadata().ok()?.modified().ok()?, path))
        })
        .collect();
    files.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    files.into_iter().take(8).map(|(_, path)| path).collect()
}

unsafe fn reveal_folder(hwnd: HWND, folder: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    let _ = std::fs::create_dir_all(folder);
    let path: Vec<u16> = folder.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let result = ShellExecuteW(
            Some(hwnd),
            w!("explore"),
            windows::core::PCWSTR(path.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
        if result.0 as isize <= 32 {
            tracing::error!(code = result.0 as isize, "folder open failed");
            notify(hwnd, "Could not open the capture folder.");
        }
    }
}

unsafe fn open_file(hwnd: HWND, path: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let result = ShellExecuteW(
            Some(hwnd),
            w!("open"),
            windows::core::PCWSTR(path.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
        if result.0 as isize <= 32 {
            tracing::error!(code = result.0 as isize, "capture open failed");
            notify(
                hwnd,
                "Could not open the capture. The file may have moved, or no app handles PNG.",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_local_uses_offset_and_unique_path_skips_existing() {
        assert_eq!(
            format_local(0 - i64::from(local_offset_secs())),
            "1970-01-01_00-00-00"
        );
        let dir = std::env::temp_dir().join(format!("xscreen-name-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("2026-10-23_12-00-00.png"), b"a").unwrap();
        let path = unique_path(&dir, "2026-10-23_12-00-00");
        assert_eq!(path.file_name().unwrap(), "2026-10-23_12-00-00_2.png");
        let fresh = unique_path(&dir, "2026-10-23_12-00-01");
        assert_eq!(fresh.file_name().unwrap(), "2026-10-23_12-00-01.png");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn encode_png_and_dib_roundtrip_keep_pixels() {
        let bgra = [
            10u8, 20, 250, 255, 30, 240, 40, 255, 220, 50, 60, 255, 0, 0, 0, 255,
        ];
        let bytes = encode_png(2, 2, &bgra).unwrap();
        assert!(bytes.starts_with(b"\x89PNG"));
        let (decoded_png, sz) = decode_png_bytes_to_bgra(&bytes).unwrap();
        assert_eq!(sz, [2, 2]);
        assert_eq!(decoded_png, bgra);

        let dib = dibv5_bytes(2, 2, &bgra);
        let (decoded_dib, sz2) = parse_dib_to_bgra(&dib).unwrap();
        assert_eq!(sz2, [2, 2]);
        assert_eq!(decoded_dib, bgra);
    }

    #[test]
    fn snap_value_snaps_within_threshold_and_leaves_others() {
        let targets = [0, 100, 500];
        assert_eq!(snap_value(5, &targets, 6), 0);
        assert_eq!(snap_value(-4, &targets, 6), 0);
        assert_eq!(snap_value(7, &targets, 6), 7);
        assert_eq!(snap_value(95, &targets, 6), 100);
        assert_eq!(snap_value(106, &targets, 6), 100);
        assert_eq!(snap_value(107, &targets, 6), 107);
    }

    #[test]
    fn precision_crosshair_has_symmetric_arms_and_transparent_target_gap() {
        for scale in [1.0, 1.5, 2.0] {
            let (dim, c, bgra) = precision_crosshair_bgra(scale);
            assert_eq!(dim % 2, 1);
            assert_eq!(c, dim / 2);
            let px = |x: u32, y: u32| -> [u8; 4] {
                let i = ((y * dim + x) * 4) as usize;
                bgra[i..i + 4].try_into().unwrap()
            };
            // Center hotspot has crisp opaque pinpoint
            assert!(px(c, c)[3] >= 240);
            // Ring at distance 2 from center is completely transparent so target pixel stays clear
            assert_eq!(px(c + 2, c)[3], 0);
            assert_eq!(px(c - 2, c)[3], 0);
            assert_eq!(px(c, c + 2)[3], 0);
            assert_eq!(px(c, c - 2)[3], 0);
            // Symmetric arms in all 4 directions
            assert_eq!(px(c + 8, c), px(c - 8, c));
            assert_eq!(px(c, c + 8), px(c, c - 8));
        }
    }
}
