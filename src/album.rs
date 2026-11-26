//! Disk is source of truth. Scan/decode/clipboard work stays off UI thread.
use eframe::egui;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::mpsc,
    time::SystemTime,
};

const MAX_CONCURRENT_DECODES: usize = 4;
const THUMB_CACHE_LIMIT: usize = 200;

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub name_lower: String,
    pub bytes: u64,
    pub modified: SystemTime,
}

enum Message {
    Scan(u64, Result<Vec<Entry>, String>),
    Thumb(u64, PathBuf, Result<(egui::ColorImage, [usize; 2]), String>),
    Copied(Result<(), String>),
}

pub struct Picture {
    pub texture: egui::TextureHandle,
    pub size: [usize; 2],
}

pub struct Album {
    pub entries: Vec<Entry>,
    pub query: String,
    pub thumbnails: HashMap<PathBuf, Result<Picture, String>>,
    thumb_order: VecDeque<PathBuf>,
    in_flight: HashSet<PathBuf>,
    pub loading: bool,
    pub copying: bool,
    pub error: Option<String>,
    pub notice: Option<(String, std::time::Instant)>,
    folder: Option<PathBuf>,
    generation: u64,
    last_scan: std::time::Instant,
    sender: mpsc::Sender<Message>,
    receiver: mpsc::Receiver<Message>,
}

impl Default for Album {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            entries: Vec::new(),
            query: String::new(),
            thumbnails: HashMap::new(),
            thumb_order: VecDeque::new(),
            in_flight: HashSet::new(),
            loading: false,
            copying: false,
            error: None,
            notice: None,
            folder: None,
            generation: 0,
            // Start expired so first open triggers an immediate scan.
            last_scan: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(60))
                .unwrap_or_else(std::time::Instant::now),
            sender,
            receiver,
        }
    }
}

impl Album {
    pub fn set_notice(&mut self, msg: impl Into<String>) {
        self.notice = Some((msg.into(), std::time::Instant::now()));
    }

    pub fn active_notice(&self) -> Option<&str> {
        self.notice.as_ref().and_then(|(msg, at)| {
            (at.elapsed() < std::time::Duration::from_secs(4)).then_some(msg.as_str())
        })
    }

    pub fn invalidate_path(&mut self, path: &Path) {
        self.thumbnails.remove(path);
        self.in_flight.remove(path);
    }

