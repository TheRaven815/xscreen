use crate::{
    capture::{Capture, CursorSnapshot, DesktopCapture, Frame, Region},
    config::Config,
    edit::{self, BlurKind, Editor, Shape, Tool},
    i18n::Strings,
    native::{Event, Hotkey, Native},
    studio::{self, ImageStudio, ShapeDrag},
};
use eframe::egui::{self, Color32, ViewportCommand, ViewportId};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::mpsc,
    thread,
};

#[path = "ui.rs"]
mod ui;

fn overlay_id(index: usize) -> ViewportId {
    ViewportId::from_hash_of(("capture", index))
}
fn settings_id() -> ViewportId {
    ViewportId::from_hash_of("settings")
}

const HANDLE_INK: Color32 = Color32::from_rgb(22, 25, 30);
const CAPTURE_RADIUS: u8 = 4;
const CAPTURE_ROW_H: f32 = 26.0;
const TOOLBAR_RADIUS: u8 = 5;
const TOOL_RADIUS: f32 = 3.5;
const TOOL_SIZE: f32 = 25.0;
const TOOLBAR_PAD: f32 = 3.0;
const SEP_W: f32 = 7.0;
const TOOLBAR_H: f32 = TOOL_SIZE + TOOLBAR_PAD * 2.0 + 2.0;
const LOUPE_RADIUS: f32 = 4.0;
const OVERLAY_BLEED: f32 = 1.0;

#[derive(Clone, Copy)]
pub(super) struct CapturePalette {
    pub light: bool,
    pub panel: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub field: Color32,
    pub hover: Color32,
    pub select: Color32,
    pub edge: Color32,
    pub hairline: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub primary: Color32,
    pub accent: Color32,
    pub accent_solid: Color32,
    pub err: Color32,
    pub popup: Color32,
    pub card: Color32,
    pub swatch_edge: Color32,
    pub shadow: Color32,
}

