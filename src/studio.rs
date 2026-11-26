//! ShareX-inspired In-App Image Editor state, document history, crop, zoom/pan, and canvas rendering.
use crate::{
    album,
    capture::{Frame, Region},
    edit::{self, Editor, Rgba, Shape, ShapeHandle, Tool},
    i18n::{Lang, Strings},
    native,
};
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke, Vec2};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::mpsc,
};

#[derive(Clone, Debug)]
pub enum ShapeDrag {
    Moving {
        index: usize,
        last_mouse: (i32, i32),
        moved: bool,
    },
    Resizing {
        index: usize,
        handle: ShapeHandle,
        start_mouse: (i32, i32),
        origin: Shape,
        moved: bool,
    },
}

#[derive(Clone, Debug)]
pub enum CropDrag {
    Creating {
        start: (i32, i32),
    },
    Moving {
        start_mouse: (i32, i32),
        origin: Region,
    },
    Resizing {
        handle: ShapeHandle,
        start_mouse: (i32, i32),
        origin: Region,
    },
}

struct CanvasBackup {
    bgra: Vec<u8>,
    width: u32,
    height: u32,
    offset: (i32, i32),
    shapes: Vec<Shape>,
    next_number: u32,
}

enum StudioMsg {
    Loaded(u64, Option<PathBuf>, Result<(Vec<u8>, [u32; 2]), String>),
    OriginSaved(u64, Result<PathBuf, String>),
    Saved {
        generation: u64,
        signature: u64,
        path: PathBuf,
        is_copy: bool,
        result: Result<(), String>,
    },
    Copied(Result<(), String>),
}

pub struct ImageStudio {
    pub source_path: Option<PathBuf>,
    pub from_capture: bool,
    pub base_bgra: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub texture: Option<egui::TextureHandle>,
    committed_bgra: Vec<u8>,
    pub blur_patches: Vec<egui::TextureHandle>,
    shapes_hash: u64,
    pub draft_blur: Option<(u64, egui::TextureHandle)>,
    pub editor: Editor,
    pub shape_drag: Option<ShapeDrag>,
    pub crop_mode: bool,
    pub crop_rect: Option<Region>,
    pub crop_drag: Option<CropDrag>,
    crop_undo: Vec<CanvasBackup>,
    crop_redo: Vec<CanvasBackup>,
    canvas_offset: (i32, i32),
    pub zoom: f32,
    pub fit_mode: bool,
    pub pan: Vec2,
    nearest_filter: bool,
    pub loading: bool,
    pub saving: bool,
    origin_saving: bool,
    pending_save: Option<(bool, Option<PathBuf>, Lang)>,
    needs_initial_save: bool,
    pub error: Option<String>,
    pub cursor_img_pos: Option<(i32, i32)>,
    saved_sig: u64,
    generation: u64,
    tx: mpsc::Sender<StudioMsg>,
    rx: mpsc::Receiver<StudioMsg>,
}

impl Default for ImageStudio {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        let mut editor = Editor::default();
        editor.tool = Tool::Select;
        Self {
            source_path: None,
            from_capture: false,
            base_bgra: Vec::new(),
            width: 0,
            height: 0,
            texture: None,
            committed_bgra: Vec::new(),
            blur_patches: Vec::new(),
            shapes_hash: 0,
            draft_blur: None,
            editor,
            shape_drag: None,
            crop_mode: false,
            crop_rect: None,
            crop_drag: None,
            crop_undo: Vec::new(),
            crop_redo: Vec::new(),
            canvas_offset: (0, 0),
            zoom: 1.0,
            fit_mode: true,
            pan: Vec2::ZERO,
            nearest_filter: false,
            loading: false,
            saving: false,
            origin_saving: false,
            pending_save: None,
            needs_initial_save: false,
            error: None,
            cursor_img_pos: None,
            saved_sig: 0,
            generation: 0,
            tx,
            rx,
        }
    }
}

pub struct StudioOutcome {
    pub notice: Option<String>,
    pub invalidate_album_path: Option<PathBuf>,
    pub refresh_album: bool,
}

impl ImageStudio {
    pub fn has_image(&self) -> bool {
        self.width > 0 && self.height > 0 && !self.base_bgra.is_empty()
    }

    pub fn is_modified(&self) -> bool {
        self.has_image() && (self.needs_initial_save || self.state_signature() != self.saved_sig)
    }

    pub fn discard_image(&mut self) {
        let color = self.editor.color;
        let width = self.editor.width;
        let custom = self.editor.custom;
        *self = Self::default();
        self.editor.color = color;
        self.editor.width = width;
        self.editor.custom = custom;
    }

