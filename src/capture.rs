use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
use windows::{
    Win32::{
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0},
            Direct3D11::*,
            Dxgi::{Common::*, *},
            Gdi::*,
        },
        UI::WindowsAndMessaging::*,
    },
    core::{Interface, Result},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy)]
pub struct Monitor {
    pub bounds: Region,
    adapter: u32,
    output: u32,
    hdr: bool,
}

#[derive(Clone)]
pub struct Frame {
    pub bounds: Region,
    pub bgra: Vec<u8>,
}

pub struct CursorSnapshot {
    icon: HICON,
    bounds: Region,
}

impl Drop for CursorSnapshot {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyIcon(self.icon);
        }
    }
}

impl CursorSnapshot {
    pub fn capture() -> Result<Option<Self>> {
        unsafe {
            let mut cursor = CURSORINFO {
                cbSize: size_of::<CURSORINFO>() as u32,
                ..Default::default()
            };
            GetCursorInfo(&mut cursor)?;
            if cursor.flags != CURSOR_SHOWING {
                return Ok(None);
            }
            let icon = CopyIcon(HICON(cursor.hCursor.0))?;
            let mut snapshot = Self {
                icon,
                bounds: Region {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
            };
            let mut info = ICONINFO::default();
            GetIconInfo(icon, &mut info)?;
            let mut bitmap = BITMAP::default();
            let color = !info.hbmColor.is_invalid();
            let result = GetObjectW(
                if color {
                    info.hbmColor.into()
                } else {
                    info.hbmMask.into()
                },
                size_of::<BITMAP>() as i32,
                Some((&mut bitmap as *mut BITMAP).cast()),
            );
            if color {
                let _ = DeleteObject(info.hbmColor.into());
            }
            let _ = DeleteObject(info.hbmMask.into());
            if result == 0 {
                return Err(windows::core::Error::from_win32());
            }
            snapshot.bounds = Region {
                x: cursor.ptScreenPos.x - info.xHotspot as i32,
                y: cursor.ptScreenPos.y - info.yHotspot as i32,
                width: bitmap.bmWidth as u32,
                height: (bitmap.bmHeight / if color { 1 } else { 2 }) as u32,
            };
            Ok(Some(snapshot))
        }
    }

    pub fn layer(&self, frame: &Frame) -> Result<Option<Frame>> {
        let a = self.bounds;
        let b = frame.bounds;
        let left = a.x.max(b.x);
        let top = a.y.max(b.y);
        let right = (i64::from(a.x) + i64::from(a.width)).min(i64::from(b.x) + i64::from(b.width));
        let bottom =
            (i64::from(a.y) + i64::from(a.height)).min(i64::from(b.y) + i64::from(b.height));
        if right <= i64::from(left) || bottom <= i64::from(top) {
            return Ok(None);
        }
        let bounds = Region {
            x: left,
            y: top,
            width: (right - i64::from(left)) as u32,
            height: (bottom - i64::from(top)) as u32,
        };
        let mut layer = frame.crop(bounds).map_err(|message| {
            windows::core::Error::new(windows::Win32::Foundation::E_INVALIDARG, message)
        })?;
        unsafe {
            let dc = CreateCompatibleDC(None);
            if dc.is_invalid() {
                return Err(windows::core::Error::from_win32());
            }
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: bounds.width as i32,
                    biHeight: -(bounds.height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
            {
                Ok(bitmap) => bitmap,
                Err(error) => {
                    let _ = DeleteDC(dc);
                    return Err(error);
                }
            };
            std::ptr::copy_nonoverlapping(layer.bgra.as_ptr(), bits.cast(), layer.bgra.len());
            let previous = SelectObject(dc, bitmap.into());
            let drawn = DrawIconEx(
                dc,
                a.x - left,
                a.y - top,
                self.icon,
                a.width as i32,
                a.height as i32,
                0,
                None,
                DI_NORMAL,
            );
            let _ = GdiFlush();
            std::ptr::copy_nonoverlapping(bits.cast(), layer.bgra.as_mut_ptr(), layer.bgra.len());
            SelectObject(dc, previous);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(dc);
            drawn?;
        }
        Ok(Some(layer))
    }
}

fn half_to_float(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = (bits >> 10) & 31;
    let fraction = f32::from(bits & 1023);
    match exponent {
        0 => sign * fraction * 2.0_f32.powi(-24),
        31 if fraction == 0.0 => sign * f32::INFINITY,
        31 => f32::NAN,
        _ => sign * (1.0 + fraction / 1024.0) * 2.0_f32.powi(i32::from(exponent) - 15),
    }
}

fn scrgb_to_bgra(rgb: [f32; 3]) -> [u8; 4] {
    // ponytail: SDR beyazı 80 nit, parlak HDR değerleri kırpılır; renk profili ve ton eşleme HDR aşamasında.
    let [r, g, b] = rgb.map(|value| {
        let value = if value.is_nan() {
            0.0
        } else {
            value.clamp(0.0, 1.0)
        };
        let srgb = if value <= 0.0031308 {
            value * 12.92
        } else {
            1.055 * value.powf(1.0 / 2.4) - 0.055
        };
        (srgb * 255.0).round() as u8
    });
    [b, g, r, 255]
}

fn rotate_bgra(
    source: &[u8],
    width: usize,
    height: usize,
    rotation: DXGI_MODE_ROTATION,
) -> Vec<u8> {
    if rotation == DXGI_MODE_ROTATION_IDENTITY || rotation == DXGI_MODE_ROTATION_UNSPECIFIED {
        return source.to_vec();
    }
    let mut target = vec![0; source.len()];
    for y in 0..height {
        for x in 0..width {
            let (tx, ty, target_width) = match rotation {
                DXGI_MODE_ROTATION_ROTATE90 => (height - 1 - y, x, height),
                DXGI_MODE_ROTATION_ROTATE180 => (width - 1 - x, height - 1 - y, width),
                DXGI_MODE_ROTATION_ROTATE270 => (y, width - 1 - x, height),
                _ => (x, y, width),
            };
            let from = (y * width + x) * 4;
            let to = (ty * target_width + tx) * 4;
            target[to..to + 4].copy_from_slice(&source[from..from + 4]);
        }
    }
    target
}

impl Frame {
    pub fn crop(&self, region: Region) -> std::result::Result<Self, &'static str> {
        let x = i64::from(region.x) - i64::from(self.bounds.x);
        let y = i64::from(region.y) - i64::from(self.bounds.y);
        if region.width == 0
            || region.height == 0
            || x < 0
            || y < 0
            || x + i64::from(region.width) > i64::from(self.bounds.width)
            || y + i64::from(region.height) > i64::from(self.bounds.height)
        {
            return Err("Region is outside the captured frame or empty.");
        }
        let stride = self.bounds.width as usize * 4;
        let row_bytes = region.width as usize * 4;
        let mut bgra = Vec::with_capacity(row_bytes * region.height as usize);
        for row in y as usize..y as usize + region.height as usize {
            let start = row * stride + x as usize * 4;
            bgra.extend_from_slice(&self.bgra[start..start + row_bytes]);
        }
        Ok(Self {
            bounds: region,
            bgra,
        })
    }

    pub fn compose(frames: &[Frame], region: Region) -> Self {
        let width = region.width as usize;
        let height = region.height as usize;
        let mut bgra = vec![0u8; width * height * 4];
        for frame in frames {
            let left = region.x.max(frame.bounds.x);
            let top = region.y.max(frame.bounds.y);
            let right = (i64::from(region.x) + i64::from(region.width))
                .min(i64::from(frame.bounds.x) + i64::from(frame.bounds.width));
            let bottom = (i64::from(region.y) + i64::from(region.height))
                .min(i64::from(frame.bounds.y) + i64::from(frame.bounds.height));
            if right <= i64::from(left) || bottom <= i64::from(top) {
                continue;
            }
            let inter = Region {
                x: left,
                y: top,
                width: (right - i64::from(left)) as u32,
                height: (bottom - i64::from(top)) as u32,
            };
            if let Ok(cropped) = frame.crop(inter) {
                let dst_x = (inter.x - region.x) as usize;
                let dst_y = (inter.y - region.y) as usize;
                let inter_w = inter.width as usize;
                let inter_h = inter.height as usize;
                for row in 0..inter_h {
                    let src_start = row * inter_w * 4;
                    let src_end = src_start + inter_w * 4;
                    let dst_start = ((dst_y + row) * width + dst_x) * 4;
                    let dst_end = dst_start + inter_w * 4;
                    bgra[dst_start..dst_end].copy_from_slice(&cropped.bgra[src_start..src_end]);
                }
            }
        }
        Self {
            bounds: region,
            bgra,
        }
    }
}

pub trait Capture {
    fn monitors(&self) -> Result<Vec<Monitor>>;
    fn frame(&self, monitor: &Monitor) -> Result<Frame>;
    // Bölge seçimi 5. adımda bağlanacak.
    #[allow(dead_code)]
    fn region(&self, monitor: &Monitor, region: Region) -> Result<Frame> {
        self.frame(monitor)?.crop(region).map_err(|message| {
            windows::core::Error::new(windows::Win32::Foundation::E_INVALIDARG, message)
        })
    }
}

pub struct DesktopCapture;

#[derive(Clone)]
struct DxgiContext {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
}

static DXGI_CONTEXTS: OnceLock<Mutex<HashMap<(u32, u32), DxgiContext>>> = OnceLock::new();

fn dxgi_contexts() -> &'static Mutex<HashMap<(u32, u32), DxgiContext>> {
    DXGI_CONTEXTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn create_device(adapter: &IDXGIAdapter1) -> Result<DxgiContext> {
    unsafe {
        let mut device = None;
        let mut context = None;
        D3D11CreateDevice(
            adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            windows::Win32::Foundation::HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
        Ok(DxgiContext {
            device: device.unwrap(),
            context: context.unwrap(),
        })
    }
}

fn cached_context(
    adapter_index: u32,
    output_index: u32,
    adapter: &IDXGIAdapter1,
) -> Result<DxgiContext> {
    let key = (adapter_index, output_index);
    {
        let guard = dxgi_contexts()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(cached) = guard.get(&key) {
            return Ok(cached.clone());
        }
    }
    let created = create_device(adapter)?;
    let mut guard = dxgi_contexts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let entry = guard.entry(key).or_insert_with(|| created.clone());
    Ok(entry.clone())
}

fn invalidate_context(monitor: &Monitor) {
    dxgi_contexts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&(monitor.adapter, monitor.output));
}

/// D3D11CreateDevice can take ~250 ms on some drivers. Warm one device per
/// attached output in the background so the first screenshot does not pay it.
pub fn prewarm() {
    let Ok(monitors) = DesktopCapture.monitors() else {
        return;
    };
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
        return;
    };
    for monitor in monitors {
        if !monitor.hdr {
            continue;
        }
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(monitor.adapter) }) else {
            continue;
        };
        let _ = cached_context(monitor.adapter, monitor.output, &adapter);
    }
}