pub(super) fn capture_palette(light: bool) -> CapturePalette {
    let c = Color32::from_rgb;
    if light {
        CapturePalette {
            light,
            panel: c(255, 255, 255),
            surface: Color32::from_rgba_unmultiplied(252, 253, 255, 248),
            raised: c(255, 255, 255),
            field: c(243, 244, 246),
            hover: c(233, 236, 241),
            select: c(219, 234, 254),
            edge: c(208, 213, 221),
            hairline: Color32::from_rgba_unmultiplied(22, 25, 30, 36),
            text: c(22, 25, 30),
            muted: c(82, 88, 102),
            primary: c(16, 18, 22),
            accent: c(0, 103, 192),
            accent_solid: c(0, 103, 192),
            err: c(207, 34, 46),
            popup: Color32::from_rgba_unmultiplied(255, 255, 255, 252),
            card: Color32::from_rgba_unmultiplied(255, 255, 255, 248),
            swatch_edge: Color32::from_black_alpha(90),
            shadow: Color32::from_rgba_unmultiplied(16, 18, 22, 42),
        }
    } else {
        CapturePalette {
            light,
            panel: c(34, 36, 42),
            surface: Color32::from_rgba_unmultiplied(30, 32, 37, 248),
            raised: c(42, 45, 52),
            field: c(22, 23, 27),
            hover: c(52, 56, 65),
            select: c(30, 66, 104),
            edge: c(56, 60, 69),
            hairline: Color32::from_rgba_unmultiplied(255, 255, 255, 32),
            text: c(236, 238, 242),
            muted: c(148, 155, 166),
            primary: c(250, 251, 253),
            accent: c(64, 160, 245),
            accent_solid: c(14, 112, 192),
            err: c(229, 83, 75),
            popup: Color32::from_rgba_unmultiplied(34, 36, 42, 252),
            card: Color32::from_rgba_unmultiplied(32, 34, 40, 248),
            swatch_edge: Color32::from_white_alpha(92),
            shadow: Color32::from_black_alpha(132),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Album,
    Editor,
    Settings,
}

fn capture_visuals(p: CapturePalette) -> egui::Visuals {
    let mut visuals = if p.light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    visuals.override_text_color = Some(p.text);
    visuals.panel_fill = p.panel;
    visuals.window_fill = p.raised;
    visuals.extreme_bg_color = p.field;
    visuals.faint_bg_color = p.raised;
    visuals.window_stroke = egui::Stroke::new(1.0_f32, p.hairline);
    visuals.window_corner_radius = egui::CornerRadius::same(6);
    visuals.menu_corner_radius = egui::CornerRadius::same(6);
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.popup_shadow = egui::Shadow::NONE;
    visuals.selection.bg_fill = with_alpha(p.accent, 64);
    visuals.selection.stroke = egui::Stroke::new(1.5_f32, p.accent);
    let paint = |w: &mut egui::style::WidgetVisuals, fill, edge, text| {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = egui::Stroke::new(1.0_f32, edge);
        w.fg_stroke = egui::Stroke::new(1.0_f32, text);
        w.corner_radius = egui::CornerRadius::same(CAPTURE_RADIUS);
        w.expansion = 0.0;
    };
    let widgets = &mut visuals.widgets;
    paint(&mut widgets.noninteractive, p.panel, p.edge, p.text);
    paint(&mut widgets.inactive, p.raised, p.edge, p.text);
    paint(&mut widgets.hovered, p.hover, p.muted, p.primary);
    paint(&mut widgets.active, p.select, p.accent, p.primary);
    paint(&mut widgets.open, p.select, p.accent, p.text);
    visuals
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

#[derive(Clone, Debug)]
enum DragState {
    Idle,
    Creating {
        start: (i32, i32),
    },
    Moving {
        start_mouse: (i32, i32),
        origin: Region,
    },
    Resizing {
        handle: Handle,
        start_mouse: (i32, i32),
        origin: Region,
    },
}

fn hit_test_handle(sel: Region, px: i32, py: i32, threshold: i32) -> Option<Handle> {
    let handles = [
        (Handle::TopLeft, sel.x, sel.y),
        (Handle::Top, sel.x + sel.width as i32 / 2, sel.y),
        (Handle::TopRight, sel.x + sel.width as i32, sel.y),
        (
            Handle::Right,
            sel.x + sel.width as i32,
            sel.y + sel.height as i32 / 2,
        ),
        (
            Handle::BottomRight,
            sel.x + sel.width as i32,
            sel.y + sel.height as i32,
        ),
        (
            Handle::Bottom,
            sel.x + sel.width as i32 / 2,
            sel.y + sel.height as i32,
        ),
        (Handle::BottomLeft, sel.x, sel.y + sel.height as i32),
        (Handle::Left, sel.x, sel.y + sel.height as i32 / 2),
    ];
    for (handle, hx, hy) in handles {
        if (px - hx).abs() <= threshold && (py - hy).abs() <= threshold {
            return Some(handle);
        }
    }
    None
}

fn cursor_for_handle(handle: Handle) -> egui::CursorIcon {
    match handle {
        Handle::TopLeft | Handle::BottomRight => egui::CursorIcon::ResizeNwSe,
        Handle::TopRight | Handle::BottomLeft => egui::CursorIcon::ResizeNeSw,
        Handle::Top | Handle::Bottom => egui::CursorIcon::ResizeVertical,
        Handle::Left | Handle::Right => egui::CursorIcon::ResizeHorizontal,
    }
}

struct CapturedScreen {
    frame: Frame,
    image: egui::ColorImage,
}

struct Overlay {
    bounds: Region,
    texture: egui::TextureHandle,
    committed_bgra: Vec<u8>,
    blur_patches: Vec<egui::TextureHandle>,
    shapes_hash: u64,
    draft_blur: Option<(u64, egui::TextureHandle)>,
    shown: bool,
}

type CaptureResult = (u64, Result<(Vec<CapturedScreen>, u128), String>);

fn color_image_from_bgra(bgra: &[u8], bounds: Region) -> egui::ColorImage {
    let pixels = bgra
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| Color32::from_rgb(p[2], p[1], p[0]))
        .collect();
    egui::ColorImage::new([bounds.width as usize, bounds.height as usize], pixels)
}

fn color_image(frame: &Frame) -> egui::ColorImage {
    color_image_from_bgra(&frame.bgra, frame.bounds)
}

fn bgra_patch_to_color_image(patch: edit::BlurPatch) -> egui::ColorImage {
    let pixels = patch
        .bgra
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| Color32::from_rgb(p[2], p[1], p[0]))
        .collect();
    egui::ColorImage::new([patch.width as usize, patch.height as usize], pixels)
}

fn draft_blur_signature(shapes_hash: u64, a: (i32, i32), b: (i32, i32), kind: BlurKind) -> u64 {
    let mut hasher = DefaultHasher::new();
    shapes_hash.hash(&mut hasher);
    a.hash(&mut hasher);
    b.hash(&mut hasher);
    kind.hash(&mut hasher);
    hasher.finish()
}

/// Optimized blur synchronizer: skips full-frame BGRA copy and CPU rasterization when no blur shapes exist.
#[allow(clippy::too_many_arguments)]
fn sync_overlay_blurs(
    ctx: &egui::Context,
    index: usize,
    bounds: Region,
    raw_bgra: &[u8],
    committed_bgra: &mut Vec<u8>,
    blur_patches: &mut Vec<egui::TextureHandle>,
    shapes_hash: &mut u64,
    draft_blur: &mut Option<(u64, egui::TextureHandle)>,
    shapes: &[Shape],
    draft: Option<&Shape>,
) {
    let has_committed_blur = shapes.iter().any(|s| matches!(s, Shape::Blur { .. }));
    let has_draft_blur = matches!(draft, Some(Shape::Blur { .. }));

    if !has_committed_blur && !has_draft_blur {
        blur_patches.clear();
        *draft_blur = None;
        *shapes_hash = studio::shapes_hash_value(shapes);
        return;
    }

    let next_hash = studio::shapes_hash_value(shapes);
    if *shapes_hash != next_hash || committed_bgra.len() != raw_bgra.len() {
        if committed_bgra.len() != raw_bgra.len() {
            *committed_bgra = raw_bgra.to_vec();
        } else {
            committed_bgra.copy_from_slice(raw_bgra);
        }
        let patches = edit::apply_shapes_at_1x(
            committed_bgra,
            bounds.width as i32,
            bounds.height as i32,
            (bounds.x, bounds.y),
            shapes,
        );
        *blur_patches = patches
            .into_iter()
            .enumerate()
            .map(|(patch_idx, patch)| {
                ctx.load_texture(
                    format!("screen-blur-{index}-{patch_idx}-{next_hash:016x}"),
                    bgra_patch_to_color_image(patch),
                    egui::TextureOptions::NEAREST,
                )
            })
            .collect();
        *shapes_hash = next_hash;
    }

    if let Some(Shape::Blur { a, b, kind }) = draft {
        let d_hash = draft_blur_signature(next_hash, *a, *b, *kind);
        if draft_blur.as_ref().map(|(h, _)| *h) != Some(d_hash) {
            if let Some(patch) = edit::blur_patch_at_1x(
                committed_bgra,
                bounds.width as i32,
                bounds.height as i32,
                (bounds.x, bounds.y),
                *a,
                *b,
                *kind,
            ) {
                let image = bgra_patch_to_color_image(patch);
                if let Some((hash, tex)) = draft_blur {
                    tex.set(image, egui::TextureOptions::NEAREST);
                    *hash = d_hash;
                } else {
                    let tex = ctx.load_texture(
                        format!("screen-blur-draft-{index}"),
                        image,
                        egui::TextureOptions::NEAREST,
                    );
                    *draft_blur = Some((d_hash, tex));
                }
            } else {
                *draft_blur = None;
            }
        }
    } else if draft_blur.is_some() {
        *draft_blur = None;
    }
}

pub struct App {
    native: Native,
    events: mpsc::Receiver<Event>,
    frames: mpsc::Receiver<CaptureResult>,
    frame_sender: mpsc::Sender<CaptureResult>,
    generation: u64,
    pending: bool,
    overlay: Vec<Overlay>,
    raw_frames: Vec<Frame>,
    selection: Option<Region>,
    drag: DragState,
    overlay_shape_drag: Option<ShapeDrag>,
    editor: Editor,
    /// A tool (including `Tool::Select`) is armed on the capture overlay.
    editing: bool,
    capture_at: Option<std::time::Instant>,
    capture_ms: u128,
    pub(crate) settings: bool,
    hotkey: Option<Hotkey>,
    draft: Hotkey,
    pub(crate) config: Config,
    folder_draft: String,
    error: Option<String>,
    applying: bool,
    toolbar_size: (f32, f32),
    pub(crate) page: Page,
    pub(crate) album: crate::album::Album,
    pub(crate) studio: ImageStudio,
    editor_discard: Option<ui::EditorDiscardAction>,
    capture_after: Option<std::time::Instant>,
    restore_main_after_capture: bool,
    focus_main: bool,
    main_window_pos: Option<egui::Pos2>,
    restore_window_pos: Option<egui::Pos2>,
    applied_window_theme: Option<bool>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        ui::install_fonts(&cc.egui_ctx);
        let (sender, events) = mpsc::channel();
        let native = Native::start(cc.egui_ctx.clone(), sender)?;
        let _ = thread::Builder::new()
            .name("xscreen-capture-warmup".into())
            .spawn(crate::capture::prewarm);
        let (frame_sender, frames) = mpsc::channel();
        let config = Config::load();
        let folder_draft = config.save_dir.clone();
        Ok(Self {
            native,
            events,
            frames,
            frame_sender,
            generation: 0,
            pending: false,
            overlay: Vec::new(),
            raw_frames: Vec::new(),
            selection: None,
            drag: DragState::Idle,
            overlay_shape_drag: None,
            editor: Editor::default(),
            editing: false,
            capture_at: None,
            capture_ms: 0,
            settings: false,
            hotkey: None,
            draft: config.hotkey,
            config,
            folder_draft,
            error: None,
            applying: false,
            toolbar_size: (0.0, TOOLBAR_H),
            page: Page::Album,
            album: crate::album::Album::default(),
            studio: ImageStudio::default(),
            editor_discard: None,
            capture_after: None,
            restore_main_after_capture: false,
            focus_main: false,
            main_window_pos: None,
            restore_window_pos: None,
            applied_window_theme: None,
        })
    }

    fn cancel(&mut self) {
        self.capture_after = None;
        self.generation = self.generation.wrapping_add(1);
        self.pending = false;
        self.overlay.clear();
        self.raw_frames.clear();
        self.selection = None;
        self.drag = DragState::Idle;
        self.overlay_shape_drag = None;
        self.editor = Editor::default();
        self.editing = false;
        self.capture_at = None;
        if std::mem::take(&mut self.restore_main_after_capture) {
            self.settings = true;
            self.focus_main = true;
        }
    }

    /// Transfer active screen selection + live annotations directly into the In-App Image Editor!
    fn open_selection_in_editor(&mut self, ctx: &egui::Context) {
        let Some(selection) = self.selection.filter(|s| s.width >= 8 && s.height >= 8) else {
            return;
        };
        self.editor.commit_text();
        let composed = Frame::compose(&self.raw_frames, selection);
        let mut shapes = std::mem::take(&mut self.editor.shapes);
        let next_num = self.editor.next_number;
        for sh in &mut shapes {
            edit::translate_shape(sh, -selection.x, -selection.y);
        }
        let save_folder = self.config.save_folder();
        let lang = self.config.lang;
        self.restore_main_after_capture = false;
        self.cancel();
        self.studio.load_captured_bgra(
            composed.bgra,
            [composed.bounds.width, composed.bounds.height],
            shapes,
            next_num,
            save_folder,
            lang,
            ctx,
        );
        self.settings = true;
        self.page = Page::Editor;
        self.focus_main = true;
    }

    fn export_selection(&mut self, save: bool) {
        let Some(selection) = self.selection.filter(|s| s.width >= 8 && s.height >= 8) else {
            self.cancel();
            return;
        };
        self.editor.commit_text();
        let mut composed = Frame::compose(&self.raw_frames, selection);
        edit::bake(&mut composed.bgra, selection, &self.editor.shapes);
        let also_save = save || self.config.copy_and_save;
        let copy_png = self.config.copy_png;
        let folder = self.config.save_folder();
        let native = self.native.hwnd_bits();
        self.cancel();
        thread::spawn(move || {
            if let Err(error) = crate::native::copy_to_clipboard(&composed, copy_png) {
                tracing::error!(%error, "clipboard copy failed");
                crate::native::Native::post(native, crate::native::NOTICE);
                return;
            }
            if !also_save {
                crate::native::Native::post(native, crate::native::COPIED);
                return;
            }
            match crate::native::encode_png(
                composed.bounds.width,
                composed.bounds.height,
                &composed.bgra,
            )
            .and_then(|bytes| crate::native::save_png(&bytes, folder.as_deref()))
            {
                Ok(_) => crate::native::Native::post(native, crate::native::SAVED),
                Err(error) => {
                    tracing::error!(%error, "save failed");
                    crate::native::Native::post(native, crate::native::NOTICE);
                }
            }
        });
    }

    fn toggle(&mut self, ctx: &egui::Context) {
        if self.capture_after.is_some() || self.pending || !self.overlay.is_empty() {
            self.cancel();
            return;
        }
        if self.settings {
            self.restore_main_after_capture = true;
            self.settings = false;
            self.capture_after =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(180));
            ctx.request_repaint();
            return;
        }
        self.settings = false;
        self.pending = true;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let sender = self.frame_sender.clone();
        let ctx = ctx.clone();
        let include_cursor = self.config.include_cursor;
        thread::spawn(move || {
            let started = std::time::Instant::now();
            let capture = || -> Result<Vec<CapturedScreen>, String> {
                let cursor = if include_cursor {
                    CursorSnapshot::capture().map_err(|e| e.to_string())?
                } else {
                    None
                };
                let backend = DesktopCapture;
                let monitors = backend.monitors().map_err(|e| e.to_string())?;
                if monitors.is_empty() {
                    return Err("No active display.".into());
                }
                let frames = thread::scope(|scope| {
                    let workers: Vec<_> = monitors
                        .iter()
                        .map(|monitor| scope.spawn(move || DesktopCapture.frame(monitor)))
                        .collect();
                    workers
                        .into_iter()
                        .map(|worker| {
                            worker
                                .join()
                                .map_err(|_| "Capture thread stopped.".to_owned())?
                                .map_err(|e| e.to_string())
                        })
                        .collect::<Result<Vec<_>, String>>()
                })?;
                let current = backend.monitors().map_err(|e| e.to_string())?;
                if monitors.iter().map(|m| m.bounds).collect::<Vec<_>>()
                    != current.iter().map(|m| m.bounds).collect::<Vec<_>>()
                {
                    return Err("Display layout changed during capture. Try again.".into());
                }
                frames
                    .into_iter()
                    .map(|mut frame| {
                        if let Some(layer) =
                            cursor.as_ref().and_then(|c| c.layer(&frame).ok().flatten())
                        {
                            let dst_x = (layer.bounds.x - frame.bounds.x) as usize;
                            let dst_y = (layer.bounds.y - frame.bounds.y) as usize;
                            for row in 0..layer.bounds.height as usize {
                                let src_start = row * layer.bounds.width as usize * 4;
                                let src_end = src_start + layer.bounds.width as usize * 4;
                                let dst_start =
                                    ((dst_y + row) * frame.bounds.width as usize + dst_x) * 4;
                                let dst_end = dst_start + layer.bounds.width as usize * 4;
                                frame.bgra[dst_start..dst_end]
                                    .copy_from_slice(&layer.bgra[src_start..src_end]);
                            }
                        }
                        let image = color_image(&frame);
                        Ok(CapturedScreen { frame, image })
                    })
                    .collect()
            };
            let capture = capture().map(|screens| {
                let capture_ms = started.elapsed().as_millis();
                if capture_ms > 100 {
                    tracing::error!(capture_ms, "capture missed the 100 ms target");
                }
                (screens, capture_ms)
            });
            let _ = sender.send((generation, capture));
            ctx.request_repaint();
        });
    }

    fn persist(&mut self) {
        if let Err(error) = self.config.save() {
            tracing::error!(%error, "settings write failed");
            self.error = Some(
                crate::i18n::Strings::new(self.config.lang)
                    .err_save_settings()
                    .into(),
            );
        }
    }

    fn settings_ui(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let s = crate::i18n::Strings::new(self.config.lang);
        let page_title = match self.page {
            Page::Album => s.tab_album(),
            Page::Editor => s.tab_editor(),
            Page::Settings => s.tab_settings(),
        };
        let title = format!("xscreen — {page_title}");
        let mut builder = egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([1020.0, 660.0])
            .with_min_inner_size([680.0, 460.0])
            .with_resizable(true);
        if let Some(pos) = self.restore_window_pos {
            builder = builder.with_position(pos);
        }
        ctx.show_viewport_immediate(settings_id(), builder, |ctx, _| {
            let (outer, minimized) =
                ctx.input(|i| (i.viewport().outer_rect, i.viewport().minimized));
            if let Some(rect) = outer
                && minimized != Some(true)
                && rect.width() > 0.0
                && rect.height() > 0.0
            {
                self.main_window_pos = Some(rect.min);
            }
            if std::mem::take(&mut self.focus_main) {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
                self.album.refresh(self.config.save_folder(), ctx, true);
            }
            if self.applied_window_theme != Some(self.config.light_theme) {
                ctx.send_viewport_cmd(ViewportCommand::SetTheme(if self.config.light_theme {
                    egui::SystemTheme::Light
                } else {
                    egui::SystemTheme::Dark
                }));
                self.applied_window_theme = Some(self.config.light_theme);
            }
            if ctx.input(|i| i.viewport().close_requested()) {
                close = true;
            }
            ui::show(self, ctx);
        });
        if close {
            self.settings = false;
            self.editor_discard = None;
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Toggle => self.toggle(ctx),
                Event::Cancel => self.cancel(),
                Event::Settings => {
                    self.cancel();
                    self.settings = true;
                    self.page = Page::Settings;
                    self.config.run_at_startup = crate::startup::is_set();
                    self.draft = self.hotkey.unwrap_or(self.config.hotkey);
                    self.folder_draft = self.config.save_dir.clone();
                    self.focus_main = true;
                    ctx.send_viewport_cmd_to(settings_id(), ViewportCommand::Focus);
                }
                Event::Album => {
                    self.cancel();
                    self.settings = true;
                    self.page = Page::Album;
                    self.folder_draft = self.config.save_dir.clone();
                    self.draft = self.hotkey.unwrap_or(self.config.hotkey);
                    self.album.refresh(self.config.save_folder(), ctx, true);
                    self.focus_main = true;
                    ctx.send_viewport_cmd_to(settings_id(), ViewportCommand::Focus);
                }
                Event::Editor => {
                    self.cancel();
                    self.settings = true;
                    self.page = Page::Editor;
                    self.folder_draft = self.config.save_dir.clone();
                    self.draft = self.hotkey.unwrap_or(self.config.hotkey);
                    self.focus_main = true;
                    ctx.send_viewport_cmd_to(settings_id(), ViewportCommand::Focus);
                }
                Event::CaptureSaved => {
                    if self.settings {
                        self.album.refresh(self.config.save_folder(), ctx, true);
                    }
                }
                Event::Quit => ctx.send_viewport_cmd(ViewportCommand::Close),
                Event::Hotkey(hotkey) => {
                    self.hotkey = hotkey;
                    if self.applying {
                        if let Some(hotkey) = hotkey {
                            self.config.hotkey = hotkey;
                            self.persist();
                        }
                        self.applying = false;
                    }
                }
                Event::Error(error) => {
                    self.error = Some(error);
                    self.applying = false;
                }
            }
        }
        while let Ok((generation, frame)) = self.frames.try_recv() {
            if generation != self.generation {
                continue;
            }
            self.pending = false;
            match frame {
                Ok((screens, capture_ms)) => {
                    self.capture_at = Some(std::time::Instant::now());
                    self.capture_ms = capture_ms;
                    let mut raw_frames = Vec::with_capacity(screens.len());
                    let mut overlays = Vec::with_capacity(screens.len());
                    let initial_hash = studio::shapes_hash_value(&[]);
                    for (index, screen) in screens.into_iter().enumerate() {
                        let CapturedScreen { frame, image } = screen;
                        let bounds = frame.bounds;
                        raw_frames.push(frame);
                        overlays.push(Overlay {
                            bounds,
                            texture: ctx.load_texture(
                                format!("screen-{index}"),
                                image,
                                egui::TextureOptions::NEAREST,
                            ),
                            // Lazy allocation: only populated if user actually uses Blur tool!
                            committed_bgra: Vec::new(),
                            blur_patches: Vec::new(),
                            shapes_hash: initial_hash,
                            draft_blur: None,
                            shown: false,
                        });
                    }
                    self.raw_frames = raw_frames;
                    self.overlay = overlays;
                    self.selection = None;
                    self.drag = DragState::Idle;
                    self.overlay_shape_drag = None;
                    self.editor = Editor::default();
                    self.editing = false;
                }
                Err(error) => {
                    tracing::error!(%error, "capture failed");
                    self.cancel();
                    self.native.error();
                }
            }
        }
        if let Some(at) = self.capture_after {
            if std::time::Instant::now() >= at {
                self.capture_after = None;
                self.toggle(ctx);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(30));
            }
        }
        if self.settings {
            self.settings_ui(ctx);
        } else {
            self.restore_window_pos = self.main_window_pos;
            self.applied_window_theme = None;
        }
        let mut close = false;
        let mut copy = false;
        let mut save = false;
        let mut open_studio = false;
        let mut edit_action: Option<EditAction> = None;

        let cursor_phys = crate::native::cursor_pos();

        let mut targets_x = Vec::new();
        let mut targets_y = Vec::new();
        for ov in &self.overlay {
            targets_x.push(ov.bounds.x);
            targets_x.push(ov.bounds.x + ov.bounds.width as i32);
            targets_y.push(ov.bounds.y);
            targets_y.push(ov.bounds.y + ov.bounds.height as i32);
        }
        if let Some(win) = cursor_phys.and_then(crate::native::window_at) {
            targets_x.push(win.x);
            targets_x.push(win.x + win.width as i32);
            targets_y.push(win.y);
            targets_y.push(win.y + win.height as i32);
        }

        let (snapped_x, snapped_y) = if let Some((cx, cy)) = cursor_phys {
            (
                crate::native::snap_value(cx, &targets_x, 6),
                crate::native::snap_value(cy, &targets_y, 6),
            )
        } else {
            (0, 0)
        };

        if let Some(started) = self.capture_at.take() {
            let capture_ms = self.capture_ms;
            let overlay_ms = capture_ms + started.elapsed().as_millis();
            if overlay_ms > 150 {
                tracing::error!(
                    capture_ms,
                    overlay_ms,
                    "overlay first frame missed the 150 ms target"
                );
            }
        }

        let i18n = Strings::new(self.config.lang);

        for (index, overlay) in self.overlay.iter_mut().enumerate() {
            let Overlay {
                bounds,
                texture,
                committed_bgra,
                blur_patches,
                shapes_hash,
                draft_blur,
                shown,
            } = &mut *overlay;
            let title = format!("xscreen · Display {} · Capture", index + 1);
            let scale = crate::native::monitor_scale(*bounds);
            let mut want_precision_cursor = false;
            ctx.show_viewport_immediate(
                overlay_id(index),
                egui::ViewportBuilder::default()
                    .with_title(&title)
                    .with_decorations(false)
                    .with_transparent(false)
                    .with_resizable(false)
                    .with_position([bounds.x as f32 / scale, bounds.y as f32 / scale])
                    .with_inner_size([
                        (bounds.width as f32 + OVERLAY_BLEED) / scale,
                        (bounds.height as f32 + OVERLAY_BLEED) / scale,
                    ])
                    .with_taskbar(false)
                    .with_window_level(egui::WindowLevel::AlwaysOnTop)
                    .with_visible(false),
                |viewport_ctx, _| {
                    let text_layout = edit::TextLayout {
                        ctx: viewport_ctx,
                        px_to_ui: 1.0 / scale,
                    };
                    let popup_was_open = egui::Popup::is_any_open(viewport_ctx);
                    let focused = viewport_ctx.input(|i| i.focused);
                    if !focused {
                        viewport_ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    }
                    let palette = capture_palette(self.config.light_theme);
                    viewport_ctx.set_visuals(capture_visuals(palette));
                    viewport_ctx.style_mut(|style| {
                        style.spacing.button_padding = egui::vec2(6.0, 3.0);
                        style.spacing.interact_size.y = CAPTURE_ROW_H;
                        style.spacing.item_spacing = egui::vec2(4.0, 4.0);
                    });
                    if viewport_ctx.input(|i| i.viewport().close_requested()) {
                        close = true;
                    }
                    if focused
                        && !popup_was_open
                        && viewport_ctx.input(|i| i.key_pressed(egui::Key::Escape))
                    {
                        if self.editor.text_at.is_some() {
                            self.editor.commit_text();
                            self.editing = false;
                        } else if self.editor.draft.is_some() {
                            self.editor.cancel_draft();
                            self.editing = false;
                        } else if self.editor.selected.is_some() {
                            self.editor.select_shape_index(None);
                        } else if self.editing {
                            self.editing = false;
                        } else {
                            close = true;
                        }
                    }
                    if focused
                        && viewport_ctx.input(|i| i.key_pressed(egui::Key::Enter))
                        && !popup_was_open
                        && self.editor.text_at.is_none()
                    {
                        copy = true;
                    }
                    if focused && self.editor.text_at.is_none() {
                        if viewport_ctx.input(|i| {
                            i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::S)
                        }) {
                            save = true;
                        }
                        if viewport_ctx.input(|i| {
                            i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::E)
                        }) {
                            open_studio = true;
                        }
                        if viewport_ctx.input(|i| {
                            i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                        }) && self.editor.selected.is_some()
                        {
                            edit_action = Some(EditAction::DeleteSelected);
                        }
                        if viewport_ctx.input(|i| {
                            i.modifiers.command && i.key_pressed(egui::Key::Z) && !i.modifiers.shift
                        }) {
                            edit_action = Some(EditAction::Undo);
                        }
                        if self.editor.can_redo()
                            && viewport_ctx.input(|i| {
                                i.modifiers.command
                                    && (i.key_pressed(egui::Key::Y)
                                        || (i.key_pressed(egui::Key::Z) && i.modifiers.shift))
                            })
                        {
                            edit_action = Some(EditAction::Redo);
                        }
                    }

                    let alt = viewport_ctx.input(|i| i.modifiers.alt);
                    let ui_hit = capture_ui_hit(viewport_ctx, popup_was_open);
                    let primary_pressed = viewport_ctx.input(|i| i.pointer.primary_pressed());
                    let secondary_clicked = viewport_ctx.input(|i| i.pointer.secondary_clicked());
                    let primary_down = viewport_ctx.input(|i| i.pointer.primary_down());
                    let primary_released = viewport_ctx.input(|i| i.pointer.primary_released());
                    let scroll = viewport_ctx.input(|i| i.raw_scroll_delta.y);
                    let double_click = viewport_ctx.input(|i| {
                        i.pointer
                            .button_double_clicked(egui::PointerButton::Primary)
                    });

                    let is_cursor_on_this_monitor = if let Some((cx, cy)) = cursor_phys {
                        bounds.x <= cx
                            && cx < bounds.x + bounds.width as i32
                            && bounds.y <= cy
                            && cy < bounds.y + bounds.height as i32
                    } else {
                        false
                    };

                    let mut hovered_obj_handle = None;

                    if is_cursor_on_this_monitor
                        && !ui_hit
                        && let Some((cx, cy)) = cursor_phys
                    {
                        let hovered_sel_handle =
                            self.selection.and_then(|s| hit_test_handle(s, cx, cy, 8));
                        let is_inside_sel = self.selection.is_some_and(|s| {
                            s.x <= cx
                                && cx < s.x + s.width as i32
                                && s.y <= cy
                                && cy < s.y + s.height as i32
                        });

                        // Right-click any drawn shape on overlay to select it immediately!
                        let hit_tol = (6.5 * scale).max(4.0);
                        if secondary_clicked
                            && is_inside_sel
                            && let Some(hit_idx) =
                                edit::hit_shape(&self.editor.shapes, (cx, cy), hit_tol, text_layout)
                        {
                            self.editor.commit_text();
                            self.editor.select_tool(Tool::Select);
                            self.editor.select_shape_index(Some(hit_idx));
                            self.editing = true;
                        }

                        hovered_obj_handle = self
                            .editor
                            .selected
                            .and_then(|i| self.editor.shapes.get(i))
                            .and_then(|sh| {
                                edit::hit_shape_handle(
                                    sh,
                                    (cx, cy),
                                    (8.0 * scale) as i32,
                                    text_layout,
                                )
                            });
                        let hovered_obj =
                            edit::hit_shape(&self.editor.shapes, (cx, cy), hit_tol, text_layout);

                        if let Some(oh) = hovered_obj_handle {
                            viewport_ctx.set_cursor_icon(studio::cursor_for_shape_handle(oh));
                        } else if let Some(handle) = hovered_sel_handle {
                            viewport_ctx.set_cursor_icon(cursor_for_handle(handle));
                        } else if self.editing {
                            if self.editor.tool == Tool::Select {
                                viewport_ctx.set_cursor_icon(
                                    if hovered_obj.is_some() || self.overlay_shape_drag.is_some() {
                                        egui::CursorIcon::Move
                                    } else {
                                        egui::CursorIcon::Default
                                    },
                                );
                            } else {
                                let over_text = self.editor.tool == Tool::Text
                                    && edit::hit_text(&self.editor.shapes, (cx, cy), text_layout)
                                        .is_some();
                                if over_text {
                                    viewport_ctx.set_cursor_icon(egui::CursorIcon::Move);
                                } else if self.editor.tool == Tool::Text {
                                    viewport_ctx.set_cursor_icon(egui::CursorIcon::Text);
                                } else {
                                    viewport_ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                                    want_precision_cursor = true;
                                }
                            }
                        } else if is_inside_sel
                            && matches!(self.drag, DragState::Idle | DragState::Moving { .. })
                        {
                            viewport_ctx.set_cursor_icon(egui::CursorIcon::Move);
                        } else {
                            viewport_ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                            want_precision_cursor = true;
                        }

                        if primary_pressed {
                            if let (Some(oh), Some(sel_idx)) =
                                (hovered_obj_handle, self.editor.selected)
                            {
                                self.editor.commit_text();
                                let origin = self.editor.shapes[sel_idx].clone();
                                self.overlay_shape_drag = Some(ShapeDrag::Resizing {
                                    index: sel_idx,
                                    handle: oh,
                                    start_mouse: (cx, cy),
                                    origin,
                                    moved: false,
                                });
                            } else if let Some(handle) = hovered_sel_handle {
                                if let Some(origin) = self.selection {
                                    self.editor.commit_text();
                                    self.editing = false;
                                    self.drag = DragState::Resizing {
                                        handle,
                                        start_mouse: (snapped_x, snapped_y),
                                        origin,
                                    };
                                }
                            } else if self.editing && is_inside_sel {
                                match self.editor.tool {
                                    Tool::Select => {
                                        self.editor.commit_text();
                                        if double_click
                                            && let Some(idx) = hovered_obj
                                            && matches!(
                                                self.editor.shapes.get(idx),
                                                Some(Shape::Text { .. })
                                            )
                                        {
                                            self.editor.begin_edit_existing_text(idx);
                                        } else if let Some(idx) = hovered_obj {
                                            self.editor.select_shape_index(Some(idx));
                                            self.overlay_shape_drag = Some(ShapeDrag::Moving {
                                                index: idx,
                                                last_mouse: (cx, cy),
                                                moved: false,
                                            });
                                        } else {
                                            self.editor.select_shape_index(None);
                                        }
                                    }
                                    Tool::Number => self.editor.place_number((cx, cy)),
                                    Tool::Text => {
                                        let had_text = self.editor.text_at.is_some();
                                        self.editor.commit_text();
                                        if let Some(index) = edit::hit_text(
                                            &self.editor.shapes,
                                            (cx, cy),
                                            text_layout,
                                        ) {
                                            if double_click {
                                                self.editor.begin_edit_existing_text(index);
                                            } else {
                                                self.editor.select_shape_index(Some(index));
                                                self.overlay_shape_drag = Some(ShapeDrag::Moving {
                                                    index,
                                                    last_mouse: (cx, cy),
                                                    moved: false,
                                                });
                                            }
                                        } else if !had_text {
                                            self.editor.begin_text((cx, cy));
                                        }
                                    }
                                    tool if tool.is_drag() => self.editor.begin((cx, cy)),
                                    _ => {}
                                }
                            } else if double_click && is_inside_sel {
                                copy = true;
                            } else if alt {
                                if let Some(win) = crate::native::window_at((cx, cy)) {
                                    self.selection = Some(win);
                                    self.drag = DragState::Idle;
                                    self.editing = false;
                                }
                            } else if is_inside_sel {
                                self.drag = DragState::Moving {
                                    start_mouse: (snapped_x, snapped_y),
                                    origin: self.selection.unwrap(),
                                };
                            } else {
                                self.drag = DragState::Creating {
                                    start: (snapped_x, snapped_y),
                                };
                                self.selection = None;
                                self.editing = false;
                            }
                        }
                    }

                    if self.editing && primary_down && self.editor.draft.is_some() {
                        if let Some((cx, cy)) = cursor_phys {
                            self.editor.extend((cx, cy));
                        }
                        viewport_ctx.request_repaint();
                    }

                    if primary_down
                        && let Some(sd) = &mut self.overlay_shape_drag
                        && let Some((cx, cy)) = cursor_phys
                    {
                        match sd {
                            ShapeDrag::Moving {
                                index,
                                last_mouse,
                                moved,
                            } => {
                                let dx = cx - last_mouse.0;
                                let dy = cy - last_mouse.1;
                                if dx != 0 || dy != 0 {
                                    if !*moved {
                                        self.editor.record_snapshot();
                                        *moved = true;
                                    }
                                    if let Some(sh) = self.editor.shapes.get_mut(*index) {
                                        edit::translate_shape(sh, dx, dy);
                                        *last_mouse = (cx, cy);
                                    }
                                }
                            }
                            ShapeDrag::Resizing {
                                index,
                                handle,
                                start_mouse,
                                origin,
                                moved,
                            } => {
                                if cx != start_mouse.0 || cy != start_mouse.1 {
                                    if !*moved {
                                        self.editor.record_snapshot();
                                        *moved = true;
                                    }
                                    let updated = edit::resize_shape_from_origin(
                                        origin,
                                        *handle,
                                        *start_mouse,
                                        (cx, cy),
                                        text_layout,
                                    );
                                    if let Some(sh) = self.editor.shapes.get_mut(*index) {
                                        *sh = updated;
                                    }
                                }
                            }
                        }
                        viewport_ctx.request_repaint();
                    }

                    if primary_released && self.overlay_shape_drag.take().is_some() {
                        self.editor.break_coalesce();
                    }

                    let width_tool = matches!(
                        self.editor.tool,
                        Tool::Select
                            | Tool::Arrow
                            | Tool::Line
                            | Tool::Rect
                            | Tool::Ellipse
                            | Tool::Triangle
                            | Tool::Pen
                            | Tool::Text
                    );
                    if self.editing
                        && width_tool
                        && is_cursor_on_this_monitor
                        && !popup_was_open
                        && !egui::Popup::is_any_open(viewport_ctx)
                        && (!ui_hit || self.editor.text_at.is_some())
                        && scroll != 0.0
                    {
                        let next = (self.editor.width + scroll.signum()).clamp(1.0, 24.0);
                        self.editor.set_width(next);
                        viewport_ctx.request_repaint();
                    }

                    if primary_down && !matches!(self.drag, DragState::Idle) && !self.editing {
                        viewport_ctx.request_repaint();
                        match self.drag {
                            DragState::Creating { start } => {
                                let left = start.0.min(snapped_x);
                                let right = start.0.max(snapped_x);
                                let top = start.1.min(snapped_y);
                                let bottom = start.1.max(snapped_y);
                                self.selection = Some(Region {
                                    x: left,
                                    y: top,
                                    width: (right - left) as u32,
                                    height: (bottom - top) as u32,
                                });
                            }
                            DragState::Moving {
                                start_mouse,
                                origin,
                            } => {
                                let dx = snapped_x - start_mouse.0;
                                let dy = snapped_y - start_mouse.1;
                                self.selection = Some(Region {
                                    x: origin.x + dx,
                                    y: origin.y + dy,
                                    width: origin.width,
                                    height: origin.height,
                                });
                            }
                            DragState::Resizing {
                                handle,
                                start_mouse,
                                origin,
                            } => {
                                let dx = snapped_x - start_mouse.0;
                                let dy = snapped_y - start_mouse.1;
                                let mut l = origin.x;
                                let mut r = origin.x + origin.width as i32;
                                let mut t = origin.y;
                                let mut b = origin.y + origin.height as i32;
                                match handle {
                                    Handle::TopLeft => {
                                        l += dx;
                                        t += dy;
                                    }
                                    Handle::Top => t += dy,
                                    Handle::TopRight => {
                                        r += dx;
                                        t += dy;
                                    }
                                    Handle::Right => r += dx,
                                    Handle::BottomRight => {
                                        r += dx;
                                        b += dy;
                                    }
                                    Handle::Bottom => b += dy,
                                    Handle::BottomLeft => {
                                        l += dx;
                                        b += dy;
                                    }
                                    Handle::Left => l += dx,
                                }
                                let min_x = l.min(r);
                                let max_x = l.max(r);
                                let min_y = t.min(b);
                                let max_y = t.max(b);
                                self.selection = Some(Region {
                                    x: min_x,
                                    y: min_y,
                                    width: (max_x - min_x) as u32,
                                    height: (max_y - min_y) as u32,
                                });
                            }
                            DragState::Idle => {}
                        }
                    }

                    if primary_released && self.editor.draft.is_some() {
                        self.editor.commit();
                        viewport_ctx.request_repaint();
                    }

                    if primary_released && !matches!(self.drag, DragState::Idle) {
                        if self.selection.is_some_and(|s| s.width < 8 || s.height < 8) {
                            self.selection = None;
                            self.editing = false;
                        }
                        self.drag = DragState::Idle;
                        viewport_ctx.request_repaint();
                    }

                    if let Some(raw_frame) = self.raw_frames.get(index) {
                        sync_overlay_blurs(
                            viewport_ctx,
                            index,
                            *bounds,
                            &raw_frame.bgra,
                            committed_bgra,
                            blur_patches,
                            shapes_hash,
                            draft_blur,
                            &self.editor.shapes,
                            self.editor.draft.as_ref(),
                        );
                    }

                    let active_selection =
                        match (alt && matches!(self.drag, DragState::Idle), cursor_phys) {
                            (true, Some(pt)) => crate::native::window_at(pt).or(self.selection),
                            _ => self.selection,
                        };
                    let hovered_handle = if !alt
                        && self.editor.text_at.is_none()
                        && matches!(self.drag, DragState::Idle)
                        && is_cursor_on_this_monitor
                        && !ui_hit
                    {
                        cursor_phys.and_then(|(cx, cy)| {
                            self.selection.and_then(|s| hit_test_handle(s, cx, cy, 8))
                        })
                    } else {
                        None
                    };
                    let toolbar_placement = if matches!(self.drag, DragState::Idle) && !alt {
                        self.selection
                            .filter(|s| s.width >= 8 && s.height >= 8)
                            .and_then(|sel| {
                                let local = (
                                    (sel.x - bounds.x) as f32 / scale,
                                    (sel.y - bounds.y) as f32 / scale,
                                    sel.width as f32 / scale,
                                    sel.height as f32 / scale,
                                );
                                let screen =
                                    (bounds.width as f32 / scale, bounds.height as f32 / scale);
                                let gap = 8.0;
                                let bar_h = self.toolbar_size.1.max(TOOLBAR_H);
                                let ((ax, ay), above) =
                                    edit::toolbar_anchor(local, screen, bar_h, gap);
                                let on_this = local.0 < screen.0
                                    && local.0 + local.2 > 0.0
                                    && local.1 < screen.1
                                    && local.1 + local.3 > 0.0
                                    && ay >= 0.0
                                    && ay < screen.1;
                                on_this.then(|| {
                                    let bar_w = if self.toolbar_size.0 > 1.0 {
                                        self.toolbar_size.0
                                    } else {
                                        430.0
                                    };
                                    let max_x = ax.clamp(bar_w.min(screen.0), screen.0);
                                    let min_x = (max_x - bar_w).max(0.0);
                                    let min_y = if above {
                                        (ay - bar_h).max(0.0)
                                    } else {
                                        ay.min((screen.1 - bar_h).max(0.0))
                                    };
                                    let rect = egui::Rect::from_min_size(
                                        egui::pos2(min_x, min_y),
                                        egui::vec2(bar_w, bar_h),
                                    );
                                    (ax, ay, above, rect)
                                })
                            })
                    } else {
                        None
                    };

                    egui::CentralPanel::default().frame(egui::Frame::NONE).show(
                        viewport_ctx,
                        |ui| {
                            viewport_ctx
                                .style_mut(|style| style.visuals.panel_fill = Color32::TRANSPARENT);
                            let screen_rect = egui::Rect::from_min_size(
                                ui.max_rect().min,
                                egui::vec2(
                                    bounds.width as f32 / scale,
                                    bounds.height as f32 / scale,
                                ),
                            );
                            ui.painter().image(
                                texture.id(),
                                screen_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                Color32::WHITE,
                            );

                            let scrim_tint = Color32::from_rgba_unmultiplied(10, 12, 16, 148);
                            let intersection = active_selection.and_then(|sel| {
                                let left = sel.x.max(bounds.x);
                                let top = sel.y.max(bounds.y);
                                let right =
                                    (sel.x + sel.width as i32).min(bounds.x + bounds.width as i32);
                                let bottom = (sel.y + sel.height as i32)
                                    .min(bounds.y + bounds.height as i32);
                                if right > left && bottom > top {
                                    Some(egui::Rect::from_min_max(
                                        egui::pos2(
                                            (left - bounds.x) as f32 / scale,
                                            (top - bounds.y) as f32 / scale,
                                        ),
                                        egui::pos2(
                                            (right - bounds.x) as f32 / scale,
                                            (bottom - bounds.y) as f32 / scale,
                                        ),
                                    ))
                                } else {
                                    None
                                }
                            });

                            let mut drawn_badge_rect = None;
                            if let Some(local_rect) = intersection {
                                if local_rect.min.y > screen_rect.min.y {
                                    ui.painter().rect_filled(
                                        egui::Rect::from_min_max(
                                            screen_rect.min,
                                            egui::pos2(screen_rect.max.x, local_rect.min.y),
                                        ),
                                        0.0,
                                        scrim_tint,
                                    );
                                }
                                if local_rect.max.y < screen_rect.max.y {
                                    ui.painter().rect_filled(
                                        egui::Rect::from_min_max(
                                            egui::pos2(screen_rect.min.x, local_rect.max.y),
                                            screen_rect.max,
                                        ),
                                        0.0,
                                        scrim_tint,
                                    );
                                }
                                if local_rect.min.x > screen_rect.min.x {
                                    ui.painter().rect_filled(
                                        egui::Rect::from_min_max(
                                            egui::pos2(screen_rect.min.x, local_rect.min.y),
                                            egui::pos2(local_rect.min.x, local_rect.max.y),
                                        ),
                                        0.0,
                                        scrim_tint,
                                    );
                                }
                                if local_rect.max.x < screen_rect.max.x {
                                    ui.painter().rect_filled(
                                        egui::Rect::from_min_max(
                                            egui::pos2(local_rect.max.x, local_rect.min.y),
                                            egui::pos2(screen_rect.max.x, local_rect.max.y),
                                        ),
                                        0.0,
                                        scrim_tint,
                                    );
                                }

                                let window_pick = alt && matches!(self.drag, DragState::Idle);
                                if window_pick {
                                    ui.painter().rect_filled(
                                        local_rect,
                                        0.0,
                                        with_alpha(palette.accent, 22),
                                    );
                                    ui.painter().rect_stroke(
                                        local_rect.expand(0.5),
                                        0.0,
                                        egui::Stroke::new(1.0_f32, Color32::from_black_alpha(120)),
                                        egui::StrokeKind::Outside,
                                    );
                                    ui.painter().rect_stroke(
                                        local_rect,
                                        0.0,
                                        egui::Stroke::new(1.5_f32, Color32::WHITE),
                                        egui::StrokeKind::Inside,
                                    );
                                } else {
                                    ui.painter().rect_stroke(
                                        local_rect.expand(0.5),
                                        0.0,
                                        egui::Stroke::new(1.0_f32, Color32::from_black_alpha(115)),
                                        egui::StrokeKind::Outside,
                                    );
                                    ui.painter().rect_stroke(
                                        local_rect,
                                        0.0,
                                        egui::Stroke::new(
                                            1.0_f32,
                                            Color32::from_rgba_unmultiplied(255, 255, 255, 238),
                                        ),
                                        egui::StrokeKind::Inside,
                                    );
                                }

                                if let Some(sel) =
                                    active_selection.filter(|s| s.width >= 8 && s.height >= 8)
                                {
                                    let (badge_text, font, left_pad) = if window_pick {
                                        (
                                            crate::i18n::Strings::new(self.config.lang)
                                                .window_pick_badge(sel.width, sel.height),
                                            egui::FontId::proportional(11.5),
                                            16.0,
                                        )
                                    } else {
                                        (
                                            format!("{} × {}", sel.width, sel.height),
                                            egui::FontId::monospace(11.5),
                                            8.0,
                                        )
                                    };
                                    let galley =
                                        ui.painter().layout_no_wrap(badge_text, font, palette.text);
                                    let badge_size =
                                        galley.size() + egui::vec2(left_pad + 8.0, 6.0);
                                    let badge_pos = dimension_badge_pos(
                                        local_rect,
                                        badge_size,
                                        screen_rect,
                                        toolbar_placement.map(|(_, _, _, r)| r),
                                    );
                                    let badge_rect =
                                        egui::Rect::from_min_size(badge_pos, badge_size);
                                    drawn_badge_rect = Some(badge_rect);
                                    paint_floating_shadow(
                                        ui.painter(),
                                        badge_rect,
                                        4,
                                        palette,
                                        true,
                                    );
                                    ui.painter().rect_filled(badge_rect, 4.0, palette.surface);
                                    ui.painter().rect_stroke(
                                        badge_rect,
                                        4.0_f32,
                                        egui::Stroke::new(1.0_f32, palette.hairline),
                                        egui::StrokeKind::Inside,
                                    );
                                    if window_pick {
                                        ui.painter().circle_filled(
                                            egui::pos2(
                                                badge_rect.left() + 9.0,
                                                badge_rect.center().y,
                                            ),
                                            2.5,
                                            palette.accent,
                                        );
                                    }
                                    ui.painter().galley(
                                        badge_rect.min + egui::vec2(left_pad, 3.0),
                                        galley,
                                        palette.text,
                                    );
                                }
                            } else {
                                ui.painter().rect_filled(screen_rect, 0.0, scrim_tint);
                            }

                            paint_shapes_view(
                                ui.painter(),
                                bounds,
                                egui::Pos2::ZERO,
                                1.0 / scale,
                                &self.editor.shapes,
                                self.editor.editing_text_index,
                                self.editor.draft.as_ref(),
                                blur_patches,
                                draft_blur.as_ref().map(|(_, t)| t),
                            );

                            if let Some(Shape::Blur { a, b, .. }) = &self.editor.draft
                                && let Some(r) = intersect(*bounds, edit::norm_rect(*a, *b))
                            {
                                let rect = egui::Rect::from_min_max(
                                    to_local(bounds, scale, (r.x, r.y)),
                                    to_local(
                                        bounds,
                                        scale,
                                        (r.x + r.width as i32, r.y + r.height as i32),
                                    ),
                                );
                                studio::paint_dashed_rect(ui.painter(), rect, palette.accent);
                            }

                            // Selected shape dashed outline & circular handles right on the Capture Overlay!
                            if self.editor.text_at.is_none()
                                && let Some(sel_idx) = self.editor.selected
                                && let Some(sel_shape) = self.editor.shapes.get(sel_idx)
                            {
                                studio::paint_shape_selection(
                                    ui.painter(),
                                    sel_shape,
                                    hovered_obj_handle,
                                    palette.accent,
                                    text_layout,
                                    |pt| to_local(bounds, scale, pt),
                                );
                            }

                            if is_cursor_on_this_monitor
                                && self.editing
                                && !ui_hit
                                && let Some((cx, cy)) = cursor_phys
                            {
                                let diameter = match self.editor.tool {
                                    Tool::Text => self.editor.text_size() / scale,
                                    Tool::Arrow
                                    | Tool::Line
                                    | Tool::Rect
                                    | Tool::Ellipse
                                    | Tool::Triangle
                                    | Tool::Pen => self.editor.width / scale,
                                    Tool::Marker => edit::MARKER_WIDTH / scale,
                                    Tool::Select | Tool::Number | Tool::Blur => 0.0,
                                };
                                if diameter > 0.0 {
                                    let center = egui::pos2(
                                        (cx - bounds.x) as f32 / scale,
                                        (cy - bounds.y) as f32 / scale,
                                    );
                                    let radius = (diameter * 0.5).clamp(2.0, 64.0);
                                    ui.painter().circle_stroke(
                                        center,
                                        radius + 1.0,
                                        egui::Stroke::new(1.5_f32, Color32::from_black_alpha(180)),
                                    );
                                    ui.painter().circle_stroke(
                                        center,
                                        radius,
                                        egui::Stroke::new(1.0_f32, Color32::WHITE),
                                    );
                                    ui.painter().circle_filled(center, 1.0, Color32::WHITE);
                                }
                            }

                            // Selection resize handles
                            if !alt
                                && self.editor.text_at.is_none()
                                && matches!(self.drag, DragState::Idle)
                                && let Some(sel) = self.selection
                            {
                                let handles = [
                                    (Handle::TopLeft, sel.x, sel.y),
                                    (Handle::Top, sel.x + sel.width as i32 / 2, sel.y),
                                    (Handle::TopRight, sel.x + sel.width as i32, sel.y),
                                    (
                                        Handle::Right,
                                        sel.x + sel.width as i32,
                                        sel.y + sel.height as i32 / 2,
                                    ),
                                    (
                                        Handle::BottomRight,
                                        sel.x + sel.width as i32,
                                        sel.y + sel.height as i32,
                                    ),
                                    (
                                        Handle::Bottom,
                                        sel.x + sel.width as i32 / 2,
                                        sel.y + sel.height as i32,
                                    ),
                                    (Handle::BottomLeft, sel.x, sel.y + sel.height as i32),
                                    (Handle::Left, sel.x, sel.y + sel.height as i32 / 2),
                                ];
                                for (kind, hx, hy) in handles {
                                    if bounds.x <= hx
                                        && hx <= bounds.x + bounds.width as i32
                                        && bounds.y <= hy
                                        && hy <= bounds.y + bounds.height as i32
                                    {
                                        let pt = egui::pos2(
                                            (hx - bounds.x) as f32 / scale,
                                            (hy - bounds.y) as f32 / scale,
                                        );
                                        let hot = hovered_handle == Some(kind);
                                        let r = if hot { 4.6 } else { 3.7 };
                                        ui.painter().circle_stroke(
                                            pt,
                                            r + 0.9,
                                            egui::Stroke::new(
                                                1.5_f32,
                                                Color32::from_black_alpha(155),
                                            ),
                                        );
                                        ui.painter().circle_filled(pt, r, Color32::WHITE);
                                        ui.painter().circle_stroke(
                                            pt,
                                            r,
                                            egui::Stroke::new(
                                                if hot { 1.5_f32 } else { 1.0_f32 },
                                                if hot { palette.accent } else { HANDLE_INK },
                                            ),
                                        );
                                    }
                                }
                            }

                            // Optional compact pixel magnifier (opt-in from Settings)
                            if self.config.show_magnifier
                                && !ui_hit
                                && is_cursor_on_this_monitor
                                && let Some((cx, cy)) = cursor_phys
                            {
                                let show_magnifier = match self.drag {
                                    DragState::Creating { .. } | DragState::Resizing { .. } => true,
                                    DragState::Idle => self.selection.is_none() && !alt,
                                    _ => false,
                                };
                                if show_magnifier {
                                    let px_x = cx - bounds.x;
                                    let px_y = cy - bounds.y;
                                    let rgb = self
                                        .raw_frames
                                        .iter()
                                        .find(|f| f.bounds == *bounds)
                                        .and_then(|f| {
                                            if px_x >= 0
                                                && px_x < bounds.width as i32
                                                && px_y >= 0
                                                && px_y < bounds.height as i32
                                            {
                                                let idx = (px_y as usize * bounds.width as usize
                                                    + px_x as usize)
                                                    * 4;
                                                Some((
                                                    f.bgra[idx + 2],
                                                    f.bgra[idx + 1],
                                                    f.bgra[idx],
                                                ))
                                            } else {
                                                None
                                            }
                                        })
                                        .unwrap_or((0, 0, 0));
                                    paint_compact_magnifier(
                                        ui.painter(),
                                        texture.id(),
                                        *bounds,
                                        scale,
                                        (cx, cy),
                                        rgb,
                                        screen_rect,
                                        drawn_badge_rect,
                                        palette,
                                    );
                                }
                            }
                        },
                    );

                    // Capture Floating Toolbar
                    if matches!(self.drag, DragState::Idle)
                        && self
                            .selection
                            .is_some_and(|s| s.width >= 8 && s.height >= 8)
                    {
                        let screen = (bounds.width as f32 / scale, bounds.height as f32 / scale);
                        if let Some((ax, ay, above, _)) = toolbar_placement {
                            let tools = egui::Area::new(egui::Id::new(("tools", index)))
                                .order(egui::Order::Foreground)
                                .movable(false)
                                .fixed_pos(egui::pos2(ax, ay))
                                .pivot(if above {
                                    egui::Align2::RIGHT_BOTTOM
                                } else {
                                    egui::Align2::RIGHT_TOP
                                })
                                .show(viewport_ctx, |ui| {
                                    let shadow_idx = ui.painter().add(egui::Shape::Noop);
                                    let resp = floating_surface_frame(
                                        palette,
                                        TOOLBAR_RADIUS,
                                        egui::vec2(TOOLBAR_PAD, TOOLBAR_PAD),
                                    )
                                    .show(ui, |ui| {
                                        ui.spacing_mut().item_spacing = egui::vec2(1.5, 0.0);
                                        ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
                                        ui.horizontal(|ui| {
                                            let mut shape_popup = false;
                                            let mut blur_popup = false;
                                            let mut blur_resp = None;

                                            // 1. Select / Move / Edit drawn objects
                                            for tool in [Tool::Select, Tool::Arrow, Tool::Line] {
                                                let selected =
                                                    self.editing && self.editor.tool == tool;
                                                let tip = tool.label_i18n(&i18n);
                                                let resp = tool_icon(
                                                    ui, tool, selected, false, tip, palette,
                                                );
                                                if resp.clicked() {
                                                    self.editor.commit_text();
                                                    self.editor.select_tool(tool);
                                                    self.editing = true;
                                                }
                                            }

                                            // 2. Shapes submenu
                                            let shape_selected =
                                                self.editing && self.editor.tool.is_shape();
                                            let shape_tip =
                                                self.editor.shape_tool.label_i18n(&i18n);
                                            let shape_resp = tool_icon(
                                                ui,
                                                self.editor.shape_tool,
                                                shape_selected,
                                                true,
                                                shape_tip,
                                                palette,
                                            );
                                            if shape_resp.secondary_clicked()
                                                || (shape_selected && shape_resp.clicked())
                                            {
                                                shape_popup = true;
                                            } else if shape_resp.clicked() {
                                                self.editor.commit_text();
                                                self.editor.select_shape(self.editor.shape_tool);
                                                self.editing = true;
                                            }
                                            egui::Popup::from_response(&shape_resp)
                                                .gap(6.0)
                                                .width(156.0)
                                                .open_memory(
                                                    shape_popup
                                                        .then_some(egui::SetOpenCommand::Toggle),
                                                )
                                                .frame(floating_popup_frame(
                                                    palette,
                                                    egui::vec2(5.0, 5.0),
                                                ))
                                                .show(|ui| {
                                                    ui.set_min_width(144.0);
                                                    ui.spacing_mut().item_spacing =
                                                        egui::vec2(0.0, 2.0);
                                                    for kind in Tool::SHAPES {
                                                        if shape_option(
                                                            ui,
                                                            kind,
                                                            self.editor.shape_tool == kind,
                                                            &i18n,
                                                            palette,
                                                        ) {
                                                            self.editor.commit_text();
                                                            self.editor.select_shape(kind);
                                                            self.editing = true;
                                                            ui.close();
                                                        }
                                                    }
                                                });

                                            // 3. Pen, Marker, Text, Number, Blur
                                            for tool in [
                                                Tool::Pen,
                                                Tool::Marker,
                                                Tool::Text,
                                                Tool::Number,
                                                Tool::Blur,
                                            ] {
                                                let selected =
                                                    self.editing && self.editor.tool == tool;
                                                let tip = tool.label_i18n(&i18n);
                                                let resp = tool_icon(
                                                    ui,
                                                    tool,
                                                    selected,
                                                    tool == Tool::Blur,
                                                    tip,
                                                    palette,
                                                );
                                                if tool == Tool::Blur
                                                    && (resp.secondary_clicked()
                                                        || (selected && resp.clicked()))
                                                {
                                                    blur_popup = true;
                                                } else if resp.clicked() {
                                                    self.editor.commit_text();
                                                    self.editor.select_tool(tool);
                                                    self.editing = true;
                                                }
                                                if tool == Tool::Blur {
                                                    blur_resp = Some(resp);
                                                }
                                            }
                                            if let Some(resp) = &blur_resp {
                                                egui::Popup::from_response(resp)
                                                    .gap(6.0)
                                                    .width(176.0)
                                                    .open_memory(
                                                        blur_popup.then_some(
                                                            egui::SetOpenCommand::Toggle,
                                                        ),
                                                    )
                                                    .frame(floating_popup_frame(
                                                        palette,
                                                        egui::vec2(5.0, 5.0),
                                                    ))
                                                    .show(|ui| {
                                                        ui.set_min_width(164.0);
                                                        ui.spacing_mut().item_spacing =
                                                            egui::vec2(0.0, 2.0);
                                                        for kind in [
                                                            BlurKind::Soft,
                                                            BlurKind::Mosaic,
                                                            BlurKind::Heavy,
                                                        ] {
                                                            if blur_option(
                                                                ui,
                                                                kind,
                                                                self.editor.blur == kind,
                                                                &i18n,
                                                                palette,
                                                            ) {
                                                                self.editor.select_blur(kind);
                                                                self.editing = true;
                                                                ui.close();
                                                            }
                                                        }
                                                    });
                                            }

                                            // Color picker (updates selected shape too!)
                                            let tool = self.editing.then_some(self.editor.tool);
                                            if tool != Some(Tool::Blur) {
                                                bar_sep(ui, palette);
                                                let mut picked = self.editor.color;
                                                color_chip(
                                                    ui,
                                                    &mut picked,
                                                    &mut self.editor.custom,
                                                    &mut self.editor.custom_open,
                                                    i18n.color_tip(),
                                                    palette,
                                                );
                                                if picked != self.editor.color {
                                                    self.editor.apply_color(picked);
                                                }
                                            }

                                            bar_sep(ui, palette);
                                            if icon_btn(
                                                ui,
                                                Icon::Undo,
                                                self.editor.can_undo(),
                                                i18n.undo_tip(),
                                                CellStyle::Default,
                                                palette,
                                            ) {
                                                edit_action = Some(EditAction::Undo);
                                            }
                                            if icon_btn(
                                                ui,
                                                Icon::Redo,
                                                self.editor.can_redo(),
                                                i18n.redo_tip(),
                                                CellStyle::Default,
                                                palette,
                                            ) {
                                                edit_action = Some(EditAction::Redo);
                                            }
                                            if self.editor.selected.is_some()
                                                && icon_btn(
                                                    ui,
                                                    Icon::Delete,
                                                    true,
                                                    i18n.prop_delete_item(),
                                                    CellStyle::Danger,
                                                    palette,
                                                )
                                            {
                                                edit_action = Some(EditAction::DeleteSelected);
                                            }

                                            bar_sep(ui, palette);
                                            if icon_btn(
                                                ui,
                                                Icon::Studio,
                                                true,
                                                &format!("{} · Ctrl+E", i18n.tab_editor()),
                                                CellStyle::Default,
                                                palette,
                                            ) {
                                                open_studio = true;
                                            }
                                            if icon_btn(
                                                ui,
                                                Icon::Copy,
                                                true,
                                                &format!("{} · Enter", i18n.copy_image()),
                                                CellStyle::Primary,
                                                palette,
                                            ) {
                                                copy = true;
                                            }
                                            if icon_btn(
                                                ui,
                                                Icon::Save,
                                                true,
                                                i18n.save_tip(),
                                                CellStyle::Default,
                                                palette,
                                            ) {
                                                save = true;
                                            }
                                            bar_sep(ui, palette);
                                            if icon_btn(
                                                ui,
                                                Icon::Close,
                                                true,
                                                &format!("{} · Esc", i18n.cancel()),
                                                CellStyle::Danger,
                                                palette,
                                            ) {
                                                close = true;
                                            }
                                        });
                                    });
                                    ui.painter().set(
                                        shadow_idx,
                                        contact_shadow_shape(
                                            resp.response.rect,
                                            TOOLBAR_RADIUS,
                                            palette,
                                        ),
                                    );
                                });
                            self.toolbar_size =
                                (tools.response.rect.width(), tools.response.rect.height());

                            if self.editing
                                && let Some(at) = self.editor.text_at
                            {
                                let font = self.editor.text_size() / scale;
                                let ink = self.editor.color;
                                let ink =
                                    Color32::from_rgba_unmultiplied(ink.r, ink.g, ink.b, ink.a);
                                let pos = egui::pos2(
                                    (at.0 - bounds.x) as f32 / scale,
                                    (at.1 - bounds.y) as f32 / scale,
                                );
                                let resp = studio::live_text_input(
                                    viewport_ctx,
                                    egui::Id::new(("text", index)),
                                    pos,
                                    egui::Rect::from_min_size(
                                        egui::Pos2::ZERO,
                                        egui::vec2(screen.0, screen.1),
                                    ),
                                    &mut self.editor.text,
                                    font,
                                    ink,
                                    palette.accent,
                                    i18n.text_placeholder(),
                                );
                                if resp.lost_focus()
                                    && viewport_ctx.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    self.editor.commit_text();
                                } else if self.editor.text_at.is_some() {
                                    resp.request_focus();
                                }
                            }
                        }
                    }
                },
            );
            if let Err(error) =
                crate::native::fit_overlay(&title, *bounds, want_precision_cursor, scale)
            {
                tracing::error!(%error, "overlay placement failed");
                close = true;
                self.native.error();
            }
            if !*shown {
                *shown = true;
                ctx.send_viewport_cmd_to(overlay_id(index), ViewportCommand::Visible(true));
                if index == 0 {
                    ctx.send_viewport_cmd_to(overlay_id(index), ViewportCommand::Focus);
                }
                ctx.request_repaint();
            }
        }

        if open_studio {
            self.open_selection_in_editor(ctx);
        } else if copy || save {
            self.editor.commit_text();
            self.export_selection(save);
        } else if close {
            self.cancel();
        } else if let Some(action) = edit_action {
            match action {
                EditAction::Undo => self.editor.undo(),
                EditAction::Redo => self.editor.redo(),
                EditAction::DeleteSelected => {
                    self.editor.delete_selected();
                }
            }
            for (index, overlay) in self.overlay.iter_mut().enumerate() {
                if let Some(raw_frame) = self.raw_frames.get(index) {
                    sync_overlay_blurs(
                        ctx,
                        index,
                        overlay.bounds,
                        &raw_frame.bgra,
                        &mut overlay.committed_bgra,
                        &mut overlay.blur_patches,
                        &mut overlay.shapes_hash,
                        &mut overlay.draft_blur,
                        &self.editor.shapes,
                        self.editor.draft.as_ref(),
                    );
                }
            }
            ctx.request_repaint();
        }
        if self.focus_main {
            ctx.request_repaint();
        }
    }
}