    fn state_signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.width.hash(&mut h);
        self.height.hash(&mut h);
        self.canvas_offset.hash(&mut h);
        shapes_hash_value(&self.editor.shapes).hash(&mut h);
        h.finish()
    }

    pub fn open_file(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.generation = self.generation.wrapping_add(1);
        let seq = self.generation;
        self.loading = true;
        self.origin_saving = false;
        self.saving = false;
        self.pending_save = None;
        self.error = None;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let res = album::decode_bgra(&path);
            let _ = tx.send(StudioMsg::Loaded(seq, Some(path), res));
            ctx.request_repaint();
        });
    }

    pub fn paste_from_clipboard(&mut self, lang: Lang, ctx: &egui::Context) {
        self.generation = self.generation.wrapping_add(1);
        let seq = self.generation;
        self.loading = true;
        self.origin_saving = false;
        self.saving = false;
        self.pending_save = None;
        self.error = None;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let res = native::read_clipboard_bgra(lang);
            let _ = tx.send(StudioMsg::Loaded(seq, None, res));
            ctx.request_repaint();
        });
    }

    /// Load a fresh screen capture into the Editor AND persist the clean original capture to Album
    /// so `Save` updates it in place while `Save copy` keeps the original intact alongside the new copy.
    #[allow(clippy::too_many_arguments)]
    pub fn load_captured_bgra(
        &mut self,
        bgra: Vec<u8>,
        size: [u32; 2],
        shapes: Vec<Shape>,
        next_number: u32,
        save_folder: Option<PathBuf>,
        lang: Lang,
        ctx: &egui::Context,
    ) {
        self.generation = self.generation.wrapping_add(1);
        let seq = self.generation;
        let raw_for_disk = bgra.clone();
        self.load_raw_bgra(None, bgra, size, ctx);
        self.from_capture = true;
        self.editor.shapes = shapes;
        self.editor.next_number = next_number;
        self.editor.tool = Tool::Select;
        self.sync_blurs(ctx, true);

        self.origin_saving = true;
        self.saving = true;
        self.pending_save = None;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let s = Strings::new(lang);
            let res = native::encode_png(size[0], size[1], &raw_for_disk)
                .and_then(|bytes| native::save_png(&bytes, save_folder.as_deref()))
                .map_err(|e| s.err_save_file(&e));
            let _ = tx.send(StudioMsg::OriginSaved(seq, res));
            ctx.request_repaint();
        });
    }

    pub fn load_raw_bgra(
        &mut self,
        source_path: Option<PathBuf>,
        bgra: Vec<u8>,
        size: [u32; 2],
        ctx: &egui::Context,
    ) {
        self.source_path = source_path.clone();
        self.from_capture = false;
        self.width = size[0];
        self.height = size[1];
        self.base_bgra = bgra;
        self.committed_bgra.clear();
        self.blur_patches.clear();
        self.draft_blur = None;
        let prev_color = self.editor.color;
        let prev_width = self.editor.width;
        let prev_custom = self.editor.custom;
        let mut ed = Editor::default();
        ed.tool = Tool::Arrow;
        ed.color = prev_color;
        ed.width = prev_width;
        ed.custom = prev_custom;
        self.editor = ed;
        self.shape_drag = None;
        self.crop_mode = false;
        self.crop_rect = None;
        self.crop_drag = None;
        self.crop_undo.clear();
        self.crop_redo.clear();
        self.canvas_offset = (0, 0);
        self.fit_mode = true;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.loading = false;
        self.origin_saving = false;
        self.saving = false;
        self.pending_save = None;
        self.error = None;
        self.needs_initial_save = source_path.is_none();
        self.rebuild_base_texture(ctx);
        self.shapes_hash = shapes_hash_value(&[]);
        self.saved_sig = self.state_signature();
    }

    fn rebuild_base_texture(&mut self, ctx: &egui::Context) {
        if !self.has_image() {
            self.texture = None;
            return;
        }
        let img = bgra_to_color_image(&self.base_bgra, self.width, self.height);
        self.nearest_filter = self.zoom >= 2.0;
        let opts = if self.nearest_filter {
            egui::TextureOptions::NEAREST
        } else {
            egui::TextureOptions::LINEAR
        };
        if let Some(tex) = &mut self.texture {
            tex.set(img, opts);
        } else {
            self.texture = Some(ctx.load_texture("studio-base", img, opts));
        }
    }

    pub fn sync_zoom_filter(&mut self, ctx: &egui::Context) {
        let want_nearest = self.zoom >= 2.0;
        if self.has_image() && want_nearest != self.nearest_filter {
            self.rebuild_base_texture(ctx);
        }
    }

    pub fn poll(&mut self, ctx: &egui::Context, s: &Strings) -> StudioOutcome {
        let mut out = StudioOutcome {
            notice: None,
            invalidate_album_path: None,
            refresh_album: false,
        };
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                StudioMsg::Loaded(seq, path, res) if seq == self.generation => {
                    self.loading = false;
                    match res {
                        Ok((bgra, size)) => self.load_raw_bgra(path, bgra, size, ctx),
                        Err(e) => self.error = Some(e),
                    }
                }
                StudioMsg::OriginSaved(seq, res) if seq == self.generation => {
                    self.origin_saving = false;
                    self.saving = false;
                    match res {
                        Ok(path) => {
                            self.source_path = Some(path.clone());
                            self.needs_initial_save = false;
                            out.invalidate_album_path = Some(path);
                            out.refresh_album = true;
                        }
                        Err(e) => self.error = Some(e),
                    }
                    if let Some((as_copy, folder, lang)) = self.pending_save.take() {
                        self.save_image(as_copy, folder, lang, ctx);
                    }
                }
                StudioMsg::Saved {
                    generation,
                    signature,
                    path,
                    is_copy,
                    result,
                } => {
                    let current = generation == self.generation;
                    if current {
                        self.saving = false;
                    }
                    match result {
                        Ok(()) => {
                            if current {
                                self.source_path = Some(path.clone());
                                self.needs_initial_save = false;
                                self.saved_sig = signature;
                            }
                            out.invalidate_album_path = Some(path);
                            out.refresh_album = true;
                            out.notice = Some(if is_copy {
                                s.saved_copy_notice().to_owned()
                            } else {
                                s.saved_notice().to_owned()
                            });
                        }
                        Err(e) if current => self.error = Some(e),
                        Err(_) => {}
                    }
                }
                StudioMsg::Copied(res) => match res {
                    Ok(()) => out.notice = Some(s.copied_notice().to_owned()),
                    Err(e) => self.error = Some(e),
                },
                _ => {}
            }
        }
        out
    }

    pub fn can_undo(&self) -> bool {
        self.editor.can_undo() || !self.crop_undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        self.editor.can_redo() || !self.crop_redo.is_empty()
    }

    pub fn undo(&mut self, ctx: &egui::Context) {
        self.editor.commit_text();
        if self.editor.can_undo() {
            self.editor.undo();
            self.sync_blurs(ctx, true);
        } else if let Some(prev) = self.crop_undo.pop() {
            let cur = CanvasBackup {
                bgra: std::mem::replace(&mut self.base_bgra, prev.bgra),
                width: std::mem::replace(&mut self.width, prev.width),
                height: std::mem::replace(&mut self.height, prev.height),
                offset: std::mem::replace(&mut self.canvas_offset, prev.offset),
                shapes: std::mem::replace(&mut self.editor.shapes, prev.shapes),
                next_number: std::mem::replace(&mut self.editor.next_number, prev.next_number),
            };
            self.crop_redo.push(cur);
            self.editor.selected = None;
            self.rebuild_base_texture(ctx);
            self.shapes_hash = 0;
            self.sync_blurs(ctx, true);
        }
    }

    pub fn redo(&mut self, ctx: &egui::Context) {
        self.editor.commit_text();
        if self.editor.can_redo() {
            self.editor.redo();
            self.sync_blurs(ctx, true);
        } else if let Some(next) = self.crop_redo.pop() {
            let cur = CanvasBackup {
                bgra: std::mem::replace(&mut self.base_bgra, next.bgra),
                width: std::mem::replace(&mut self.width, next.width),
                height: std::mem::replace(&mut self.height, next.height),
                offset: std::mem::replace(&mut self.canvas_offset, next.offset),
                shapes: std::mem::replace(&mut self.editor.shapes, next.shapes),
                next_number: std::mem::replace(&mut self.editor.next_number, next.next_number),
            };
            self.crop_undo.push(cur);
            self.editor.selected = None;
            self.rebuild_base_texture(ctx);
            self.shapes_hash = 0;
            self.sync_blurs(ctx, true);
        }
    }

    pub fn apply_crop(&mut self, ctx: &egui::Context) -> bool {
        let Some(crop) = self.crop_rect.take() else {
            self.crop_mode = false;
            return false;
        };
        self.crop_mode = false;
        self.crop_drag = None;
        if crop.width < 4 || crop.height < 4 {
            return false;
        }
        let Some((cropped_bgra, [nw, nh])) =
            edit::crop_bgra(&self.base_bgra, self.width, self.height, crop)
        else {
            return false;
        };
        if [nw, nh] == [self.width, self.height] {
            return false;
        }
        self.editor.commit_text();
        let x = crop.x.max(0);
        let y = crop.y.max(0);
        let (ox, oy) = self.canvas_offset;
        let backup = CanvasBackup {
            bgra: std::mem::replace(&mut self.base_bgra, cropped_bgra),
            width: std::mem::replace(&mut self.width, nw),
            height: std::mem::replace(&mut self.height, nh),
            offset: std::mem::replace(&mut self.canvas_offset, (ox + x, oy + y)),
            shapes: self.editor.shapes.clone(),
            next_number: self.editor.next_number,
        };
        if self.crop_undo.len() >= 12 {
            self.crop_undo.remove(0);
        }
        self.crop_undo.push(backup);
        self.crop_redo.clear();
        for shape in &mut self.editor.shapes {
            edit::translate_shape(shape, -x, -y);
        }
        self.editor.selected = None;
        self.pan = Vec2::ZERO;
        self.fit_mode = true;
        self.rebuild_base_texture(ctx);
        self.shapes_hash = 0;
        self.sync_blurs(ctx, true);
        true
    }

    pub fn bake_flattened(&mut self) -> Option<Frame> {
        if !self.has_image() {
            return None;
        }
        self.editor.commit_text();
        let bounds = Region {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        };
        let mut bgra = self.base_bgra.clone();
        edit::bake(&mut bgra, bounds, &self.editor.shapes);
        Some(Frame { bounds, bgra })
    }

    pub fn copy_to_clipboard(&mut self, copy_png: bool, lang: Lang, ctx: &egui::Context) {
        let Some(frame) = self.bake_flattened() else {
            return;
        };
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let err_msg = Strings::new(lang).err_copy_failed().to_owned();
        std::thread::spawn(move || {
            let res = native::copy_to_clipboard(&frame, copy_png).map_err(|_| err_msg);
            let _ = tx.send(StudioMsg::Copied(res));
            ctx.request_repaint();
        });
    }

    pub fn save_image(
        &mut self,
        as_copy: bool,
        save_folder: Option<PathBuf>,
        lang: Lang,
        ctx: &egui::Context,
    ) {
        if self.origin_saving {
            self.pending_save = Some((as_copy, save_folder, lang));
            self.saving = true;
            return;
        }
        if self.saving {
            return;
        }
        let Some(frame) = self.bake_flattened() else {
            return;
        };
        self.saving = true;
        self.error = None;
        let generation = self.generation;
        let signature = self.state_signature();
        let overwrite_target = (!as_copy).then(|| self.source_path.clone()).flatten();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let s = Strings::new(lang);
            let encoded = native::encode_png(frame.bounds.width, frame.bounds.height, &frame.bgra)
                .map_err(|e| s.err_save_file(&e));
            match encoded {
                Ok(bytes) => {
                    if let Some(target) = overwrite_target {
                        let res =
                            native::save_png_at(&bytes, &target).map_err(|e| s.err_save_file(&e));
                        let _ = tx.send(StudioMsg::Saved {
                            generation,
                            signature,
                            path: target,
                            is_copy: false,
                            result: res,
                        });
                    } else {
                        match native::save_png(&bytes, save_folder.as_deref()) {
                            Ok(new_path) => {
                                let _ = tx.send(StudioMsg::Saved {
                                    generation,
                                    signature,
                                    path: new_path,
                                    is_copy: as_copy,
                                    result: Ok(()),
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(StudioMsg::Saved {
                                    generation,
                                    signature,
                                    path: PathBuf::new(),
                                    is_copy: as_copy,
                                    result: Err(s.err_save_file(&e)),
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(StudioMsg::Saved {
                        generation,
                        signature,
                        path: PathBuf::new(),
                        is_copy: as_copy,
                        result: Err(e),
                    });
                }
            }
            ctx.request_repaint();
        });
    }

    /// Recompute blur preview textures only when blur shapes actually exist and changed.
    pub fn sync_blurs(&mut self, ctx: &egui::Context, force: bool) {
        if !self.has_image() {
            return;
        }
        let has_committed_blur = self
            .editor
            .shapes
            .iter()
            .any(|s| matches!(s, Shape::Blur { .. }));
        let has_draft_blur = matches!(self.editor.draft, Some(Shape::Blur { .. }));

        if !has_committed_blur && !has_draft_blur {
            self.blur_patches.clear();
            self.draft_blur = None;
            self.shapes_hash = shapes_hash_value(&self.editor.shapes);
            return;
        }

        let next_hash = shapes_hash_value(&self.editor.shapes);
        if force
            || self.shapes_hash != next_hash
            || self.committed_bgra.len() != self.base_bgra.len()
        {
            if self.committed_bgra.len() != self.base_bgra.len() {
                self.committed_bgra = self.base_bgra.clone();
            } else {
                self.committed_bgra.copy_from_slice(&self.base_bgra);
            }
            let patches = edit::apply_shapes_at_1x(
                &mut self.committed_bgra,
                self.width as i32,
                self.height as i32,
                (0, 0),
                &self.editor.shapes,
            );
            self.blur_patches = patches
                .into_iter()
                .enumerate()
                .map(|(i, p)| {
                    ctx.load_texture(
                        format!("studio-blur-{i}-{next_hash:016x}"),
                        bgra_to_color_image(&p.bgra, p.width, p.height),
                        egui::TextureOptions::NEAREST,
                    )
                })
                .collect();
            self.shapes_hash = next_hash;
        }

        if let Some(Shape::Blur { a, b, kind }) = &self.editor.draft {
            let mut dh = DefaultHasher::new();
            next_hash.hash(&mut dh);
            a.hash(&mut dh);
            b.hash(&mut dh);
            kind.hash(&mut dh);
            let d_sig = dh.finish();
            if self.draft_blur.as_ref().map(|(h, _)| *h) != Some(d_sig) {
                if let Some(patch) = edit::blur_patch_at_1x(
                    &self.committed_bgra,
                    self.width as i32,
                    self.height as i32,
                    (0, 0),
                    *a,
                    *b,
                    *kind,
                ) {
                    let img = bgra_to_color_image(&patch.bgra, patch.width, patch.height);
                    if let Some((h, tex)) = &mut self.draft_blur {
                        tex.set(img, egui::TextureOptions::NEAREST);
                        *h = d_sig;
                    } else {
                        let tex = ctx.load_texture(
                            "studio-blur-draft",
                            img,
                            egui::TextureOptions::NEAREST,
                        );
                        self.draft_blur = Some((d_sig, tex));
                    }
                } else {
                    self.draft_blur = None;
                }
            }
        } else {
            self.draft_blur = None;
        }
    }
}

pub fn bgra_to_color_image(bgra: &[u8], width: u32, height: u32) -> egui::ColorImage {
    let pixels = bgra
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| Color32::from_rgba_unmultiplied(p[2], p[1], p[0], p[3]))
        .collect();
    egui::ColorImage::new([width as usize, height as usize], pixels)
}

pub fn shapes_hash_value(shapes: &[Shape]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for s in shapes {
        hash_shape(s, &mut hasher);
    }
    hasher.finish()
}

pub fn hash_shape(shape: &Shape, hasher: &mut impl Hasher) {
    std::mem::discriminant(shape).hash(hasher);
    let hr = |c: &Rgba, h: &mut _| {
        c.r.hash(h);
        c.g.hash(h);
        c.b.hash(h);
        c.a.hash(h);
    };
    match shape {
        Shape::Arrow { a, b, color, width } | Shape::Line { a, b, color, width } => {
            a.hash(hasher);
            b.hash(hasher);
            hr(color, hasher);
            width.to_bits().hash(hasher);
        }
        Shape::Rect {
            a,
            b,
            color,
            width,
            filled,
        }
        | Shape::Ellipse {
            a,
            b,
            color,
            width,
            filled,
        }
        | Shape::Triangle {
            a,
            b,
            color,
            width,
            filled,
        } => {
            a.hash(hasher);
            b.hash(hasher);
            hr(color, hasher);
            width.to_bits().hash(hasher);
            filled.hash(hasher);
        }
        Shape::Pen {
            points,
            color,
            width,
        }
        | Shape::Marker {
            points,
            color,
            width,
        } => {
            points.hash(hasher);
            hr(color, hasher);
            width.to_bits().hash(hasher);
        }
        Shape::Text {
            at,
            text,
            color,
            size,
        } => {
            at.hash(hasher);
            text.hash(hasher);
            hr(color, hasher);
            size.to_bits().hash(hasher);
        }
        Shape::Number { at, n, color } => {
            at.hash(hasher);
            n.hash(hasher);
            hr(color, hasher);
        }
        Shape::Blur { a, b, kind } => {
            a.hash(hasher);
            b.hash(hasher);
            kind.hash(hasher);
        }
    }
}

pub fn cursor_for_shape_handle(h: ShapeHandle) -> egui::CursorIcon {
    match h {
        ShapeHandle::Start | ShapeHandle::End => egui::CursorIcon::Crosshair,
        ShapeHandle::TopLeft | ShapeHandle::BottomRight => egui::CursorIcon::ResizeNwSe,
        ShapeHandle::TopRight | ShapeHandle::BottomLeft => egui::CursorIcon::ResizeNeSw,
        ShapeHandle::Top | ShapeHandle::Bottom => egui::CursorIcon::ResizeVertical,
        ShapeHandle::Left | ShapeHandle::Right => egui::CursorIcon::ResizeHorizontal,
    }
}

/// Draw ShareX-style dashed selection frame and crisp circular handles for a selected shape.
pub fn paint_shape_selection(
    painter: &egui::Painter,
    shape: &Shape,
    hovered_handle: Option<ShapeHandle>,
    accent: Color32,
    text_layout: edit::TextLayout<'_>,
    map_pt: impl Fn((i32, i32)) -> Pos2,
) {
    // Draw dashed bounding box for non-line shapes, or subtle shaft highlight for Line/Arrow.
    match shape {
        Shape::Line { a, b, .. } | Shape::Arrow { a, b, .. } => {
            let pa = map_pt(*a);
            let pb = map_pt(*b);
            paint_dashed_segment(
                painter,
                pa,
                pb,
                5.0,
                4.0,
                Stroke::new(1.0_f32, Color32::from_white_alpha(210)),
            );
        }
        _ => {
            if let Some(r) = edit::shape_bounds(shape, text_layout) {
                // Text bounds already include padding. Put the frame on the handles.
                let extra = if matches!(shape, Shape::Text { .. }) {
                    0.0
                } else {
                    2.0
                };
                let min = map_pt((r.x, r.y)) - Vec2::splat(extra);
                let max =
                    map_pt((r.x + r.width as i32, r.y + r.height as i32)) + Vec2::splat(extra);
                let rect = Rect::from_min_max(min, max);
                paint_dashed_rect(painter, rect, accent);
            }
        }
    }

    // Draw circular handles on top
    for (kind, pt) in edit::shape_handles(shape, text_layout) {
        let pos = map_pt(pt);
        let hot = hovered_handle == Some(kind);
        let radius = if hot { 5.0 } else { 4.0 };
        painter.circle_filled(pos, radius + 1.2, Color32::from_black_alpha(150));
        painter.circle_filled(pos, radius, Color32::WHITE);
        painter.circle_stroke(
            pos,
            radius,
            Stroke::new(
                if hot { 1.8_f32 } else { 1.2_f32 },
                if hot {
                    accent
                } else {
                    Color32::from_rgb(20, 24, 30)
                },
            ),
        );
    }
}

/// Shared in-place editor for capture and studio. Keep the text origin fixed:
/// Area's screen constraints and TextEdit's default horizontal scrolling would
/// otherwise move the input or trap it at its previous frame's width.
#[allow(clippy::too_many_arguments)]
pub fn live_text_input(
    ctx: &egui::Context,
    id: egui::Id,
    origin: Pos2,
    clip_rect: Rect,
    text: &mut String,
    font_size: f32,
    ink: Color32,
    accent: Color32,
    placeholder: &str,
) -> egui::Response {
    let font = egui::FontId::proportional(font_size.max(1.0));
    let width = ctx.fonts_mut(|fonts| {
        let content = if text.is_empty() {
            placeholder
        } else {
            text.as_str()
        };
        (fonts
            .layout_no_wrap(content.to_owned(), font.clone(), ink)
            .size()
            .x
            + 12.0)
            .max(80.0)
    });
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .movable(false)
        .fixed_pos(origin)
        .constrain(false)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.set_clip_rect(clip_rect);
            ui.set_width(width);
            let output = egui::TextEdit::singleline(text)
                .id(id.with("input"))
                .hint_text(placeholder)
                .font(font)
                .text_color(ink)
                .background_color(Color32::TRANSPARENT)
                .frame(false)
                .margin(egui::Margin::ZERO)
                .clip_text(false)
                .desired_width(width)
                .show(ui);
            let mut response = output.response;
            // Text events are applied inside TextEdit::show, after the initial
            // allocation. Include that new galley in the frame in this frame too.
            response.rect.max.x =
                response.rect.min.x + response.rect.width().max(output.galley.size().x + 12.0);
            response.rect.max.y =
                response.rect.min.y + response.rect.height().max(output.galley.size().y);
            ui.expand_to_include_rect(response.rect);
            paint_dashed_rect(
                ui.painter(),
                response.rect.expand2(egui::vec2(4.0, 2.0)),
                accent,
            );
            response
        })
        .inner
}

pub fn paint_dashed_rect(painter: &egui::Painter, rect: Rect, accent: Color32) {
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0_f32, Color32::from_black_alpha(140)),
        egui::StrokeKind::Outside,
    );
    let dash = 5.0;
    let gap = 4.0;
    let stroke = Stroke::new(1.2_f32, Color32::WHITE);
    let accent_stroke = Stroke::new(1.2_f32, accent);
    let corners = [
        (rect.left_top(), rect.right_top()),
        (rect.right_top(), rect.right_bottom()),
        (rect.right_bottom(), rect.left_bottom()),
        (rect.left_bottom(), rect.left_top()),
    ];
    for (a, b) in corners {
        paint_dashed_segment(painter, a, b, dash, gap, stroke);
        paint_dashed_segment_offset(painter, a, b, dash, gap, dash, accent_stroke);
    }
}