fn is_hdr_output(output: &IDXGIOutput) -> bool {
    unsafe {
        let Ok(output) = output.cast::<IDXGIOutput6>() else {
            return false;
        };
        let Ok(desc) = output.GetDesc1() else {
            return false;
        };
        matches!(
            desc.ColorSpace,
            DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
                | DXGI_COLOR_SPACE_RGB_STUDIO_G2084_NONE_P2020
                | DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709
        )
    }
}

impl Capture for DesktopCapture {
    fn monitors(&self) -> Result<Vec<Monitor>> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
            let mut monitors = Vec::new();
            let mut adapter_index = 0;
            loop {
                let adapter = match factory.EnumAdapters1(adapter_index) {
                    Ok(adapter) => adapter,
                    Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                    Err(error) => return Err(error),
                };
                let mut output_index = 0;
                loop {
                    let output = match adapter.EnumOutputs(output_index) {
                        Ok(output) => output,
                        Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                        Err(error) => return Err(error),
                    };
                    let desc = output.GetDesc()?;
                    if desc.AttachedToDesktop.as_bool() {
                        let rect = desc.DesktopCoordinates;
                        monitors.push(Monitor {
                            bounds: Region {
                                x: rect.left,
                                y: rect.top,
                                width: (rect.right - rect.left) as u32,
                                height: (rect.bottom - rect.top) as u32,
                            },
                            adapter: adapter_index,
                            output: output_index,
                            hdr: is_hdr_output(&output),
                        });
                    }
                    output_index += 1;
                }
                adapter_index += 1;
            }
            Ok(monitors)
        }
    }

    fn frame(&self, monitor: &Monitor) -> Result<Frame> {
        // SDR capture is an 8-bit desktop either way, and GDI returns it in
        // ~20 ms without creating a D3D11 device or driver threads. Use DXGI
        // only where HDR/wide-gamut pixels actually need it.
        if !monitor.hdr {
            match gdi_frame(monitor) {
                Ok(frame) => return Ok(frame),
                Err(error) => tracing::error!(%error, "GDI failed; trying DXGI"),
            }
        }
        match dxgi_frame(monitor) {
            Ok(frame) => Ok(frame),
            Err(error) if access_lost(&error) => {
                tracing::error!(%error, "DXGI access lost; retrying once");
                match dxgi_frame(monitor) {
                    Ok(frame) => Ok(frame),
                    Err(retry) => {
                        tracing::error!(%retry, "DXGI retry failed; capture cancelled");
                        Err(retry)
                    }
                }
            }
            Err(error) if device_lost(&error) => {
                tracing::error!(%error, "DXGI device lost; dropping cached device");
                invalidate_context(monitor);
                gdi_frame(monitor)
            }
            Err(error) if no_new_frame(&error) => {
                tracing::debug!(%error, "no new DXGI frame; using GDI");
                gdi_frame(monitor)
            }
            Err(error) if !protected_black(&error) => {
                tracing::error!(%error, "DXGI failed; trying GDI fallback");
                gdi_frame(monitor)
            }
            Err(error) => {
                tracing::error!(%error, "protected content; GDI would be black, capture cancelled");
                Err(error)
            }
        }
    }
}