enum EditAction {
    Undo,
    Redo,
    DeleteSelected,
}

#[derive(Clone, Copy)]
enum Icon {
    Undo,
    Redo,
    Delete,
    Studio,
    Copy,
    Save,
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CellStyle {
    Default,
    Primary,
    Danger,
}

fn floating_surface_frame(p: CapturePalette, radius: u8, margin: egui::Vec2) -> egui::Frame {
    egui::Frame::NONE
        .fill(p.surface)
        .stroke(egui::Stroke::new(1.0_f32, p.hairline))
        .corner_radius(radius)
        .shadow(egui::epaint::Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: p.shadow,
        })
        .inner_margin(margin)
}

fn floating_popup_frame(p: CapturePalette, margin: egui::Vec2) -> egui::Frame {
    egui::Frame::NONE
        .fill(p.popup)
        .stroke(egui::Stroke::new(1.0_f32, p.hairline))
        .corner_radius(6)
        .shadow(egui::epaint::Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: p.shadow,
        })
        .inner_margin(margin)
}

fn contact_shadow_shape(rect: egui::Rect, radius: u8, p: CapturePalette) -> egui::Shape {
    egui::Shape::Rect(
        egui::epaint::Shadow {
            offset: [0, 2],
            blur: 6,
            spread: 0,
            color: if p.light {
                Color32::from_black_alpha(22)
            } else {
                Color32::from_black_alpha(90)
            },
        }
        .as_shape(rect, egui::CornerRadius::same(radius)),
    )
}