fn paint_dashed_segment(
    painter: &egui::Painter,
    a: Pos2,
    b: Pos2,
    dash: f32,
    gap: f32,
    stroke: Stroke,
) {
    paint_dashed_segment_offset(painter, a, b, dash, gap, 0.0, stroke);
}

fn paint_dashed_segment_offset(
    painter: &egui::Painter,
    a: Pos2,
    b: Pos2,
    dash: f32,
    gap: f32,
    offset: f32,
    stroke: Stroke,
) {
    let delta = b - a;
    let len = delta.length();
    if len < 1.0 {
        return;
    }
    let dir = delta / len;
    let mut t = offset;
    let step = (dash + gap).max(2.0);
    while t < len {
        let end = (t + dash).min(len);
        if end > t {
            painter.line_segment([a + dir * t, a + dir * end], stroke);
        }
        t += step;
    }
}

/// Paint subtle checkerboard backdrop inside `rect` so image bounds and alpha stand out cleanly.
pub fn paint_checkerboard(painter: &egui::Painter, rect: Rect, light: bool) {
    let (base, alt) = if light {
        (
            Color32::from_rgb(238, 240, 243),
            Color32::from_rgb(222, 226, 231),
        )
    } else {
        (Color32::from_rgb(30, 31, 35), Color32::from_rgb(38, 40, 45))
    };
    painter.rect_filled(rect, 0.0, base);
    let clipped = painter.with_clip_rect(rect);
    let cell = 12.0_f32;
    let cols = ((rect.width() / cell).ceil() as i32).min(240);
    let rows = ((rect.height() / cell).ceil() as i32).min(180);
    // If zoomed out with huge cell counts, skip individual cells to keep GPU vertex count minimal.
    if cols * rows > 12_000 {
        return;
    }
    for ry in 0..rows {
        for cx in 0..cols {
            if (cx + ry) % 2 == 1 {
                let min = Pos2::new(rect.min.x + cx as f32 * cell, rect.min.y + ry as f32 * cell);
                let max = Pos2::new(
                    (min.x + cell).min(rect.max.x),
                    (min.y + cell).min(rect.max.y),
                );
                clipped.rect_filled(Rect::from_min_max(min, max), 0.0, alt);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_input_frame(
        ctx: &egui::Context,
        origin: Pos2,
        text: &mut String,
        font_size: f32,
        events: Vec<egui::Event>,
    ) -> (Rect, egui::FullOutput) {
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut rect = Rect::NOTHING;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ctx| {
                let response = live_text_input(
                    ctx,
                    egui::Id::new("test_text"),
                    origin,
                    screen,
                    text,
                    font_size,
                    Color32::WHITE,
                    Color32::LIGHT_BLUE,
                    "Text",
                );
                rect = response.rect;
                response.request_focus();
            },
        );
        (rect, output)
    }

    #[test]
    fn live_text_stays_at_click_and_grows_with_typing_and_font_size() {
        for scale in [0.5, 1.0, 2.0] {
            let ctx = egui::Context::default();
            let origin = egui::pos2(240.0, 180.0);
            let mut text = String::new();
            for _ in 0..2 {
                text_input_frame(&ctx, origin, &mut text, 24.0 * scale, vec![]);
            }
            let (empty, _) = text_input_frame(&ctx, origin, &mut text, 24.0 * scale, vec![]);
            let (typed, output) = text_input_frame(
                &ctx,
                origin,
                &mut text,
                24.0 * scale,
                vec![egui::Event::Text(
                    "Türkçe metin yazdıkça kutu genişlemeli".into(),
                )],
            );
            assert_eq!(typed.min, origin);
            assert!(typed.width() > empty.width());
            let text_width = ctx.fonts_mut(|f| {
                f.layout_no_wrap(
                    text.clone(),
                    egui::FontId::proportional(24.0 * scale),
                    Color32::WHITE,
                )
                .size()
                .x
            });
            assert!(
                typed.width() >= text_width,
                "input must not scroll inside old width"
            );
            assert!(
                output
                    .shapes
                    .iter()
                    .filter(|s| matches!(s.shape, egui::Shape::LineSegment { .. }))
                    .count()
                    > 8,
                "editing box must have dashed edges"
            );
            let (larger, _) = text_input_frame(&ctx, origin, &mut text, 72.0 * scale, vec![]);
            assert_eq!(larger.min, origin);
            assert!(larger.width() > typed.width() * 2.0);
            assert!(larger.height() > typed.height() * 2.0);
        }
    }

    #[test]
    fn live_text_near_screen_edge_does_not_relocate_or_cap_box_width() {
        let ctx = egui::Context::default();
        let origin = egui::pos2(780.0, 580.0);
        let mut text = "Long annotation near screen edge".to_owned();
        for size in [24.0, 48.0, 96.0] {
            for _ in 0..2 {
                let (rect, _) = text_input_frame(&ctx, origin, &mut text, size, vec![]);
                assert_eq!(rect.min, origin, "screen bounds must not move text anchor");
                assert!(rect.width() > 80.0, "screen edge must not cap text box");
            }
        }
    }

    fn wait_until_idle(studio: &mut ImageStudio, ctx: &egui::Context, s: &Strings) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while (studio.saving || studio.loading) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let _ = studio.poll(ctx, s);
        }
        let _ = studio.poll(ctx, s);
    }

    #[test]
    fn undoing_crop_restores_saved_state() {
        let ctx = egui::Context::default();
        let mut studio = ImageStudio::default();
        studio.load_raw_bgra(
            Some("saved.png".into()),
            vec![120; 16 * 16 * 4],
            [16, 16],
            &ctx,
        );
        studio.crop_rect = Some(Region {
            x: 2,
            y: 2,
            width: 12,
            height: 12,
        });
        assert!(studio.apply_crop(&ctx));
        assert!(studio.is_modified());
        studio.undo(&ctx);
        assert!(!studio.is_modified());
        studio.redo(&ctx);
        assert!(studio.is_modified());
    }

    #[test]
    fn save_completion_does_not_mark_later_edits_clean() {
        let ctx = egui::Context::default();
        let mut studio = ImageStudio::default();
        let path = PathBuf::from("saved.png");
        studio.load_raw_bgra(Some(path.clone()), vec![120; 16 * 16 * 4], [16, 16], &ctx);
        studio.editor.place_number((4, 4));
        let signature = studio.state_signature();
        studio.editor.place_number((8, 8));
        studio.saving = true;
        studio
            .tx
            .send(StudioMsg::Saved {
                generation: studio.generation,
                signature,
                path,
                is_copy: false,
                result: Ok(()),
            })
            .unwrap();
        studio.poll(&ctx, &Strings::new(Lang::En));
        assert!(!studio.saving);
        assert!(studio.is_modified());
        studio.undo(&ctx);
        assert!(!studio.is_modified());
    }

    #[test]
    fn captured_image_in_editor_preserves_original_in_album_on_save_copy() {
        let dir =
            std::env::temp_dir().join(format!("xscreen-studio-savecopy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let ctx = egui::Context::default();
        let s = Strings::new(Lang::Tr);
        let mut studio = ImageStudio::default();

        // Pure blue 16x16 original capture
        let mut raw = vec![0u8; 16 * 16 * 4];
        for px in raw.as_chunks_mut::<4>().0 {
            *px = [255, 0, 0, 255]; // BGRA blue
        }

        studio.load_captured_bgra(
            raw,
            [16, 16],
            Vec::new(),
            1,
            Some(dir.clone()),
            Lang::Tr,
            &ctx,
        );
        wait_until_idle(&mut studio, &ctx, &s);

        let orig_path = studio.source_path.clone().expect("original capture saved");
        assert!(orig_path.exists(), "original capture must exist on disk");
        assert!(!studio.is_modified(), "unannotated capture starts clean");

        // Draw a filled red rectangle covering the image and click "Save copy"
        studio.editor.push(Shape::Rect {
            a: (0, 0),
            b: (15, 15),
            color: Rgba::rgb(255, 0, 0),
            width: 2.0,
            filled: true,
        });
        assert!(studio.is_modified());

        studio.save_image(true, Some(dir.clone()), Lang::Tr, &ctx);
        wait_until_idle(&mut studio, &ctx, &s);
        assert!(!studio.is_modified(), "a completed save clears the warning");

        let copy_path = studio.source_path.clone().expect("copy path set");
        assert_ne!(orig_path, copy_path, "Save copy must write a separate file");
        assert!(orig_path.exists(), "original capture must remain in Album");
        assert!(copy_path.exists(), "edited copy must also exist in Album");

        let png_count = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
            .count();
        assert_eq!(
            png_count, 2,
            "Album must contain both original and saved copy"
        );

        // Verify original pixels stayed blue while copy pixels are red
        let (orig_bgra, _) = album::decode_bgra(&orig_path).unwrap();
        let (copy_bgra, _) = album::decode_bgra(&copy_path).unwrap();
        assert_eq!(&orig_bgra[..4], &[255, 0, 0, 255], "original stayed blue");
        assert_eq!(&copy_bgra[..4], &[0, 0, 255, 255], "copy has red edit");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn immediate_save_copy_while_origin_saving_still_keeps_both_files() {
        let dir =
            std::env::temp_dir().join(format!("xscreen-studio-queued-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let ctx = egui::Context::default();
        let s = Strings::new(Lang::En);
        let mut studio = ImageStudio::default();
        let raw = vec![128u8; 12 * 12 * 4];

        studio.load_captured_bgra(
            raw,
            [12, 12],
            Vec::new(),
            1,
            Some(dir.clone()),
            Lang::En,
            &ctx,
        );
        // Trigger Save copy immediately BEFORE polling OriginSaved!
        studio.save_image(true, Some(dir.clone()), Lang::En, &ctx);
        wait_until_idle(&mut studio, &ctx, &s);

        let png_count = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
            .count();
        assert_eq!(
            png_count, 2,
            "both original and queued Save copy must exist"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