/// Kilit ekranı veya çözünürlük değişimi. GDI'ye düşmek eski kare verir; bir kez DXGI yeter.
fn access_lost(error: &windows::core::Error) -> bool {
    error.code() == DXGI_ERROR_ACCESS_LOST
}

/// DRM / HDCP. GDI de siyah basar.
fn protected_black(error: &windows::core::Error) -> bool {
    error.message() == "Protected content; black frame."
}

/// Sabit masaüstünde yeni DXGI karesi yok. Bu normaldir; GDI aynı ekranı anında verir.
fn no_new_frame(error: &windows::core::Error) -> bool {
    error.message() == "DXGI returned an empty frame." || error.code() == DXGI_ERROR_WAIT_TIMEOUT
}

/// Sürücü güncellemesi veya GPU reset'i önbellekteki cihazı geçersiz bırakır.
fn device_lost(error: &windows::core::Error) -> bool {
    error.code() == DXGI_ERROR_DEVICE_REMOVED || error.code() == DXGI_ERROR_DEVICE_RESET
}

fn dxgi_frame(monitor: &Monitor) -> Result<Frame> {
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
        let adapter = factory.EnumAdapters1(monitor.adapter)?;
        let output = adapter.EnumOutputs(monitor.output)?;
        let rotation = output.GetDesc()?.Rotation;
        let DxgiContext { device, context } =
            cached_context(monitor.adapter, monitor.output, &adapter)?;
        let duplication = match output.cast::<IDXGIOutput5>() {
            Ok(output) => output.DuplicateOutput1(
                &device,
                0,
                &[DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_B8G8R8A8_UNORM],
            )?,
            Err(_) => output.cast::<IDXGIOutput1>()?.DuplicateOutput(&device)?,
        };
        // İlk AcquireNextFrame çoğu sürücüde boş (AccumulatedFrames == 0).
        // O dokuyu kopyalamak simsiyah kare verir. Eski kod boş kareyi 500 ms
        // bekliyordu; bu, sabit ekranda her yakalamayı yarım saniye geciktiriyor.
        // Şimdi anında dene, boşsa GDI'ye düş: sonuç aynı, bekleme yok.
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;
        duplication.AcquireNextFrame(0, &mut info, &mut resource)?;
        if info.ProtectedContentMaskedOut.as_bool() {
            if let Some(resource) = resource.take() {
                drop(resource);
            }
            let _ = duplication.ReleaseFrame();
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Protected content; black frame.",
            ));
        }
        if info.AccumulatedFrames == 0 {
            if let Some(resource) = resource.take() {
                drop(resource);
            }
            duplication.ReleaseFrame()?;
            return Err(windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "DXGI returned an empty frame.",
            ));
        }
        let result = (|| {
            let texture: ID3D11Texture2D = resource.unwrap().cast()?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc);
            let rotated =
                rotation == DXGI_MODE_ROTATION_ROTATE90 || rotation == DXGI_MODE_ROTATION_ROTATE270;
            let size = if rotated {
                (desc.Height, desc.Width)
            } else {
                (desc.Width, desc.Height)
            };
            if size != (monitor.bounds.width, monitor.bounds.height) {
                return Err(windows::core::Error::new(
                    windows::Win32::Foundation::E_FAIL,
                    "Display layout changed during capture.",
                ));
            }
            let hdr = desc.Format == DXGI_FORMAT_R16G16B16A16_FLOAT;
            if !hdr && desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM {
                return Err(windows::core::Error::new(
                    windows::Win32::Foundation::E_NOTIMPL,
                    "Unsupported pixel format.",
                ));
            }
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            let mut staging = None;
            device.CreateTexture2D(&desc, None, Some(&mut staging))?;
            let staging = staging.unwrap();
            context.CopyResource(&staging, &texture);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let stride = desc.Width as usize * if hdr { 8 } else { 4 };
            let mut bgra = vec![0; desc.Width as usize * desc.Height as usize * 4];
            for (row, target) in bgra.chunks_exact_mut(desc.Width as usize * 4).enumerate() {
                let source = std::slice::from_raw_parts(
                    (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                    stride,
                );
                if hdr {
                    for (source, target) in source
                        .as_chunks::<8>()
                        .0
                        .iter()
                        .zip(target.as_chunks_mut::<4>().0)
                    {
                        let rgb = [0, 2, 4]
                            .map(|i| half_to_float(u16::from_le_bytes([source[i], source[i + 1]])));
                        target.copy_from_slice(&scrgb_to_bgra(rgb));
                    }
                } else {
                    target.copy_from_slice(source);
                }
            }
            context.Unmap(&staging, 0);
            Ok(Frame {
                bounds: monitor.bounds,
                bgra: rotate_bgra(&bgra, desc.Width as usize, desc.Height as usize, rotation),
            })
        })();
        let release = duplication.ReleaseFrame();
        let frame = result?;
        release?;
        Ok(frame)
    }
}