fn paint_floating_shadow(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: u8,
    p: CapturePalette,
    compact: bool,
) {
    let ambient = egui::epaint::Shadow {
        offset: [0, if compact { 3 } else { 6 }],
        blur: if compact { 10 } else { 18 },
        spread: 0,
        color: if compact && !p.light {
            Color32::from_black_alpha(96)
        } else {
            p.shadow
        },
    };
    painter.add(egui::Shape::Rect(
        ambient.as_shape(rect, egui::CornerRadius::same(radius)),
    ));
    painter.add(contact_shadow_shape(rect, radius, p));
}

fn dimension_badge_pos(
    sel_local: egui::Rect,
    badge_size: egui::Vec2,
    screen: egui::Rect,
    toolbar_rect: Option<egui::Rect>,
) -> egui::Pos2 {
    let gap = 8.0;
    let margin = 6.0;
    let clamp_x = |x: f32| {
        x.clamp(
            screen.min.x + margin,
            (screen.max.x - badge_size.x - margin).max(screen.min.x + margin),
        )
    };
    let clamp_y = |y: f32| {
        y.clamp(
            screen.min.y + margin,
            (screen.max.y - badge_size.y - margin).max(screen.min.y + margin),
        )
    };
    let fits_screen = |pos: egui::Pos2| {
        let r = egui::Rect::from_min_size(pos, badge_size);
        r.min.x >= screen.min.x + margin - 0.5
            && r.max.x <= screen.max.x - margin + 0.5
            && r.min.y >= screen.min.y + margin - 0.5
            && r.max.y <= screen.max.y - margin + 0.5
    };
    let clear_of_toolbar = |pos: egui::Pos2| {
        toolbar_rect
            .is_none_or(|tb| !egui::Rect::from_min_size(pos, badge_size).intersects(tb.expand(4.0)))
    };

    let above_left = egui::pos2(
        clamp_x(sel_local.min.x),
        sel_local.min.y - badge_size.y - gap,
    );
    let below_left = egui::pos2(clamp_x(sel_local.min.x), sel_local.max.y + gap);
    let inside_top = egui::pos2(clamp_x(sel_local.min.x + 8.0), sel_local.min.y + 8.0);
    let mut candidates = vec![above_left, below_left, inside_top];
    if let Some(tb) = toolbar_rect {
        candidates.push(egui::pos2(
            clamp_x(tb.min.x - badge_size.x - gap),
            clamp_y(sel_local.min.y - badge_size.y - gap),
        ));
    }
    for pos in candidates {
        if fits_screen(pos) && clear_of_toolbar(pos) {
            return pos;
        }
    }
    egui::pos2(
        clamp_x(sel_local.min.x + 8.0),
        clamp_y(sel_local.min.y + 8.0),
    )
}