    pub fn refresh(&mut self, folder: Option<PathBuf>, ctx: &egui::Context, force: bool) {
        self.poll(ctx, &crate::i18n::Strings::new(crate::i18n::Lang::En));
        let changed = folder != self.folder;
        if !force && !changed && (self.loading || self.last_scan.elapsed().as_secs() < 6) {
            return;
        }
        self.folder = folder.clone();
        self.loading = true;
        self.error = None;
        self.last_scan = std::time::Instant::now();
        if changed {
            self.entries.clear();
            self.thumbnails.clear();
            self.thumb_order.clear();
            self.in_flight.clear();
        }
        self.generation = self.generation.wrapping_add(1);
        if changed {
            // Keep generation bump clean across folder switch.
        } else {
            self.in_flight.clear();
        }
        let generation = self.generation;
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = folder
                .ok_or_else(|| "Capture folder unavailable. Choose a folder in Settings.".into())
                .and_then(|folder| scan(&folder));
            let _ = sender.send(Message::Scan(generation, result));
            ctx.request_repaint();
        });
    }

    pub fn poll(&mut self, ctx: &egui::Context, s: &crate::i18n::Strings) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                Message::Scan(generation, result) if generation == self.generation => {
                    self.loading = false;
                    match result {
                        Ok(entries) => {
                            self.thumbnails.retain(|path, _| {
                                entries.iter().any(|entry| {
                                    &entry.path == path
                                        && self.entries.iter().any(|old| {
                                            old.path == entry.path
                                                && old.modified == entry.modified
                                                && old.bytes == entry.bytes
                                        })
                                })
                            });
                            self.thumb_order.retain(|p| self.thumbnails.contains_key(p));
                            self.entries = entries;
                        }
                        Err(_) => self.error = Some(s.err_read_folder().to_owned()),
                    }
                }
                Message::Thumb(generation, path, result) if generation == self.generation => {
                    self.in_flight.remove(&path);
                    let picture = result.map(|(image, size)| Picture {
                        texture: ctx.load_texture(
                            path.to_string_lossy(),
                            image,
                            egui::TextureOptions::LINEAR,
                        ),
                        size,
                    });
                    while self.thumbnails.len() >= THUMB_CACHE_LIMIT {
                        if let Some(oldest) = self.thumb_order.pop_front() {
                            self.thumbnails.remove(&oldest);
                        } else {
                            break;
                        }
                    }
                    if !self.thumbnails.contains_key(&path) {
                        self.thumb_order.push_back(path.clone());
                    }
                    self.thumbnails.insert(path, picture);
                }
                Message::Copied(result) => {
                    self.copying = false;
                    match result {
                        Ok(()) => self.set_notice(s.copied_notice()),
                        Err(_) => self.set_notice(s.err_copy_failed()),
                    }
                }
                _ => {}
            }
        }
    }

    pub fn request_thumb(&mut self, path: &Path, ctx: &egui::Context) {
        if self.loading
            || self.in_flight.len() >= MAX_CONCURRENT_DECODES
            || self.thumbnails.contains_key(path)
            || self.in_flight.contains(path)
        {
            return;
        }
        let owned = path.to_owned();
        self.in_flight.insert(owned.clone());
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        let generation = self.generation;
        std::thread::spawn(move || {
            let image =
                decode_rgba(&owned).map(|(rgba, size)| (scaled_image(&rgba, size, 420), size));
            let _ = sender.send(Message::Thumb(generation, owned, image));
            ctx.request_repaint();
        });
    }

    pub fn copy(&mut self, path: PathBuf, png: bool, ctx: &egui::Context) {
        if self.copying {
            return;
        }
        self.copying = true;
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = decode_bgra(&path).and_then(|(bgra, size)| {
                let frame = crate::capture::Frame {
                    bounds: crate::capture::Region {
                        x: 0,
                        y: 0,
                        width: size[0],
                        height: size[1],
                    },
                    bgra,
                };
                crate::native::copy_to_clipboard(&frame, png)
                    .map_err(|_| "Could not copy image. Try again.".into())
            });
            let _ = sender.send(Message::Copied(result));
            ctx.request_repaint();
        });
    }
}

fn scan(folder: &Path) -> Result<Vec<Entry>, String> {
    let files = match std::fs::read_dir(folder) {
        Ok(files) => files,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(
            "Could not read capture folder. Check path and permissions in Settings, then Refresh."
                .into(),
        ),
    };
    let mut entries = Vec::new();
    for file in files {
        let file = file.map_err(|_| "Could not read folder contents. Try Refresh.".to_owned())?;
        let path = file.path();
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            continue;
        }
        let Ok(metadata) = file.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let name = file.file_name().to_string_lossy().into_owned();
        let name_lower = name.to_lowercase();
        entries.push(Entry {
            name,
            name_lower,
            path,
            bytes: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }
    entries.sort_unstable_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.path.cmp(&a.path))
    });
    Ok(entries)
}

/// Decode a PNG file into top-down BGRA bytes and `[width, height]` at 1:1 resolution.
pub fn decode_bgra(path: &Path) -> Result<(Vec<u8>, [u32; 2]), String> {
    let (mut bytes, [w, h]) = decode_rgba(path)?;
    for px in bytes.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    Ok((bytes, [w as u32, h as u32]))
}