fn gdi_frame(monitor: &Monitor) -> Result<Frame> {
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return Err(windows::core::Error::from_win32());
        }
        let memory = CreateCompatibleDC(Some(screen));
        if memory.is_invalid() {
            ReleaseDC(None, screen);
            return Err(windows::core::Error::from_win32());
        }
        let bounds = monitor.bounds;
        let bitmap = CreateCompatibleBitmap(screen, bounds.width as i32, bounds.height as i32);
        if bitmap.is_invalid() {
            let error = windows::core::Error::from_win32();
            let _ = DeleteDC(memory);
            ReleaseDC(None, screen);
            return Err(error);
        }
        let previous = SelectObject(memory, bitmap.into());
        let copied = BitBlt(
            memory,
            0,
            0,
            bounds.width as i32,
            bounds.height as i32,
            Some(screen),
            bounds.x,
            bounds.y,
            SRCCOPY | CAPTUREBLT,
        );
        SelectObject(memory, previous);
        let result = copied.and_then(|_| {
            let mut bgra = vec![0; bounds.width as usize * bounds.height as usize * 4];
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: bounds.width as i32,
                    biHeight: -(bounds.height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            if GetDIBits(
                memory,
                bitmap,
                0,
                bounds.height,
                Some(bgra.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            ) != bounds.height as i32
            {
                return Err(windows::core::Error::from_win32());
            }
            Ok(Frame { bounds, bgra })
        });
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_keeps_physical_pixels_in_display_order() {
        let pixels: Vec<u8> = (1..=6).flat_map(|n| [n, 0, 0, 255]).collect();
        let ids = |rotation| {
            rotate_bgra(&pixels, 3, 2, rotation)
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[0])
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(DXGI_MODE_ROTATION_IDENTITY), [1, 2, 3, 4, 5, 6]);
        assert_eq!(ids(DXGI_MODE_ROTATION_ROTATE90), [4, 1, 5, 2, 6, 3]);
        assert_eq!(ids(DXGI_MODE_ROTATION_ROTATE180), [6, 5, 4, 3, 2, 1]);
        assert_eq!(ids(DXGI_MODE_ROTATION_ROTATE270), [3, 6, 2, 5, 1, 4]);
    }

    #[test]
    fn empty_dxgi_acquire_is_treated_as_a_normal_gdi_fallback() {
        let empty = windows::core::Error::new(
            windows::Win32::Foundation::E_FAIL,
            "DXGI returned an empty frame.",
        );
        let timeout = windows::core::Error::from_hresult(DXGI_ERROR_WAIT_TIMEOUT);
        assert!(no_new_frame(&empty));
        assert!(no_new_frame(&timeout));
        assert!(!no_new_frame(&windows::core::Error::from_hresult(
            DXGI_ERROR_ACCESS_LOST
        )));
    }

    #[test]
    fn hdr_conversion_handles_sdr_white_clipping_and_invalid_values() {
        assert_eq!(half_to_float(0x3c00), 1.0);
        assert_eq!(half_to_float(0xbc00), -1.0);
        assert_eq!(half_to_float(1), 2.0_f32.powi(-24));
        assert!(half_to_float(0x7c00).is_infinite());
        assert!(half_to_float(0x7e00).is_nan());
        assert_eq!(scrgb_to_bgra([0.0, 0.0, 0.0]), [0, 0, 0, 255]);
        assert_eq!(scrgb_to_bgra([1.0, 0.5, 0.0]), [0, 188, 255, 255]);
        assert_eq!(scrgb_to_bgra([2.0, -1.0, f32::NAN]), [0, 0, 255, 255]);
    }

    #[test]
    fn cursor_is_separate_and_clipped_at_negative_monitor_edge() {
        unsafe {
            let cursor = CursorSnapshot {
                icon: CopyIcon(HICON(LoadCursorW(None, IDC_ARROW).unwrap().0)).unwrap(),
                bounds: Region {
                    x: -2,
                    y: -2,
                    width: 32,
                    height: 32,
                },
            };
            let frame = Frame {
                bounds: Region {
                    x: -8,
                    y: -8,
                    width: 16,
                    height: 16,
                },
                bgra: vec![100; 16 * 16 * 4],
            };
            let layer = cursor.layer(&frame).unwrap().unwrap();
            assert_eq!(
                layer.bounds,
                Region {
                    x: -2,
                    y: -2,
                    width: 10,
                    height: 10
                }
            );
            assert_eq!(frame.bgra, vec![100; 16 * 16 * 4]);
            let outside = Frame {
                bounds: Region {
                    x: 100,
                    y: 100,
                    ..frame.bounds
                },
                bgra: frame.bgra,
            };
            assert!(cursor.layer(&outside).unwrap().is_none());
        }
    }

    #[test]
    fn crop_handles_negative_origins_and_rejects_invalid_regions() {
        let frame = Frame {
            bounds: Region {
                x: -2,
                y: -1,
                width: 2,
                height: 2,
            },
            bgra: (0..16).collect(),
        };
        let region = Region {
            x: -1,
            y: 0,
            width: 1,
            height: 1,
        };
        assert_eq!(frame.crop(region).unwrap().bgra, [12, 13, 14, 15]);
        assert!(frame.crop(Region { x: 0, ..region }).is_err());
        assert!(frame.crop(Region { width: 0, ..region }).is_err());
        assert!(
            frame
                .crop(Region {
                    x: i32::MAX,
                    width: u32::MAX,
                    ..region
                })
                .is_err()
        );
    }

    #[test]
    fn compose_stitches_regions_across_multiple_virtual_monitors() {
        let left_screen = Frame {
            bounds: Region {
                x: -10,
                y: 0,
                width: 10,
                height: 5,
            },
            bgra: vec![1; 10 * 5 * 4],
        };
        let right_screen = Frame {
            bounds: Region {
                x: 0,
                y: 0,
                width: 10,
                height: 5,
            },
            bgra: vec![2; 10 * 5 * 4],
        };
        let selection = Region {
            x: -2,
            y: 1,
            width: 4,
            height: 2,
        };
        let composed = Frame::compose(&[left_screen, right_screen], selection);
        assert_eq!(composed.bounds, selection);
        // Each row should have 2 pixels of 1, then 2 pixels of 2:
        for row in composed.bgra.as_chunks::<16>().0 {
            assert_eq!(&row[0..8], &[1, 1, 1, 1, 1, 1, 1, 1]);
            assert_eq!(&row[8..16], &[2, 2, 2, 2, 2, 2, 2, 2]);
        }
    }
}