#[allow(clippy::too_many_arguments)]
fn paint_compact_magnifier(
    painter: &egui::Painter,
    texture_id: egui::TextureId,
    bounds: Region,
    scale: f32,
    cursor_phys: (i32, i32),
    rgb: (u8, u8, u8),
    screen_rect: egui::Rect,
    badge_rect: Option<egui::Rect>,
    palette: CapturePalette,
) {
    const COLS: i32 = 11;
    const ROWS: i32 = 9;
    const CELL: f32 = 7.0;
    const PAD: f32 = 3.0;
    const FOOTER_H: f32 = 19.0;

    let zoom_w = COLS as f32 * CELL; // 77.0
    let zoom_h = ROWS as f32 * CELL; // 63.0
    let card_w = zoom_w + PAD * 2.0; // 83.0
    let card_h = PAD + zoom_h + FOOTER_H; // 85.0
    let card_sz = egui::vec2(card_w, card_h);

    let local_cx = (cursor_phys.0 - bounds.x) as f32 / scale;
    let local_cy = (cursor_phys.1 - bounds.y) as f32 / scale;
    let candidates = [
        egui::pos2(local_cx + 16.0, local_cy + 16.0),
        egui::pos2(local_cx - card_w - 14.0, local_cy + 16.0),
        egui::pos2(local_cx + 16.0, local_cy - card_h - 14.0),
        egui::pos2(local_cx - card_w - 14.0, local_cy - card_h - 14.0),
    ];
    let safe_screen = screen_rect.shrink(6.0);
    let picked = candidates
        .into_iter()
        .find(|&pos| {
            let r = egui::Rect::from_min_size(pos, card_sz);
            safe_screen.contains_rect(r) && badge_rect.is_none_or(|b| !r.intersects(b.expand(4.0)))
        })
        .unwrap_or_else(|| {
            let mut p = candidates[0];
            if p.x + card_w > safe_screen.max.x {
                p.x = local_cx - card_w - 14.0;
            }
            if p.y + card_h > safe_screen.max.y {
                p.y = local_cy - card_h - 14.0;
            }
            egui::pos2(
                p.x.clamp(
                    safe_screen.min.x,
                    (safe_screen.max.x - card_w).max(safe_screen.min.x),
                ),
                p.y.clamp(
                    safe_screen.min.y,
                    (safe_screen.max.y - card_h).max(safe_screen.min.y),
                ),
            )
        });

    let card_rect = egui::Rect::from_min_size(picked, card_sz);
    paint_floating_shadow(painter, card_rect, LOUPE_RADIUS as u8, palette, true);
    painter.rect_filled(card_rect, LOUPE_RADIUS, palette.card);
    painter.rect_stroke(
        card_rect,
        LOUPE_RADIUS,
        egui::Stroke::new(1.0_f32, palette.hairline),
        egui::StrokeKind::Inside,
    );

    let zoom_rect = egui::Rect::from_min_size(
        card_rect.min + egui::vec2(PAD, PAD),
        egui::vec2(zoom_w, zoom_h),
    );
    painter.rect_filled(zoom_rect, 2.0, Color32::from_rgb(14, 15, 18));

    let px_x = cursor_phys.0 - bounds.x;
    let px_y = cursor_phys.1 - bounds.y;
    let half_x = COLS / 2; // 5
    let half_y = ROWS / 2; // 4
    let win_x0 = px_x - half_x;
    let win_y0 = px_y - half_y;
    let sx0 = win_x0.max(0);
    let sy0 = win_y0.max(0);
    let sx1 = (px_x + half_x + 1).min(bounds.width as i32);
    let sy1 = (px_y + half_y + 1).min(bounds.height as i32);

    if sx1 > sx0 && sy1 > sy0 && bounds.width > 0 && bounds.height > 0 {
        let dst_min =
            zoom_rect.min + egui::vec2((sx0 - win_x0) as f32 * CELL, (sy0 - win_y0) as f32 * CELL);
        let dst_size = egui::vec2((sx1 - sx0) as f32 * CELL, (sy1 - sy0) as f32 * CELL);
        let uv = egui::Rect::from_min_max(
            egui::pos2(
                sx0 as f32 / bounds.width as f32,
                sy0 as f32 / bounds.height as f32,
            ),
            egui::pos2(
                sx1 as f32 / bounds.width as f32,
                sy1 as f32 / bounds.height as f32,
            ),
        );
        painter.image(
            texture_id,
            egui::Rect::from_min_size(dst_min, dst_size),
            uv,
            Color32::WHITE,
        );
    }

    // Subtle 1px pixel grid so individual pixels stay readable even on flat dark/light surfaces
    let grid_col = Color32::from_rgba_unmultiplied(145, 155, 170, 22);
    for col in 1..COLS {
        let x = zoom_rect.left() + col as f32 * CELL;
        painter.line_segment(
            [
                egui::pos2(x, zoom_rect.top() + 1.0),
                egui::pos2(x, zoom_rect.bottom() - 1.0),
            ],
            egui::Stroke::new(1.0_f32, grid_col),
        );
    }
    for row in 1..ROWS {
        let y = zoom_rect.top() + row as f32 * CELL;
        painter.line_segment(
            [
                egui::pos2(zoom_rect.left() + 1.0, y),
                egui::pos2(zoom_rect.right() - 1.0, y),
            ],
            egui::Stroke::new(1.0_f32, grid_col),
        );
    }

    let c = zoom_rect.center();
    let half_cell = CELL * 0.5;
    let cross_line = with_alpha(palette.accent, 115);
    for (p0, p1) in [
        (
            egui::pos2(zoom_rect.left() + 1.0, c.y),
            egui::pos2(c.x - half_cell - 1.0, c.y),
        ),
        (
            egui::pos2(c.x + half_cell + 1.0, c.y),
            egui::pos2(zoom_rect.right() - 1.0, c.y),
        ),
        (
            egui::pos2(c.x, zoom_rect.top() + 1.0),
            egui::pos2(c.x, c.y - half_cell - 1.0),
        ),
        (
            egui::pos2(c.x, c.y + half_cell + 1.0),
            egui::pos2(c.x, zoom_rect.bottom() - 1.0),
        ),
    ] {
        painter.line_segment([p0, p1], egui::Stroke::new(1.0_f32, cross_line));
    }

    let reticle = egui::Rect::from_center_size(c, egui::vec2(CELL, CELL));
    painter.rect_stroke(
        reticle.expand(1.0),
        1.0,
        egui::Stroke::new(1.0_f32, Color32::from_black_alpha(165)),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        reticle,
        1.0,
        egui::Stroke::new(1.0_f32, palette.accent),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        zoom_rect,
        2.0_f32,
        egui::Stroke::new(1.0_f32, palette.edge),
        egui::StrokeKind::Inside,
    );

    let footer_cy = (zoom_rect.bottom() + card_rect.bottom()) * 0.5;
    let swatch = egui::Rect::from_center_size(
        egui::pos2(card_rect.left() + 9.5, footer_cy),
        egui::vec2(9.0, 9.0),
    );
    painter.rect_filled(swatch, 2.0, Color32::from_rgb(rgb.0, rgb.1, rgb.2));
    painter.rect_stroke(
        swatch,
        2.0,
        egui::Stroke::new(1.0_f32, palette.swatch_edge),
        egui::StrokeKind::Inside,
    );
    painter.text(
        egui::pos2(swatch.right() + 5.0, footer_cy),
        egui::Align2::LEFT_CENTER,
        format!("#{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2),
        egui::FontId::monospace(10.5),
        palette.text,
    );
}

fn parse_hex_rgb(input: &str) -> Option<(u8, u8, u8)> {
    let hex = input.trim().strip_prefix('#').unwrap_or(input.trim());
    match hex.len() {
        6 => Some((
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        )),
        3 => {
            let expand = |s: &str| u8::from_str_radix(s, 16).ok().map(|v| v * 17);
            Some((
                expand(&hex[0..1])?,
                expand(&hex[1..2])?,
                expand(&hex[2..3])?,
            ))
        }
        _ => None,
    }
}

fn tool_icon(
    ui: &mut egui::Ui,
    tool: Tool,
    on: bool,
    submenu: bool,
    tip: &str,
    p: CapturePalette,
) -> egui::Response {
    icon_cell(
        ui,
        on,
        true,
        tip,
        CellStyle::Default,
        p,
        |paint, r, ink, _| {
            draw_tool(paint, r, tool, ink);
            if submenu {
                let cue = if on {
                    Color32::WHITE
                } else {
                    with_alpha(p.muted, 190)
                };
                let tip_pt = egui::pos2(r.right() - 3.2, r.bottom() - 3.0);
                paint.add(egui::Shape::convex_polygon(
                    vec![
                        tip_pt + egui::vec2(-2.5, 0.4),
                        tip_pt + egui::vec2(0.4, -2.5),
                        tip_pt + egui::vec2(0.4, 0.4),
                    ],
                    cue,
                    egui::Stroke::NONE,
                ));
            }
        },
    )
}

fn icon_cell(
    ui: &mut egui::Ui,
    on: bool,
    enabled: bool,
    tip: &str,
    style: CellStyle,
    p: CapturePalette,
    draw: impl FnOnce(&egui::Painter, egui::Rect, Color32, Color32),
) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(TOOL_SIZE, TOOL_SIZE), egui::Sense::click());
    let hovered = enabled && resp.hovered();
    let hover_t = ui.ctx().animate_bool_with_time(resp.id, hovered, 0.08);
    let backdrop = match style {
        CellStyle::Primary => p.accent_solid.lerp_to_gamma(Color32::WHITE, hover_t * 0.12),
        CellStyle::Danger => {
            if hover_t > 0.0 {
                p.surface.lerp_to_gamma(with_alpha(p.err, 45), hover_t)
            } else {
                p.surface
            }
        }
        CellStyle::Default => {
            if on {
                p.accent_solid
            } else if hover_t > 0.0 {
                p.surface.lerp_to_gamma(p.hover, hover_t)
            } else {
                p.surface
            }
        }
    };
    if style == CellStyle::Primary || on {
        ui.painter().rect_filled(rect, TOOL_RADIUS, backdrop);
        ui.painter().rect_stroke(
            rect,
            TOOL_RADIUS,
            egui::Stroke::new(1.0_f32, p.accent),
            egui::StrokeKind::Inside,
        );
    } else if hover_t > 0.01 {
        ui.painter().rect_filled(rect, TOOL_RADIUS, backdrop);
    }
    let ink = if !enabled {
        with_alpha(p.muted, 130)
    } else if style == CellStyle::Primary || on {
        Color32::WHITE
    } else if style == CellStyle::Danger {
        p.text.lerp_to_gamma(p.err, hover_t)
    } else if resp.has_focus() {
        p.primary
    } else {
        p.text.lerp_to_gamma(p.primary, hover_t)
    };
    if enabled && resp.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            TOOL_RADIUS - 0.5,
            egui::Stroke::new(1.5_f32, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    draw(&ui.painter().with_clip_rect(rect), rect, ink, backdrop);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), on, tip)
    });
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(tip)
    } else {
        resp
    }
}