pub fn decode_rgba(path: &Path) -> Result<(Vec<u8>, [usize; 2]), String> {
    let read = || -> Result<_, Box<dyn std::error::Error>> {
        let file = std::io::BufReader::new(std::fs::File::open(path)?);
        let mut decoder = png::Decoder::new(file);
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        decoder.set_limits(png::Limits {
            bytes: 256 * 1024 * 1024,
        });
        let mut reader = decoder.read_info()?;
        let size = reader.output_buffer_size().ok_or("Image too large")?;
        let pixels = u64::from(reader.info().width) * u64::from(reader.info().height);
        if pixels == 0 || pixels > 40_000_000 || size > 256 * 1024 * 1024 {
            return Err("Image too large".into());
        }
        let mut buffer = vec![0; size];
        let info = reader.next_frame(&mut buffer)?;
        let bytes = &buffer[..info.buffer_size()];
        let out_len = pixels as usize * 4;
        let mut rgba = vec![0u8; out_len];
        match info.color_type {
            png::ColorType::Rgba => rgba.copy_from_slice(&bytes[..out_len]),
            png::ColorType::Rgb => {
                for (p, dst) in bytes
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .zip(rgba.as_chunks_mut::<4>().0)
                {
                    *dst = [p[0], p[1], p[2], 255];
                }
            }
            png::ColorType::Grayscale => {
                for (&v, dst) in bytes.iter().zip(rgba.as_chunks_mut::<4>().0) {
                    *dst = [v, v, v, 255];
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for (p, dst) in bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .zip(rgba.as_chunks_mut::<4>().0)
                {
                    *dst = [p[0], p[0], p[0], p[1]];
                }
            }
            _ => return Err("Unsupported PNG".into()),
        }
        Ok((rgba, [info.width as usize, info.height as usize]))
    };
    read().map_err(|_| {
        "Image unavailable. File may be damaged, moved, or exceed 40 megapixels.".into()
    })
}

/// Box-average area downsampler: preserves crisp text and UI lines without aliasing.
fn scaled_image(rgba: &[u8], size: [usize; 2], edge: usize) -> egui::ColorImage {
    let [sw, sh] = size;
    let scale = (edge as f64 / sw.max(sh).max(1) as f64).min(1.0);
    let width = (sw as f64 * scale).round().max(1.0) as usize;
    let height = (sh as f64 * scale).round().max(1.0) as usize;
    if width == sw && height == sh {
        let pixels = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect();
        return egui::ColorImage::new([width, height], pixels);
    }
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        let sy0 = y * sh / height;
        let sy1 = ((y + 1) * sh / height).max(sy0 + 1).min(sh);
        for x in 0..width {
            let sx0 = x * sw / width;
            let sx1 = ((x + 1) * sw / width).max(sx0 + 1).min(sw);
            let mut sums = [0u32; 4];
            let mut count = 0u32;
            for sy in sy0..sy1 {
                let row = sy * sw * 4;
                for sx in sx0..sx1 {
                    let i = row + sx * 4;
                    sums[0] += u32::from(rgba[i]);
                    sums[1] += u32::from(rgba[i + 1]);
                    sums[2] += u32::from(rgba[i + 2]);
                    sums[3] += u32::from(rgba[i + 3]);
                    count += 1;
                }
            }
            let c = count.max(1);
            pixels.push(egui::Color32::from_rgba_unmultiplied(
                (sums[0] / c) as u8,
                (sums[1] / c) as u8,
                (sums[2] / c) as u8,
                (sums[3] / c) as u8,
            ));
        }
    }
    egui::ColorImage::new([width, height], pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scales_without_upscaling_or_losing_aspect() {
        let rgba = vec![255; 800 * 400 * 4];
        assert_eq!(scaled_image(&rgba, [800, 400], 200).size, [200, 100]);
        assert_eq!(scaled_image(&[255; 8], [2, 1], 200).size, [2, 1]);
    }
    #[test]
    fn scans_png_only_and_accepts_missing_folder() {
        let dir = std::env::temp_dir().join(format!("xscreen-album-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.png"), b"invalid").unwrap();
        std::fs::write(dir.join("b.PNG"), b"invalid").unwrap();
        std::fs::write(dir.join("c.png.part"), b"partial").unwrap();
        let entries = scan(&dir).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].modified >= entries[1].modified);
        assert!(decode_rgba(&entries[0].path).is_err());
        assert!(scan(&dir.join("absent")).unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn decodes_real_png() {
        let dir = std::env::temp_dir().join(format!("xscreen-decode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("image.png");
        std::fs::write(
            &path,
            crate::native::encode_png(2, 1, &[0, 0, 255, 255, 0, 255, 0, 255]).unwrap(),
        )
        .unwrap();
        let (rgba, size) = decode_rgba(&path).unwrap();
        assert_eq!(size, [2, 1]);
        assert_eq!(&rgba[..4], &[255, 0, 0, 255]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