fn icon_btn(
    ui: &mut egui::Ui,
    icon: Icon,
    enabled: bool,
    tip: &str,
    style: CellStyle,
    p: CapturePalette,
) -> bool {
    icon_cell(
        ui,
        false,
        enabled,
        tip,
        style,
        p,
        |painter, r, ink, backdrop| draw_icon(painter, r, icon, ink, backdrop),
    )
    .clicked()
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn icon_pt(c: egui::Pos2, x: f32, y: f32) -> egui::Pos2 {
    c + egui::vec2(x, y)
}

fn icon_line(painter: &egui::Painter, points: &[egui::Pos2], stroke: egui::Stroke) {
    if points.len() < 2 {
        return;
    }
    painter.add(egui::Shape::line(points.to_vec(), stroke));
    let cap = stroke.width * 0.5;
    painter.circle_filled(points[0], cap, stroke.color);
    painter.circle_filled(*points.last().unwrap(), cap, stroke.color);
}

pub(super) fn draw_tool(painter: &egui::Painter, rect: egui::Rect, tool: Tool, ink: Color32) {
    let c = rect.center();
    let stroke = egui::Stroke::new(1.5_f32, ink);
    match tool {
        Tool::Select => {
            // Crisp ShareX-style pointer cursor glyph
            let tip = icon_pt(c, -4.2, -5.6);
            let pts = vec![
                tip,
                icon_pt(c, -4.2, 4.4),
                icon_pt(c, -1.3, 1.8),
                icon_pt(c, 1.0, 5.6),
                icon_pt(c, 2.5, 4.8),
                icon_pt(c, 0.2, 1.0),
                icon_pt(c, 3.8, 1.0),
            ];
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
        Tool::Arrow => {
            icon_line(
                painter,
                &[icon_pt(c, -4.8, 4.8), icon_pt(c, 4.8, -4.8)],
                stroke,
            );
            icon_line(
                painter,
                &[
                    icon_pt(c, -0.2, -4.8),
                    icon_pt(c, 4.8, -4.8),
                    icon_pt(c, 4.8, 0.2),
                ],
                stroke,
            );
        }
        Tool::Line => {
            let a = icon_pt(c, -4.6, 3.8);
            let b = icon_pt(c, 4.6, -3.8);
            icon_line(painter, &[a, b], stroke);
            painter.circle_filled(a, 1.65, ink);
            painter.circle_filled(b, 1.65, ink);
        }
        Tool::Rect => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(11.6, 9.4)),
                1.4,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Tool::Ellipse => {
            painter.circle_stroke(c, 5.2, stroke);
        }
        Tool::Triangle => {
            let top = icon_pt(c, 0.0, -4.8);
            icon_line(
                painter,
                &[top, icon_pt(c, 5.2, 4.4), icon_pt(c, -5.2, 4.4), top],
                stroke,
            );
        }
        Tool::Pen => {
            let along = egui::vec2(
                std::f32::consts::FRAC_1_SQRT_2,
                -std::f32::consts::FRAC_1_SQRT_2,
            );
            let across = egui::vec2(
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
            );
            let at = |t: f32, w: f32| c + along * t + across * w;
            let half = 1.85;
            let shoulder = -3.8;
            let butt = 5.4;
            painter.add(egui::Shape::convex_polygon(
                vec![at(-6.2, 0.0), at(shoulder, half), at(shoulder, -half)],
                ink,
                egui::Stroke::NONE,
            ));
            painter.add(egui::Shape::closed_line(
                vec![
                    at(shoulder, -half),
                    at(butt, -half),
                    at(butt, half),
                    at(shoulder, half),
                ],
                stroke,
            ));
            icon_line(
                painter,
                &[at(3.6, -half), at(3.6, half)],
                egui::Stroke::new(1.2_f32, ink),
            );
        }
        Tool::Marker => {
            let along = egui::vec2(
                std::f32::consts::FRAC_1_SQRT_2,
                -std::f32::consts::FRAC_1_SQRT_2,
            );
            let across = egui::vec2(
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
            );
            let at = |t: f32, w: f32| c + along * t + across * w;
            icon_line(
                painter,
                &[at(-7.2, 0.0), at(-4.8, 0.0)],
                egui::Stroke::new(3.8_f32, with_alpha(ink, 74)),
            );
            painter.add(egui::Shape::closed_line(
                vec![
                    at(-5.0, -1.8),
                    at(-3.2, -2.3),
                    at(5.0, -2.3),
                    at(5.0, 2.3),
                    at(-3.2, 2.3),
                    at(-5.0, 1.8),
                ],
                stroke,
            ));
            icon_line(
                painter,
                &[at(3.5, -2.3), at(3.5, 2.3)],
                egui::Stroke::new(1.2_f32, ink),
            );
        }
        Tool::Text => {
            icon_line(
                painter,
                &[
                    icon_pt(c, -4.6, -3.2),
                    icon_pt(c, -4.6, -4.6),
                    icon_pt(c, 4.6, -4.6),
                    icon_pt(c, 4.6, -3.2),
                ],
                stroke,
            );
            icon_line(
                painter,
                &[icon_pt(c, 0.0, -4.6), icon_pt(c, 0.0, 4.8)],
                stroke,
            );
            icon_line(
                painter,
                &[icon_pt(c, -2.0, 4.8), icon_pt(c, 2.0, 4.8)],
                egui::Stroke::new(1.3_f32, ink),
            );
        }
        Tool::Number => {
            painter.circle_stroke(c, 5.8, stroke);
            let numeral = egui::Stroke::new(1.35_f32, ink);
            icon_line(
                painter,
                &[
                    icon_pt(c, -1.4, -1.4),
                    icon_pt(c, 0.3, -2.9),
                    icon_pt(c, 0.3, 2.9),
                ],
                numeral,
            );
            icon_line(
                painter,
                &[icon_pt(c, -1.6, 2.9), icon_pt(c, 2.0, 2.9)],
                numeral,
            );
        }
        Tool::Blur => {
            for gy in -1i32..=1 {
                for gx in -1i32..=1 {
                    let falloff = match (gx.abs(), gy.abs()) {
                        (0, 0) => 1.0,
                        (0, 1) | (1, 0) => 0.56,
                        _ => 0.28,
                    };
                    let center = c + egui::vec2(gx as f32 * 3.5, gy as f32 * 3.5);
                    painter.rect_filled(
                        egui::Rect::from_center_size(center, egui::vec2(2.6, 2.6)),
                        0.6,
                        with_alpha(ink, (falloff * 255.0) as u8),
                    );
                }
            }
        }
    }
}

fn popup_option_row(
    ui: &mut egui::Ui,
    width: f32,
    label: &str,
    selected: bool,
    p: CapturePalette,
    draw_icon: impl FnOnce(&egui::Painter, egui::Pos2, Color32),
) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(width, CAPTURE_ROW_H), egui::Sense::click());
    let hovered = resp.hovered();
    let hover_t = ui.ctx().animate_bool_with_time(resp.id, hovered, 0.08);
    if selected {
        ui.painter().rect_filled(rect, 4.0, p.select);
    } else if hover_t > 0.01 {
        ui.painter()
            .rect_filled(rect, 4.0, p.popup.lerp_to_gamma(p.hover, hover_t));
    }
    let ink = if hovered || selected {
        p.primary
    } else {
        p.text
    };
    let icon = egui::pos2(rect.left() + 14.0, rect.center().y);
    draw_icon(&ui.painter().with_clip_rect(rect), icon, ink);
    ui.painter().text(
        egui::pos2(rect.left() + 28.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        ink,
    );
    if selected {
        check_mark(
            &ui.painter().with_clip_rect(rect),
            egui::pos2(rect.right() - 12.0, rect.center().y),
            p.accent,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

fn shape_option(
    ui: &mut egui::Ui,
    kind: Tool,
    selected: bool,
    i18n: &Strings,
    p: CapturePalette,
) -> bool {
    popup_option_row(
        ui,
        144.0,
        kind.label_i18n(i18n),
        selected,
        p,
        |painter, pos, ink| {
            let icon_rect = egui::Rect::from_center_size(pos, egui::vec2(18.0, 18.0));
            draw_tool(painter, icon_rect, kind, ink);
        },
    )
}

fn blur_option(
    ui: &mut egui::Ui,
    kind: BlurKind,
    selected: bool,
    i18n: &Strings,
    p: CapturePalette,
) -> bool {
    popup_option_row(
        ui,
        164.0,
        kind.label_i18n(i18n),
        selected,
        p,
        |painter, pos, ink| {
            blur_kind_icon(painter, pos, kind, ink);
        },
    )
}

fn blur_kind_icon(painter: &egui::Painter, c: egui::Pos2, kind: BlurKind, ink: Color32) {
    match kind {
        BlurKind::Soft => {
            painter.circle_filled(c, 6.0, with_alpha(ink, 42));
            painter.circle_filled(c, 4.0, with_alpha(ink, 96));
            painter.circle_filled(c, 2.0, ink);
        }
        BlurKind::Mosaic => {
            for gy in -1..=1 {
                for gx in -1..=1 {
                    let tone = match (gx, gy) {
                        (0, 0) => 235,
                        (0, _) | (_, 0) => 148,
                        _ => 76,
                    };
                    painter.rect_filled(
                        egui::Rect::from_center_size(
                            c + egui::vec2(gx as f32 * 3.8, gy as f32 * 3.8),
                            egui::vec2(3.0, 3.0),
                        ),
                        0.6,
                        with_alpha(ink, tone),
                    );
                }
            }
        }
        BlurKind::Heavy => {
            for gy in -1..=1 {
                for gx in -1..=1 {
                    painter.rect_filled(
                        egui::Rect::from_center_size(
                            c + egui::vec2(gx as f32 * 3.6, gy as f32 * 3.6),
                            egui::vec2(3.2, 3.2),
                        ),
                        0.6,
                        with_alpha(ink, 210),
                    );
                }
            }
        }
    }
}

fn check_mark(painter: &egui::Painter, c: egui::Pos2, ink: Color32) {
    let stroke = egui::Stroke::new(1.5_f32, ink);
    icon_line(
        painter,
        &[
            c + egui::vec2(-3.8, 0.0),
            c + egui::vec2(-1.0, 3.0),
            c + egui::vec2(4.2, -3.2),
        ],
        stroke,
    );
}

fn draw_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    icon: Icon,
    ink: Color32,
    _backdrop: Color32,
) {
    let c = rect.center();
    let stroke = egui::Stroke::new(1.5_f32, ink);
    match icon {
        Icon::Undo => return_arrow(painter, c, true, ink),
        Icon::Redo => return_arrow(painter, c, false, ink),
        Icon::Delete => ui::draw_ui_icon(painter, rect.shrink(4.0), ui::UiIcon::Delete, ink),
        Icon::Studio => ui::draw_ui_icon(painter, rect.shrink(4.0), ui::UiIcon::Editor, ink),
        Icon::Copy => {
            let front = egui::Rect::from_min_max(icon_pt(c, -4.8, -1.6), icon_pt(c, 2.2, 5.4));
            painter.rect_stroke(front, 1.4, stroke, egui::StrokeKind::Inside);
            icon_line(
                painter,
                &[
                    icon_pt(c, -1.8, -1.6),
                    icon_pt(c, -1.8, -4.2),
                    icon_pt(c, -1.0, -5.0),
                    icon_pt(c, 4.0, -5.0),
                    icon_pt(c, 4.8, -4.2),
                    icon_pt(c, 4.8, 1.0),
                    icon_pt(c, 4.0, 1.8),
                    icon_pt(c, 2.2, 1.8),
                ],
                stroke,
            );
        }
        Icon::Save => {
            icon_line(
                painter,
                &[
                    icon_pt(c, -5.0, 1.6),
                    icon_pt(c, -5.0, 4.0),
                    icon_pt(c, -4.2, 4.8),
                    icon_pt(c, 4.2, 4.8),
                    icon_pt(c, 5.0, 4.0),
                    icon_pt(c, 5.0, 1.6),
                ],
                stroke,
            );
            icon_line(
                painter,
                &[icon_pt(c, 0.0, -5.0), icon_pt(c, 0.0, 1.8)],
                stroke,
            );
            icon_line(
                painter,
                &[
                    icon_pt(c, -3.2, -1.0),
                    icon_pt(c, 0.0, 2.2),
                    icon_pt(c, 3.2, -1.0),
                ],
                stroke,
            );
        }
        Icon::Close => {
            icon_line(
                painter,
                &[icon_pt(c, -4.0, -4.0), icon_pt(c, 4.0, 4.0)],
                stroke,
            );
            icon_line(
                painter,
                &[icon_pt(c, 4.0, -4.0), icon_pt(c, -4.0, 4.0)],
                stroke,
            );
        }
    }
}

pub(super) fn return_arrow(painter: &egui::Painter, c: egui::Pos2, undo: bool, ink: Color32) {
    let stroke = egui::Stroke::new(1.5_f32, ink);
    let dir = if undo { 1.0 } else { -1.0 };
    let x = |v: f32| c.x + v * dir;
    let y = |v: f32| c.y + v;
    let tip = egui::pos2(x(-4.8), y(-1.6));
    icon_line(
        painter,
        &[
            egui::pos2(x(-1.8), y(-4.4)),
            tip,
            egui::pos2(x(-1.8), y(1.2)),
        ],
        stroke,
    );
    let mut points = vec![tip, egui::pos2(x(1.6), y(-1.6))];
    for step in 1..=16 {
        let angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * step as f32 / 16.0;
        points.push(egui::pos2(
            x(1.6) + dir * angle.cos() * 3.3,
            y(1.7) + angle.sin() * 3.3,
        ));
    }
    points.push(egui::pos2(x(-0.6), y(5.0)));
    icon_line(painter, &points, stroke);
}

fn bar_sep(ui: &mut egui::Ui, p: CapturePalette) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(SEP_W, TOOL_SIZE), egui::Sense::hover());
    let x = rect.center().x;
    ui.painter().line_segment(
        [
            egui::pos2(x, rect.top() + 6.0),
            egui::pos2(x, rect.bottom() - 6.0),
        ],
        egui::Stroke::new(1.0_f32, p.edge),
    );
}

#[derive(Clone, Default)]
struct HexDraft {
    text: String,
    last_color: Option<edit::Rgba>,
}

fn push_custom_color(custom: &mut [Option<edit::Rgba>; 8], picked: edit::Rgba) {
    if !custom.iter().flatten().any(|c| *c == picked) {
        custom.rotate_right(1);
        custom[0] = Some(picked);
    }
}

pub(super) fn color_chip(
    ui: &mut egui::Ui,
    color: &mut edit::Rgba,
    custom: &mut [Option<edit::Rgba>; 8],
    custom_open: &mut bool,
    tip: &str,
    p: CapturePalette,
) {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(TOOL_SIZE, TOOL_SIZE), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tip));
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let hover_t = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.08);
    if open {
        ui.painter().rect_filled(rect, TOOL_RADIUS, p.select);
        ui.painter().rect_stroke(
            rect,
            TOOL_RADIUS,
            egui::Stroke::new(1.0_f32, p.accent),
            egui::StrokeKind::Inside,
        );
    } else {
        let fill = p.raised.lerp_to_gamma(p.hover, hover_t);
        ui.painter().rect_filled(rect, TOOL_RADIUS, fill);
        ui.painter().rect_stroke(
            rect,
            TOOL_RADIUS,
            egui::Stroke::new(1.0_f32, p.edge),
            egui::StrokeKind::Inside,
        );
    }

    let swatch_rect = egui::Rect::from_center_size(
        egui::pos2(rect.center().x - 1.0, rect.center().y),
        egui::vec2(14.0, 14.0),
    );
    ui.painter().rect_filled(
        swatch_rect,
        3.0,
        Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a),
    );
    ui.painter().rect_stroke(
        swatch_rect,
        3.0,
        egui::Stroke::new(1.0_f32, p.swatch_edge),
        egui::StrokeKind::Inside,
    );
    // Subtle corner dropdown caret cue
    let corner = egui::pos2(rect.right() - 2.5, rect.bottom() - 2.5);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            corner,
            egui::pos2(corner.x - 4.0, corner.y),
            egui::pos2(corner.x, corner.y - 4.0),
        ],
        p.muted,
        egui::Stroke::NONE,
    ));
    let resp = resp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tip);
    if resp.clicked() && !open {
        *custom_open = false;
    }

    egui::Popup::menu(&resp)
        .gap(6.0)
        .width(240.0)
        .frame(floating_popup_frame(p, egui::vec2(10.0, 10.0)))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(216.0);
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            egui::Grid::new("palette")
                .spacing(egui::vec2(4.0, 4.0))
                .min_col_width(23.0)
                .max_col_width(23.0)
                .min_row_height(23.0)
                .show(ui, |ui| {
                    for (index, swatch) in edit::PALETTE.iter().enumerate() {
                        if palette_cell(ui, *color, Some(*swatch), p, false) {
                            *color = *swatch;
                            ui.close();
                        }
                        if (index + 1) % 8 == 0 {
                            ui.end_row();
                        }
                    }
                });

            ui.add_space(4.0);
            egui::Grid::new("custom-colors")
                .spacing(egui::vec2(4.0, 4.0))
                .min_col_width(23.0)
                .max_col_width(23.0)
                .min_row_height(23.0)
                .show(ui, |ui| {
                    for slot in custom.iter() {
                        if palette_cell(ui, *color, *slot, p, true) {
                            if let Some(swatch) = slot {
                                *color = *swatch;
                                ui.close();
                            } else {
                                *custom_open = true;
                            }
                        }
                    }
                });

            ui.add_space(4.0);
            if custom_color_row(ui, *color, *custom_open, p) {
                *custom_open = !*custom_open;
            }
            if *custom_open {
                ui.add_space(4.0);
                let hex_id = egui::Id::new("capture_hex_draft");
                let mut hex_draft = ui
                    .ctx()
                    .data_mut(|d| d.get_temp::<HexDraft>(hex_id).unwrap_or_default());
                if hex_draft.last_color != Some(*color) {
                    hex_draft.text = format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b);
                    hex_draft.last_color = Some(*color);
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Hex").size(11.0).color(p.muted));
                    ui.add_space(4.0);
                    let hex_resp = ui.add_sized(
                        [82.0, 22.0],
                        egui::TextEdit::singleline(&mut hex_draft.text)
                            .font(egui::FontId::monospace(11.5))
                            .margin(egui::Margin::symmetric(6, 3)),
                    );
                    if hex_resp.changed()
                        && let Some((r, g, b)) = parse_hex_rgb(&hex_draft.text)
                    {
                        *color = edit::Rgba::rgb(r, g, b);
                        hex_draft.last_color = Some(*color);
                    }
                    let enter =
                        hex_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if hex_resp.lost_focus() || enter {
                        if let Some((r, g, b)) = parse_hex_rgb(&hex_draft.text) {
                            *color = edit::Rgba::rgb(r, g, b);
                            push_custom_color(custom, *color);
                        }
                        hex_draft.text = format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b);
                        hex_draft.last_color = Some(*color);
                    }
                });
                ui.add_space(2.0);
                ui.spacing_mut().slider_width = 144.0;
                ui.spacing_mut().interact_size.y = 18.0;
                let mut changed = false;
                let mut settled = false;
                for (label, value) in [
                    ("R", &mut color.r),
                    ("G", &mut color.g),
                    ("B", &mut color.b),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(label).size(11.0).color(p.muted));
                        let resp = ui.add(
                            egui::Slider::new(value, 0..=255)
                                .show_value(true)
                                .trailing_fill(false),
                        );
                        changed |= resp.changed();
                        settled |= resp.drag_stopped() || resp.lost_focus();
                    });
                }
                if changed {
                    color.a = 255;
                    hex_draft.text = format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b);
                    hex_draft.last_color = Some(*color);
                }
                if settled {
                    push_custom_color(custom, *color);
                }
                ui.ctx().data_mut(|d| d.insert_temp(hex_id, hex_draft));
            }
        });
}

fn custom_color_row(ui: &mut egui::Ui, color: edit::Rgba, open: bool, p: CapturePalette) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(216.0, CAPTURE_ROW_H), egui::Sense::click());
    let hover_t = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.08);
    if hover_t > 0.01 {
        ui.painter()
            .rect_filled(rect, 4.0, p.popup.lerp_to_gamma(p.hover, hover_t));
    }
    let swatch = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::vec2(13.0, 13.0),
    );
    ui.painter().rect_filled(
        swatch,
        3.0,
        Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a),
    );
    ui.painter().rect_stroke(
        swatch,
        3.0,
        egui::Stroke::new(1.0_f32, p.swatch_edge),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 24.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "RGB / Hex",
        egui::FontId::proportional(12.0),
        p.text,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 24.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b),
        egui::FontId::monospace(10.5),
        p.muted,
    );
    let sign_center = egui::pos2(rect.right() - 10.0, rect.center().y);
    let stroke = egui::Stroke::new(1.25_f32, p.muted);
    ui.painter().line_segment(
        [
            sign_center + egui::vec2(-3.0, 0.0),
            sign_center + egui::vec2(3.0, 0.0),
        ],
        stroke,
    );
    if !open {
        ui.painter().line_segment(
            [
                sign_center + egui::vec2(0.0, -3.0),
                sign_center + egui::vec2(0.0, 3.0),
            ],
            stroke,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

fn palette_cell(
    ui: &mut egui::Ui,
    current: edit::Rgba,
    swatch: Option<edit::Rgba>,
    p: CapturePalette,
    empty_opens: bool,
) -> bool {
    let (cell, resp) = ui.allocate_exact_size(egui::vec2(23.0, 23.0), egui::Sense::click());
    let fill = egui::Rect::from_center_size(cell.center(), egui::vec2(19.0, 19.0));
    match swatch {
        Some(swatch) => {
            ui.painter().rect_filled(
                fill,
                3.5,
                Color32::from_rgba_unmultiplied(swatch.r, swatch.g, swatch.b, swatch.a),
            );
            ui.painter().rect_stroke(
                fill,
                3.5,
                egui::Stroke::new(1.0_f32, with_alpha(p.swatch_edge, 48)),
                egui::StrokeKind::Inside,
            );
            if current.r == swatch.r && current.g == swatch.g && current.b == swatch.b {
                ui.painter().rect_stroke(
                    fill.expand(2.0),
                    4.5,
                    egui::Stroke::new(1.8_f32, p.accent),
                    egui::StrokeKind::Inside,
                );
            } else if resp.hovered() {
                ui.painter().rect_stroke(
                    fill.expand(1.5),
                    4.0,
                    egui::Stroke::new(1.0_f32, p.muted),
                    egui::StrokeKind::Inside,
                );
            }
        }
        None => {
            ui.painter().rect_stroke(
                fill,
                3.5,
                egui::Stroke::new(1.0_f32, if resp.hovered() { p.muted } else { p.edge }),
                egui::StrokeKind::Inside,
            );
            let c = fill.center();
            let plus = egui::Stroke::new(1.2_f32, if resp.hovered() { p.primary } else { p.muted });
            ui.painter()
                .line_segment([c + egui::vec2(-3.0, 0.0), c + egui::vec2(3.0, 0.0)], plus);
            ui.painter()
                .line_segment([c + egui::vec2(0.0, -3.0), c + egui::vec2(0.0, 3.0)], plus);
        }
    }
    if resp.hovered() {
        resp.clone().on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    resp.clicked() && (swatch.is_some() || empty_opens)
}

fn capture_ui_hit(ctx: &egui::Context, popup_was_open: bool) -> bool {
    popup_was_open
        || egui::Popup::is_any_open(ctx)
        || ctx.input(|i| i.pointer.interact_pos()).is_some_and(|pos| {
            ctx.layer_id_at(pos)
                .is_some_and(|layer| layer.order != egui::Order::Background)
        })
}

fn intersect(frame: Region, selection: Region) -> Option<Region> {
    let left = frame.x.max(selection.x);
    let top = frame.y.max(selection.y);
    let right = (i64::from(frame.x) + i64::from(frame.width))
        .min(i64::from(selection.x) + i64::from(selection.width));
    let bottom = (i64::from(frame.y) + i64::from(frame.height))
        .min(i64::from(selection.y) + i64::from(selection.height));
    if right <= i64::from(left) || bottom <= i64::from(top) {
        return None;
    }
    Some(Region {
        x: left,
        y: top,
        width: (right - i64::from(left)) as u32,
        height: (bottom - i64::from(top)) as u32,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_shapes_view(
    painter: &egui::Painter,
    bounds: &Region,
    origin_ui: egui::Pos2,
    px_to_ui: f32,
    shapes: &[Shape],
    skip_text_index: Option<usize>,
    draft: Option<&Shape>,
    blur_patches: &[egui::TextureHandle],
    draft_blur: Option<&egui::TextureHandle>,
) {
    let mut blur_i = 0;
    for (idx, shape) in shapes.iter().enumerate() {
        let patch = if matches!(shape, Shape::Blur { .. }) {
            let tex = blur_patches.get(blur_i);
            blur_i += 1;
            tex
        } else {
            None
        };
        if skip_text_index == Some(idx) {
            continue;
        }
        paint_shape_mapped(painter, bounds, origin_ui, px_to_ui, shape, patch);
    }
    if let Some(shape) = draft {
        let patch = matches!(shape, Shape::Blur { .. })
            .then_some(draft_blur)
            .flatten();
        paint_shape_mapped(painter, bounds, origin_ui, px_to_ui, shape, patch);
    }
}

fn to_local(bounds: &Region, scale: f32, p: (i32, i32)) -> egui::Pos2 {
    egui::pos2(
        (p.0 - bounds.x) as f32 / scale,
        (p.1 - bounds.y) as f32 / scale,
    )
}

fn map_view(bounds: &Region, origin_ui: egui::Pos2, px_to_ui: f32, p: (i32, i32)) -> egui::Pos2 {
    egui::pos2(
        origin_ui.x + (p.0 - bounds.x) as f32 * px_to_ui,
        origin_ui.y + (p.1 - bounds.y) as f32 * px_to_ui,
    )
}

fn stroke_open_polyline(painter: &egui::Painter, pts: &[egui::Pos2], stroke: egui::Stroke) {
    if pts.is_empty() {
        return;
    }
    let cap = stroke.width * 0.5;
    for pair in pts.windows(2) {
        painter.line_segment([pair[0], pair[1]], stroke);
    }
    for &p in pts {
        painter.circle_filled(p, cap, stroke.color);
    }
}

fn stroke_closed_polygon(painter: &egui::Painter, pts: &[egui::Pos2], stroke: egui::Stroke) {
    if pts.len() < 2 {
        return;
    }
    let cap = stroke.width * 0.5;
    for i in 0..pts.len() {
        painter.line_segment([pts[i], pts[(i + 1) % pts.len()]], stroke);
    }
    for &p in pts {
        painter.circle_filled(p, cap, stroke.color);
    }
}

pub(super) fn paint_shape_mapped(
    painter: &egui::Painter,
    bounds: &Region,
    origin_ui: egui::Pos2,
    px_to_ui: f32,
    shape: &Shape,
    blur_texture: Option<&egui::TextureHandle>,
) {
    let color_of = |c: edit::Rgba| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
    let map = |p: (i32, i32)| map_view(bounds, origin_ui, px_to_ui, p);
    match shape {
        Shape::Line { a, b, color, width } => {
            painter.line_segment(
                [map(*a), map(*b)],
                egui::Stroke::new((*width * px_to_ui).max(1.0), color_of(*color)),
            );
        }
        Shape::Arrow { a, b, color, width } => {
            let stroke_w = (*width * px_to_ui).max(1.0);
            let ink = color_of(*color);
            let stroke = egui::Stroke::new(stroke_w, ink);
            painter.line_segment([map(*a), map(*b)], stroke);
            if let Some(head) = edit::arrow_head(*a, *b, (*width * 4.0).max(10.0)) {
                painter.add(egui::Shape::convex_polygon(
                    vec![map(*b), map(head[0]), map(head[1])],
                    ink,
                    egui::Stroke::NONE,
                ));
            }
        }
        Shape::Rect {
            a,
            b,
            color,
            width,
            filled,
        } => {
            if a == b {
                return;
            }
            let r = edit::norm_rect(*a, *b);
            let rect = egui::Rect::from_min_max(
                map((r.x, r.y)),
                map((r.x + r.width as i32, r.y + r.height as i32)),
            );
            let stroke_w = (*width * px_to_ui).max(1.0);
            let ink = color_of(*color);
            if *filled || rect.width() <= stroke_w * 2.0 || rect.height() <= stroke_w * 2.0 {
                let filled_r = egui::Rect::from_min_max(
                    rect.min,
                    egui::pos2(
                        rect.max.x.max(rect.min.x + 1.0),
                        rect.max.y.max(rect.min.y + 1.0),
                    ),
                );
                painter.rect_filled(filled_r, 0.0, ink);
            } else {
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(stroke_w, ink),
                    egui::StrokeKind::Inside,
                );
            }
        }
        Shape::Ellipse {
            a,
            b,
            color,
            width,
            filled,
        } => {
            if a == b {
                return;
            }
            let r = edit::norm_rect(*a, *b);
            let rect = egui::Rect::from_min_max(
                map((r.x, r.y)),
                map((r.x + r.width as i32, r.y + r.height as i32)),
            );
            let ink = color_of(*color);
            if *filled {
                painter.add(egui::Shape::ellipse_filled(
                    rect.center(),
                    rect.size() * 0.5,
                    ink,
                ));
                return;
            }
            let stroke = egui::Stroke::new((*width * px_to_ui).max(1.0), ink);
            if rect.width() < 2.0 || rect.height() < 2.0 {
                stroke_open_polyline(painter, &[rect.min, rect.max], stroke);
            } else {
                let min_dim = rect.width().min(rect.height());
                let max_dim = rect.width().max(rect.height());
                if min_dim < (stroke.width * 2.5).max(6.0) || max_dim > min_dim * 2.5 {
                    const SEGMENTS: usize = 48;
                    let c = rect.center();
                    let rx = rect.width() * 0.5;
                    let ry = rect.height() * 0.5;
                    let pts: Vec<egui::Pos2> = (0..SEGMENTS)
                        .map(|i| {
                            let angle = std::f32::consts::TAU * i as f32 / SEGMENTS as f32;
                            egui::pos2(c.x + rx * angle.cos(), c.y + ry * angle.sin())
                        })
                        .collect();
                    stroke_closed_polygon(painter, &pts, stroke);
                } else {
                    painter.add(egui::Shape::ellipse_stroke(
                        rect.center(),
                        rect.size() * 0.5,
                        stroke,
                    ));
                }
            }
        }
        Shape::Triangle {
            a,
            b,
            color,
            width,
            filled,
        } => {
            if a == b {
                return;
            }
            let ink = color_of(*color);
            let pts: [egui::Pos2; 3] = edit::triangle_points(*a, *b).map(map);
            if *filled {
                painter.add(egui::Shape::convex_polygon(
                    pts.to_vec(),
                    ink,
                    egui::Stroke::NONE,
                ));
            } else {
                let stroke = egui::Stroke::new((*width * px_to_ui).max(1.0), ink);
                stroke_closed_polygon(painter, &pts, stroke);
            }
        }
        Shape::Pen {
            points,
            color,
            width,
        } => {
            if points.len() >= 2 {
                let pts: Vec<_> = points.iter().copied().map(map).collect();
                stroke_open_polyline(
                    painter,
                    &pts,
                    egui::Stroke::new((*width * px_to_ui).max(1.0), color_of(*color)),
                );
            }
        }
        Shape::Marker {
            points,
            color,
            width,
        } => {
            let pts: Vec<_> = points.iter().copied().map(map).collect();
            if pts.len() >= 2 {
                painter.add(egui::Shape::line(
                    pts,
                    egui::Stroke::new((*width * px_to_ui).max(1.0), color_of(*color)),
                ));
            }
        }
        Shape::Text {
            at,
            text,
            color,
            size,
        } => {
            let text_layout = edit::TextLayout {
                ctx: painter.ctx(),
                px_to_ui,
            };
            painter.galley(map(*at), text_layout.galley(text, *size), color_of(*color));
        }
        Shape::Number { at, n, color } => {
            let c = map(*at);
            let radius = (12.0 * px_to_ui).max(4.0);
            painter.circle_filled(c, radius, color_of(*color));
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                n.to_string(),
                egui::FontId::proportional((12.0 * px_to_ui).max(6.0)),
                color_of(color.contrast_text()),
            );
        }
        Shape::Blur { a, b, .. } => {
            let Some(blur_texture) = blur_texture else {
                return;
            };
            let Some(r) = intersect(*bounds, edit::norm_rect(*a, *b)) else {
                return;
            };
            if r.width < 2 || r.height < 2 {
                return;
            }
            let rect = egui::Rect::from_min_max(
                map((r.x, r.y)),
                map((r.x + r.width as i32, r.y + r.height as i32)),
            );
            painter.image(
                blur_texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_input_separates_canvas_controls_and_popup_dismissal() {
        let ctx = egui::Context::default();
        let draw = |ctx: &egui::Context| {
            egui::CentralPanel::default().show(ctx, |_| {});
            egui::Area::new(egui::Id::new("test-toolbar"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(100.0, 100.0))
                .movable(false)
                .show(ctx, |ui| {
                    ui.add_sized([100.0, 34.0], egui::Button::new("Tool"));
                });
        };
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), draw);
        }
        for (pos, over_control) in [
            (egui::pos2(20.0, 20.0), false),
            (egui::pos2(120.0, 115.0), true),
        ] {
            let input = egui::RawInput {
                events: vec![egui::Event::PointerMoved(pos)],
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                assert_eq!(capture_ui_hit(ctx, false), over_control);
                assert!(
                    capture_ui_hit(ctx, true),
                    "popup dismissal must not reach canvas"
                );
                draw(ctx);
            });
        }
    }

    #[test]
    fn capture_theme_switches_every_surface() {
        let light = capture_palette(true);
        let dark = capture_palette(false);
        assert!(light.light);
        assert!(!dark.light);
        assert_eq!(capture_visuals(light).panel_fill, light.panel);
        assert_eq!(capture_visuals(dark).panel_fill, dark.panel);
        assert!(light.panel.r() > light.text.r());
        assert!(dark.panel.r() < dark.text.r());
        assert_ne!(light.surface, dark.surface);
        assert_ne!(light.popup, dark.popup);
        assert_ne!(light.swatch_edge, dark.swatch_edge);
    }

    #[test]
    fn handle_hit_test_matches_corner_and_edge_points() {
        let region = Region {
            x: 100,
            y: 100,
            width: 200,
            height: 100,
        };
        assert_eq!(hit_test_handle(region, 102, 98, 8), Some(Handle::TopLeft));
        assert_eq!(hit_test_handle(region, 200, 100, 8), Some(Handle::Top));
        assert_eq!(hit_test_handle(region, 305, 95, 8), Some(Handle::TopRight));
        assert_eq!(hit_test_handle(region, 300, 150, 8), Some(Handle::Right));
        assert_eq!(
            hit_test_handle(region, 300, 200, 8),
            Some(Handle::BottomRight)
        );
        assert_eq!(hit_test_handle(region, 200, 200, 8), Some(Handle::Bottom));
        assert_eq!(
            hit_test_handle(region, 100, 200, 8),
            Some(Handle::BottomLeft)
        );
        assert_eq!(hit_test_handle(region, 100, 150, 8), Some(Handle::Left));
        assert_eq!(hit_test_handle(region, 200, 150, 8), None);
    }

    #[test]
    fn intersect_drops_frame_outside_selection() {
        let frame = Region {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let selection = Region {
            x: 80,
            y: 90,
            width: 40,
            height: 20,
        };
        assert_eq!(
            intersect(frame, selection),
            Some(Region {
                x: 80,
                y: 90,
                width: 20,
                height: 10,
            })
        );
    }

    #[test]
    fn dimension_badge_avoids_toolbar_overlap_when_flipped_above() {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0));
        let sel = egui::Rect::from_min_size(egui::pos2(300.0, 720.0), egui::vec2(180.0, 60.0));
        let badge_size = egui::vec2(76.0, 24.0);
        let toolbar = egui::Rect::from_min_size(egui::pos2(100.0, 676.0), egui::vec2(380.0, 36.0));
        let pos = dimension_badge_pos(sel, badge_size, screen, Some(toolbar));
        let badge = egui::Rect::from_min_size(pos, badge_size);
        assert!(
            !badge.intersects(toolbar),
            "badge should not overlap toolbar"
        );
        assert!(screen.contains_rect(badge), "badge must stay on screen");
    }

    #[test]
    fn parse_hex_rgb_accepts_full_and_short_hex() {
        assert_eq!(parse_hex_rgb("#D4A94E"), Some((212, 169, 78)));
        assert_eq!(parse_hex_rgb("1b1d22"), Some((27, 29, 34)));
        assert_eq!(parse_hex_rgb("#f80"), Some((255, 136, 0)));
        assert_eq!(parse_hex_rgb("0Af"), Some((0, 170, 255)));
        assert_eq!(parse_hex_rgb("#12345"), None);
        assert_eq!(parse_hex_rgb("zzzzzz"), None);
    }

    fn stub_app() -> App {
        let (_ev_tx, events) = mpsc::channel();
        let (frame_sender, frames) = mpsc::channel();
        App {
            native: Native::stub(),
            events,
            frames,
            frame_sender,
            generation: 0,
            pending: false,
            overlay: Vec::new(),
            raw_frames: Vec::new(),
            selection: None,
            drag: DragState::Idle,
            overlay_shape_drag: None,
            editor: Editor::default(),
            editing: false,
            capture_at: None,
            capture_ms: 0,
            settings: true,
            hotkey: None,
            draft: Hotkey::default(),
            config: Config {
                save_dir: std::env::temp_dir()
                    .join("xscreen-ui-smoke-absent")
                    .to_string_lossy()
                    .into_owned(),
                ..Config::default()
            },
            folder_draft: String::new(),
            error: None,
            applying: false,
            toolbar_size: (0.0, TOOLBAR_H),
            page: Page::Album,
            album: crate::album::Album::default(),
            studio: ImageStudio::default(),
            editor_discard: None,
            capture_after: None,
            restore_main_after_capture: false,
            focus_main: false,
            main_window_pos: None,
            restore_window_pos: None,
            applied_window_theme: None,
        }
    }

    fn studio_raw_input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1020.0, 660.0),
            )),
            ..Default::default()
        }
    }

    fn studio_key_frame(app: &mut App, ctx: &egui::Context, key: egui::Key, pressed: bool) {
        let mut input = studio_raw_input();
        input.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(input, |ctx| ui::show(app, ctx));
    }

    #[test]
    fn editor_escape_protects_drawings_and_ignores_key_repeat() {
        let ctx = egui::Context::default();
        let mut app = stub_app();
        let _ = ctx.run(studio_raw_input(), |ctx| {
            app.studio.load_raw_bgra(
                Some("saved.png".into()),
                vec![120; 200 * 120 * 4],
                [200, 120],
                ctx,
            );
        });
        app.page = Page::Editor;
        app.studio.editor.select_tool(Tool::Pen);
        app.studio.editor.begin((20, 20));
        app.studio.editor.extend((70, 60));
        assert!(app.studio.editor.commit());
        app.studio.crop_mode = true;

        studio_key_frame(&mut app, &ctx, egui::Key::Escape, true);
        assert!(!app.studio.crop_mode);
        assert!(app.editor_discard.is_none());
        studio_key_frame(&mut app, &ctx, egui::Key::Escape, true);
        assert!(app.editor_discard.is_none(), "repeat must not open dialog");
        studio_key_frame(&mut app, &ctx, egui::Key::Escape, false);
        studio_key_frame(&mut app, &ctx, egui::Key::Escape, true);
        assert!(matches!(
            app.editor_discard,
            Some(ui::EditorDiscardAction::ReturnToAlbum)
        ));
        studio_key_frame(&mut app, &ctx, egui::Key::Escape, true);
        assert!(app.editor_discard.is_some(), "repeat must not close dialog");

        studio_key_frame(&mut app, &ctx, egui::Key::Enter, true);
        assert!(app.editor_discard.is_none(), "Enter defaults to No");
        assert!(app.page == Page::Editor);
        assert_eq!(app.studio.editor.shapes.len(), 1);
        studio_key_frame(&mut app, &ctx, egui::Key::Enter, false);
        studio_key_frame(&mut app, &ctx, egui::Key::Escape, false);

        studio_key_frame(&mut app, &ctx, egui::Key::Escape, true);
        assert!(app.editor_discard.is_some());
        studio_key_frame(&mut app, &ctx, egui::Key::Escape, false);
        studio_key_frame(&mut app, &ctx, egui::Key::Escape, true);
        assert!(app.editor_discard.is_none(), "Esc cancels dialog");
        assert_eq!(app.studio.editor.shapes.len(), 1);
        studio_key_frame(&mut app, &ctx, egui::Key::Escape, false);

        studio_key_frame(&mut app, &ctx, egui::Key::Escape, true);
        studio_key_frame(&mut app, &ctx, egui::Key::Tab, true);
        studio_key_frame(&mut app, &ctx, egui::Key::Enter, true);
        assert!(app.editor_discard.is_none());
        assert!(app.page == Page::Album, "Yes returns to Album");
        assert!(!app.studio.has_image());
    }

    #[test]
    fn editor_escape_returns_to_album_without_prompt_when_clean() {
        let ctx = egui::Context::default();
        let mut app = stub_app();
        let _ = ctx.run(studio_raw_input(), |ctx| {
            app.studio.load_raw_bgra(
                Some("saved.png".into()),
                vec![120; 200 * 120 * 4],
                [200, 120],
                ctx,
            );
        });
        app.page = Page::Editor;
        app.studio.editor.select_tool(Tool::Select);
        studio_key_frame(&mut app, &ctx, egui::Key::Escape, true);
        assert!(app.page == Page::Album);
        assert!(app.editor_discard.is_none());
        assert!(app.studio.has_image());
    }

    #[test]
    fn ui_show_smoke_all_pages_themes_and_languages() {
        let ctx = egui::Context::default();
        ui::install_fonts(&ctx);
        let mut app = stub_app();
        let raw_input = studio_raw_input();
        for frame in 0..6 {
            if frame == 1 {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            let out = ctx.run(raw_input.clone(), |ctx| {
                ui::show(&mut app, ctx);
            });
            let delay = out
                .viewport_output
                .get(&egui::ViewportId::ROOT)
                .map(|v| v.repaint_delay);
            if frame >= 3 {
                assert!(
                    delay.is_some_and(|d| d >= std::time::Duration::from_millis(500)),
                    "steady-state ui::show must not spin repaints (frame {frame}: {delay:?})"
                );
            }
        }
        // Load an image + shapes into Studio and verify Editor, Album, Settings across EN/TR and Dark/Light
        let _ = ctx.run(raw_input.clone(), |ctx| {
            app.studio
                .load_raw_bgra(None, vec![120u8; 200 * 120 * 4], [200, 120], ctx);
            app.studio.editor.push(Shape::Arrow {
                a: (20, 20),
                b: (150, 90),
                color: edit::Rgba::rgb(235, 59, 46),
                width: 4.0,
            });
            app.studio.editor.select_shape_index(Some(0));
        });
        for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Tr] {
            for light in [false, true] {
                app.config.lang = lang;
                app.config.light_theme = light;
                for page in [Page::Album, Page::Editor, Page::Settings] {
                    app.page = page;
                    let _ = ctx.run(raw_input.clone(), |ctx| {
                        ui::show(&mut app, ctx);
                    });
                }
            }
        }
    }

    #[test]
    fn degenerate_and_flat_shapes_keep_bounded_tessellation_geometry() {
        let ctx = egui::Context::default();
        let bounds = Region {
            x: 0,
            y: 0,
            width: 500,
            height: 500,
        };
        let color = edit::Rgba::rgb(255, 80, 60);
        let mut cases = Vec::new();
        for d in [0, 1, 2] {
            for width in [3.0_f32, 12.0] {
                for (a, b) in [((100, 100), (300, 100 + d)), ((100, 100), (100 + d, 300))] {
                    cases.push(Shape::Triangle {
                        a,
                        b,
                        color,
                        width,
                        filled: false,
                    });
                    cases.push(Shape::Ellipse {
                        a,
                        b,
                        color,
                        width,
                        filled: false,
                    });
                    cases.push(Shape::Rect {
                        a,
                        b,
                        color,
                        width,
                        filled: false,
                    });
                }
            }
        }
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(500.0, 500.0),
            )),
            ..Default::default()
        };
        let out = ctx.run(raw_input, |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("flat_shapes_test"),
            ));
            for shape in &cases {
                paint_shape_mapped(&painter, &bounds, egui::Pos2::ZERO, 1.0, shape, None);
            }
        });
        let primitives = ctx.tessellate(out.shapes, out.pixels_per_point);
        let mut vertex_count = 0usize;
        for clipped in primitives {
            if let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive {
                for v in &mesh.vertices {
                    vertex_count += 1;
                    assert!(
                        v.pos.x.is_finite()
                            && v.pos.y.is_finite()
                            && (90.0..=310.0).contains(&v.pos.x)
                            && (90.0..=310.0).contains(&v.pos.y),
                        "vertex out of expected bounds [90..=310]: {:?}",
                        v.pos
                    );
                }
            }
        }
        assert!(vertex_count > 0, "expected tessellated vertices for shapes");
    }

    #[test]
    fn dashed_text_selection_surrounds_rendered_glyphs() {
        let ctx = egui::Context::default();
        ui::install_fonts(&ctx);
        let bounds = Region {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let shape = Shape::Text {
            at: (-1800, 240),
            text: "ENESSSSSSSSSSSSSSSSSSSS".into(),
            color: edit::Rgba::rgb(255, 64, 48),
            size: 60.0,
        };
        for scale in [1.0, 1.5, 2.0] {
            let out = ctx.run(egui::RawInput::default(), |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Background,
                    egui::Id::new("text_selection_test"),
                ));
                paint_shape_mapped(
                    &painter,
                    &bounds,
                    egui::Pos2::ZERO,
                    1.0 / scale,
                    &shape,
                    None,
                );
                studio::paint_shape_selection(
                    &painter,
                    &shape,
                    None,
                    Color32::LIGHT_BLUE,
                    edit::TextLayout {
                        ctx,
                        px_to_ui: 1.0 / scale,
                    },
                    |pt| to_local(&bounds, scale, pt),
                );
            });
            let glyphs = out
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(text) => Some(text.visual_bounding_rect()),
                    _ => None,
                })
                .expect("rendered glyphs");
            let handles = out
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Circle(circle) if circle.fill == Color32::WHITE => {
                        Some(circle.center)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(handles.len(), 4);
            let frame = egui::Rect::from_two_pos(handles[0], handles[2]);
            assert!(
                frame.contains_rect(glyphs),
                "selection frame {frame:?} clips glyphs {glyphs:?}"
            );
            assert!(
                frame.right() - glyphs.right() < 10.0,
                "frame should end near the last glyph"
            );
            assert!(
                out.shapes.iter().any(|s| matches!(s.shape,
                    egui::Shape::LineSegment { points, .. }
                        if points.iter().all(|p| p.x == frame.right())
                )),
                "right dashed edge should align with the handles"
            );
        }
    }

    #[test]
    fn compact_magnifier_renders_cleanly_at_monitor_edges_and_near_badge() {
        let ctx = egui::Context::default();
        ui::install_fonts(&ctx);
        let bounds = Region {
            x: 0,
            y: 0,
            width: 320,
            height: 200,
        };
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 200.0));
        let badge = egui::Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(72.0, 22.0));
        let raw_input = egui::RawInput {
            screen_rect: Some(screen_rect),
            ..Default::default()
        };
        let out = ctx.run(raw_input, |ctx| {
            let tex = ctx.load_texture(
                "mag-test",
                egui::ColorImage::new([320, 200], vec![Color32::from_rgb(12, 12, 12); 320 * 200]),
                egui::TextureOptions::NEAREST,
            );
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("mag_test_layer"),
            ));
            for &(cx, cy) in &[(0, 0), (319, 199), (10, 10), (160, 100)] {
                paint_compact_magnifier(
                    &painter,
                    tex.id(),
                    bounds,
                    1.0,
                    (cx, cy),
                    (12, 12, 12),
                    screen_rect,
                    Some(badge),
                    capture_palette(false),
                );
            }
        });
        let primitives = ctx.tessellate(out.shapes, out.pixels_per_point);
        assert!(!primitives.is_empty());
    }
}
