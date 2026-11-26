use crate::{capture::Region, i18n::Strings};
use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use eframe::egui;
use std::sync::{Arc, OnceLock};

pub const HISTORY_LIMIT: usize = 50;
/// Kalem noktası seyreltme: kare mesafe eşiği (fiziksel piksel).
pub const PEN_MIN_DIST2: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlurKind {
    Soft,
    Mosaic,
    Heavy,
}

impl BlurKind {
    pub fn label_i18n(self, s: &Strings) -> &'static str {
        match self {
            Self::Soft => s.blur_soft(),
            Self::Mosaic => s.blur_mosaic(),
            Self::Heavy => s.blur_heavy(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Arrow,
    Line,
    Rect,
    Ellipse,
    Triangle,
    Pen,
    Marker,
    Text,
    Number,
    Blur,
}

impl Tool {
    pub const SHAPES: [Self; 3] = [Self::Rect, Self::Ellipse, Self::Triangle];

    pub fn is_shape(self) -> bool {
        matches!(self, Self::Rect | Self::Ellipse | Self::Triangle)
    }

    pub fn label_i18n(self, s: &Strings) -> &'static str {
        match self {
            Self::Select => s.tool_select(),
            Self::Arrow => s.tool_arrow(),
            Self::Line => s.tool_line(),
            Self::Rect => s.tool_rect(),
            Self::Ellipse => s.tool_ellipse(),
            Self::Triangle => s.tool_triangle(),
            Self::Pen => s.tool_pen(),
            Self::Marker => s.tool_marker(),
            Self::Text => s.tool_text(),
            Self::Number => s.tool_number(),
            Self::Blur => s.tool_blur(),
        }
    }

    pub fn is_drag(self) -> bool {
        matches!(
            self,
            Self::Arrow
                | Self::Rect
                | Self::Ellipse
                | Self::Triangle
                | Self::Line
                | Self::Pen
                | Self::Marker
                | Self::Blur
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeHandle {
    Start,
    End,
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Pick black or white ink that stays readable on this fill.
    pub fn contrast_text(self) -> Self {
        let channel = |v: u8| {
            let s = f32::from(v) / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        let luminance =
            0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b);
        if luminance > 0.3 {
            Self::rgb(0, 0, 0)
        } else {
            Self::rgb(255, 255, 255)
        }
    }
}

pub const PALETTE: [Rgba; 32] = [
    Rgba::rgb(235, 59, 46),
    Rgba::rgb(255, 140, 0),
    Rgba::rgb(250, 204, 21),
    Rgba::rgb(34, 197, 94),
    Rgba::rgb(14, 112, 192),
    Rgba::rgb(168, 85, 247),
    Rgba::rgb(255, 255, 255),
    Rgba::rgb(24, 24, 27),
    Rgba::rgb(255, 214, 10),
    Rgba::rgb(48, 209, 88),
    Rgba::rgb(0, 199, 190),
    Rgba::rgb(56, 189, 248),
    Rgba::rgb(99, 102, 241),
    Rgba::rgb(244, 63, 94),
    Rgba::rgb(236, 72, 153),
    Rgba::rgb(162, 132, 94),
    Rgba::rgb(142, 142, 147),
    Rgba::rgb(99, 99, 102),
    Rgba::rgb(64, 64, 64),
    Rgba::rgb(198, 198, 200),
    Rgba::rgb(0, 0, 0),
    Rgba::rgb(120, 120, 128),
    Rgba::rgb(174, 82, 255),
    Rgba::rgb(255, 159, 10),
    Rgba::rgb(100, 210, 255),
    Rgba::rgb(200, 255, 100),
    Rgba::rgb(255, 230, 160),
    Rgba::rgb(255, 170, 200),
    Rgba::rgb(110, 80, 60),
    Rgba::rgb(0, 90, 160),
    Rgba::rgb(0, 128, 96),
    Rgba::rgb(150, 40, 120),
];

pub const WIDTHS: [f32; 4] = [2.0, 4.0, 8.0, 14.0];

/// Lightshot-style marker: a wide, translucent yellow highlighter.
pub const MARKER_WIDTH: f32 = 20.0;
pub const MARKER_ALPHA: u8 = 128;
pub const MARKER_YELLOW: Rgba = Rgba::rgb(255, 255, 0);

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Arrow {
        a: (i32, i32),
        b: (i32, i32),
        color: Rgba,
        width: f32,
    },
    Line {
        a: (i32, i32),
        b: (i32, i32),
        color: Rgba,
        width: f32,
    },
    Rect {
        a: (i32, i32),
        b: (i32, i32),
        color: Rgba,
        width: f32,
        filled: bool,
    },
    Ellipse {
        a: (i32, i32),
        b: (i32, i32),
        color: Rgba,
        width: f32,
        filled: bool,
    },
    Triangle {
        a: (i32, i32),
        b: (i32, i32),
        color: Rgba,
        width: f32,
        filled: bool,
    },
    Pen {
        points: Vec<(i32, i32)>,
        color: Rgba,
        width: f32,
    },
    Marker {
        points: Vec<(i32, i32)>,
        color: Rgba,
        width: f32,
    },
    Text {
        at: (i32, i32),
        text: String,
        color: Rgba,
        size: f32,
    },
    Number {
        at: (i32, i32),
        n: u32,
        color: Rgba,
    },
    Blur {
        a: (i32, i32),
        b: (i32, i32),
        kind: BlurKind,
    },
}

impl Shape {
    pub fn kind_name(&self, s: &Strings) -> &'static str {
        match self {
            Self::Arrow { .. } => s.tool_arrow(),
            Self::Line { .. } => s.tool_line(),
            Self::Rect { .. } => s.tool_rect(),
            Self::Ellipse { .. } => s.tool_ellipse(),
            Self::Triangle { .. } => s.tool_triangle(),
            Self::Pen { .. } => s.tool_pen(),
            Self::Marker { .. } => s.tool_marker(),
            Self::Text { .. } => s.tool_text(),
            Self::Number { .. } => s.tool_number(),
            Self::Blur { .. } => s.tool_blur(),
        }
    }

    pub fn color(&self) -> Option<Rgba> {
        match self {
            Self::Arrow { color, .. }
            | Self::Line { color, .. }
            | Self::Rect { color, .. }
            | Self::Ellipse { color, .. }
            | Self::Triangle { color, .. }
            | Self::Pen { color, .. }
            | Self::Text { color, .. }
            | Self::Number { color, .. } => Some(*color),
            Self::Marker { color, .. } => Some(Rgba::rgb(color.r, color.g, color.b)),
            Self::Blur { .. } => None,
        }
    }

    pub fn set_color(&mut self, next: Rgba) {
        match self {
            Self::Arrow { color, .. }
            | Self::Line { color, .. }
            | Self::Rect { color, .. }
            | Self::Ellipse { color, .. }
            | Self::Triangle { color, .. }
            | Self::Pen { color, .. }
            | Self::Text { color, .. }
            | Self::Number { color, .. } => *color = Rgba::rgb(next.r, next.g, next.b),
            Self::Marker { color, .. } => {
                *color = Rgba {
                    r: next.r,
                    g: next.g,
                    b: next.b,
                    a: MARKER_ALPHA,
                };
            }
            Self::Blur { .. } => {}
        }
    }

    pub fn stroke_width(&self) -> Option<f32> {
        match self {
            Self::Arrow { width, .. }
            | Self::Line { width, .. }
            | Self::Rect { width, .. }
            | Self::Ellipse { width, .. }
            | Self::Triangle { width, .. }
            | Self::Pen { width, .. }
            | Self::Marker { width, .. } => Some(*width),
            Self::Text { size, .. } => Some((*size / 5.0).clamp(1.5, 24.0)),
            Self::Number { .. } | Self::Blur { .. } => None,
        }
    }

    pub fn set_stroke_width(&mut self, w: f32) {
        match self {
            Self::Arrow { width, .. }
            | Self::Line { width, .. }
            | Self::Rect { width, .. }
            | Self::Ellipse { width, .. }
            | Self::Triangle { width, .. }
            | Self::Pen { width, .. } => *width = w.clamp(1.0, 32.0),
            Self::Marker { width, .. } => *width = w.clamp(6.0, 48.0),
            Self::Text { size, .. } => *size = (w * 5.0).clamp(8.0, 320.0),
            Self::Number { .. } | Self::Blur { .. } => {}
        }
    }

    pub fn filled(&self) -> Option<bool> {
        match self {
            Self::Rect { filled, .. }
            | Self::Ellipse { filled, .. }
            | Self::Triangle { filled, .. } => Some(*filled),
            _ => None,
        }
    }

    pub fn set_filled(&mut self, next: bool) {
        if let Self::Rect { filled, .. }
        | Self::Ellipse { filled, .. }
        | Self::Triangle { filled, .. } = self
        {
            *filled = next;
        }
    }

    pub fn blur_kind(&self) -> Option<BlurKind> {
        match self {
            Self::Blur { kind, .. } => Some(*kind),
            _ => None,
        }
    }

    pub fn set_blur_kind(&mut self, next: BlurKind) {
        if let Self::Blur { kind, .. } = self {
            *kind = next;
        }
    }
}

pub fn norm_rect(a: (i32, i32), b: (i32, i32)) -> Region {
    let x = a.0.min(b.0);
    let y = a.1.min(b.1);
    Region {
        x,
        y,
        width: a.0.abs_diff(b.0),
        height: a.1.abs_diff(b.1),
    }
}

pub fn triangle_points(a: (i32, i32), b: (i32, i32)) -> [(i32, i32); 3] {
    let r = norm_rect(a, b);
    let top = (r.x + r.width as i32 / 2, r.y);
    let br = (r.x + r.width as i32, r.y + r.height as i32);
    let bl = (r.x, r.y + r.height as i32);
    [top, br, bl]
}

#[derive(Clone, Debug)]
struct Snapshot {
    shapes: Vec<Shape>,
    next_number: u32,
    selected: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Editor {
    pub tool: Tool,
    pub shape_tool: Tool,
    pub color: Rgba,
    pub width: f32,
    pub filled: bool,
    pub blur: BlurKind,
    pub shapes: Vec<Shape>,
    pub selected: Option<usize>,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    pub next_number: u32,
    pub draft: Option<Shape>,
    pub text: String,
    pub text_at: Option<(i32, i32)>,
    pub editing_text_index: Option<usize>,
    pub custom: [Option<Rgba>; 8],
    pub custom_open: bool,
    prop_coalesce_idx: Option<usize>,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            tool: Tool::Arrow,
            shape_tool: Tool::Rect,
            color: PALETTE[0],
            width: WIDTHS[1],
            filled: false,
            blur: BlurKind::Soft,
            shapes: Vec::new(),
            selected: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_number: 1,
            draft: None,
            text: String::new(),
            text_at: None,
            editing_text_index: None,
            custom: [None; 8],
            custom_open: false,
            prop_coalesce_idx: None,
        }
    }
}

impl Editor {
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn current_snapshot(&self) -> Snapshot {
        Snapshot {
            shapes: self.shapes.clone(),
            next_number: self.next_number,
            selected: self.selected,
        }
    }

    pub fn record_snapshot(&mut self) {
        self.prop_coalesce_idx = None;
        if self.undo_stack.len() >= HISTORY_LIMIT {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.current_snapshot());
        self.redo_stack.clear();
    }

    pub fn break_coalesce(&mut self) {
        self.prop_coalesce_idx = None;
    }

    fn record_coalesced_prop(&mut self, idx: usize) {
        if self.prop_coalesce_idx != Some(idx) {
            self.record_snapshot();
            self.prop_coalesce_idx = Some(idx);
        }
    }

    pub fn select_shape_index(&mut self, idx: Option<usize>) {
        self.selected = idx.filter(|&i| i < self.shapes.len());
        self.prop_coalesce_idx = None;
        if let Some(shape) = self.selected.and_then(|i| self.shapes.get(i)) {
            if let Some(c) = shape.color() {
                self.color = c;
            }
            if let Some(w) = shape.stroke_width() {
                self.width = w;
            }
            if let Some(f) = shape.filled() {
                self.filled = f;
            }
            if let Some(b) = shape.blur_kind() {
                self.blur = b;
            }
        }
    }

    pub fn select_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.prop_coalesce_idx = None;
        if tool != Tool::Select {
            self.selected = None;
        }
        if tool.is_shape() {
            self.shape_tool = tool;
        }
        if tool == Tool::Marker {
            self.color = MARKER_YELLOW;
        }
    }

    pub fn select_shape(&mut self, tool: Tool) {
        if tool.is_shape() {
            self.shape_tool = tool;
            self.tool = tool;
            self.selected = None;
            self.prop_coalesce_idx = None;
        }
    }

    pub fn select_blur(&mut self, kind: BlurKind) {
        self.blur = kind;
        if let Some(idx) = self.selected
            && self
                .shapes
                .get(idx)
                .is_some_and(|s| matches!(s, Shape::Blur { .. }))
        {
            self.record_coalesced_prop(idx);
            if let Some(s) = self.shapes.get_mut(idx) {
                s.set_blur_kind(kind);
            }
            return;
        }
        self.tool = Tool::Blur;
        self.selected = None;
    }

    pub fn apply_color(&mut self, color: Rgba) {
        self.color = color;
        if let Some(idx) = self.selected
            && idx < self.shapes.len()
        {
            self.record_coalesced_prop(idx);
            self.shapes[idx].set_color(color);
        }
    }

    pub fn apply_filled(&mut self, filled: bool) {
        self.filled = filled;
        if let Some(idx) = self.selected
            && idx < self.shapes.len()
            && self.shapes[idx].filled().is_some()
        {
            self.record_coalesced_prop(idx);
            self.shapes[idx].set_filled(filled);
        }
    }

    pub fn text_size(&self) -> f32 {
        (self.width * 5.0).clamp(8.0, 400.0)
    }

    pub fn begin_text(&mut self, at: (i32, i32)) {
        self.editing_text_index = None;
        self.text_at = Some(at);
        self.text.clear();
    }

    /// Re-open an existing `Shape::Text` in-place for editing.
    pub fn begin_edit_existing_text(&mut self, idx: usize) {
        if let Some(Shape::Text {
            at,
            text,
            color,
            size,
        }) = self.shapes.get(idx).cloned()
        {
            self.record_snapshot();
            self.color = color;
            self.width = (size / 5.0).clamp(1.6, 48.0);
            self.text = text;
            self.text_at = Some(at);
            self.editing_text_index = Some(idx);
            self.selected = None;
        }
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width;
        if let Some(idx) = self.selected
            && idx < self.shapes.len()
        {
            self.record_coalesced_prop(idx);
            self.shapes[idx].set_stroke_width(width);
        }
        if let Some(shape) = &mut self.draft {
            match shape {
                Shape::Arrow { width: w, .. }
                | Shape::Line { width: w, .. }
                | Shape::Rect { width: w, .. }
                | Shape::Ellipse { width: w, .. }
                | Shape::Triangle { width: w, .. }
                | Shape::Pen { width: w, .. } => *w = width,
                Shape::Marker { .. }
                | Shape::Text { .. }
                | Shape::Number { .. }
                | Shape::Blur { .. } => {}
            }
        }
    }

    pub fn delete_selected(&mut self) -> bool {
        let Some(idx) = self.selected else {
            return false;
        };
        if idx >= self.shapes.len() {
            self.selected = None;
            return false;
        }
        self.record_snapshot();
        let removed = self.shapes.remove(idx);
        self.selected = None;
        if matches!(removed, Shape::Number { .. }) {
            self.recompute_next_number();
        }
        true
    }

    pub fn duplicate_selected(&mut self) -> bool {
        let Some(idx) = self.selected else {
            return false;
        };
        let Some(mut copy) = self.shapes.get(idx).cloned() else {
            return false;
        };
        self.record_snapshot();
        translate_shape(&mut copy, 16, 16);
        if let Shape::Number { n, .. } = &mut copy {
            *n = self.next_number;
            self.next_number = self.next_number.saturating_add(1);
        }
        self.shapes.push(copy);
        self.selected = Some(self.shapes.len() - 1);
        true
    }

    pub fn bring_selected_to_front(&mut self) -> bool {
        let Some(idx) = self.selected else {
            return false;
        };
        if idx + 1 >= self.shapes.len() {
            return false;
        }
        self.record_snapshot();
        let item = self.shapes.remove(idx);
        self.shapes.push(item);
        self.selected = Some(self.shapes.len() - 1);
        true
    }

    pub fn clear_all(&mut self) {
        if self.shapes.is_empty() {
            return;
        }
        self.record_snapshot();
        self.shapes.clear();
        self.selected = None;
        self.next_number = 1;
    }

    fn recompute_next_number(&mut self) {
        self.next_number = self
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Number { n, .. } => Some(*n),
                _ => None,
            })
            .max()
            .map(|n| n + 1)
            .unwrap_or(1);
    }

    pub fn undo(&mut self) {
        self.prop_coalesce_idx = None;
        if let Some(prev) = self.undo_stack.pop() {
            let cur = self.current_snapshot();
            self.redo_stack.push(cur);
            self.shapes = prev.shapes;
            self.next_number = prev.next_number;
            self.selected = prev.selected.filter(|&i| i < self.shapes.len());
        }
    }

    pub fn redo(&mut self) {
        self.prop_coalesce_idx = None;
        if let Some(next) = self.redo_stack.pop() {
            let cur = self.current_snapshot();
            if self.undo_stack.len() >= HISTORY_LIMIT {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(cur);
            self.shapes = next.shapes;
            self.next_number = next.next_number;
            self.selected = next.selected.filter(|&i| i < self.shapes.len());
        }
    }

    pub fn push(&mut self, shape: Shape) {
        self.record_snapshot();
        if self.shapes.len() >= HISTORY_LIMIT {
            self.shapes.remove(0);
        }
        self.shapes.push(shape);
    }

    pub fn cancel_draft(&mut self) {
        self.draft = None;
    }

    pub fn begin(&mut self, at: (i32, i32)) {
        if !self.tool.is_drag() {
            return;
        }
        let color = self.color;
        let width = self.width;
        let filled = self.filled;
        self.draft = Some(match self.tool {
            Tool::Arrow => Shape::Arrow {
                a: at,
                b: at,
                color,
                width,
            },
            Tool::Line => Shape::Line {
                a: at,
                b: at,
                color,
                width,
            },
            Tool::Rect => Shape::Rect {
                a: at,
                b: at,
                color,
                width,
                filled,
            },
            Tool::Ellipse => Shape::Ellipse {
                a: at,
                b: at,
                color,
                width,
                filled,
            },
            Tool::Triangle => Shape::Triangle {
                a: at,
                b: at,
                color,
                width,
                filled,
            },
            Tool::Pen => Shape::Pen {
                points: vec![at],
                color,
                width,
            },
            Tool::Marker => Shape::Marker {
                points: vec![at],
                color: Rgba {
                    a: MARKER_ALPHA,
                    ..color
                },
                width: MARKER_WIDTH,
            },
            Tool::Blur => Shape::Blur {
                a: at,
                b: at,
                kind: self.blur,
            },
            Tool::Select | Tool::Text | Tool::Number => unreachable!(),
        });
    }

    pub fn extend(&mut self, at: (i32, i32)) {
        match &mut self.draft {
            Some(
                Shape::Arrow { b, .. }
                | Shape::Line { b, .. }
                | Shape::Rect { b, .. }
                | Shape::Ellipse { b, .. }
                | Shape::Triangle { b, .. }
                | Shape::Blur { b, .. },
            ) => *b = at,
            Some(Shape::Pen { points, .. } | Shape::Marker { points, .. }) => {
                if let Some(last) = points.last().copied() {
                    let dx = at.0 - last.0;
                    let dy = at.1 - last.1;
                    if dx * dx + dy * dy >= PEN_MIN_DIST2 {
                        points.push(at);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn commit(&mut self) -> bool {
        let Some(shape) = self.draft.take() else {
            return false;
        };
        let keep = match &shape {
            Shape::Arrow { a, b, .. } | Shape::Line { a, b, .. } => a != b,
            Shape::Rect { a, b, .. }
            | Shape::Ellipse { a, b, .. }
            | Shape::Triangle { a, b, .. }
            | Shape::Blur { a, b, .. } => {
                let r = norm_rect(*a, *b);
                r.width >= 2 && r.height >= 2
            }
            Shape::Pen { points, .. } | Shape::Marker { points, .. } => points.len() >= 2,
            _ => true,
        };
        if keep {
            self.push(shape);
        }
        keep
    }

    pub fn place_number(&mut self, at: (i32, i32)) {
        self.record_snapshot();
        let n = self.next_number;
        self.next_number = self.next_number.saturating_add(1);
        if self.shapes.len() >= HISTORY_LIMIT {
            self.shapes.remove(0);
        }
        self.shapes.push(Shape::Number {
            at,
            n,
            color: self.color,
        });
    }

    pub fn commit_text(&mut self) {
        let Some(at) = self.text_at.take() else {
            return;
        };
        let editing_idx = self.editing_text_index.take();
        let text = std::mem::take(&mut self.text);
        let text = text.trim().to_owned();
        if let Some(idx) = editing_idx {
            if idx < self.shapes.len() {
                if text.is_empty() {
                    self.shapes.remove(idx);
                    self.selected = None;
                } else {
                    self.shapes[idx] = Shape::Text {
                        at,
                        text,
                        color: self.color,
                        size: self.text_size(),
                    };
                    self.selected = Some(idx);
                }
            }
            return;
        }
        if text.is_empty() {
            return;
        }
        self.push(Shape::Text {
            at,
            text,
            color: self.color,
            size: self.text_size(),
        });
    }
}

/// Lay out annotations with the viewport's actual font and scale, not export metrics.
#[derive(Clone, Copy)]
pub struct TextLayout<'a> {
    pub ctx: &'a egui::Context,
    pub px_to_ui: f32,
}

impl TextLayout<'_> {
    pub fn galley(self, text: &str, size: f32) -> Arc<egui::Galley> {
        self.ctx.fonts_mut(|fonts| {
            fonts.layout_no_wrap(
                text.to_owned(),
                egui::FontId::proportional((size * self.px_to_ui).max(1.0)),
                egui::Color32::PLACEHOLDER,
            )
        })
    }

    /// The same layout used for painting, converted back to canvas pixels.
    pub fn bounds(self, shape: &Shape) -> Option<Region> {
        let Shape::Text { at, text, size, .. } = shape else {
            return None;
        };
        let galley = self.galley(text, *size);
        let rect = galley.rect.union(galley.mesh_bounds);
        let pad = 4;
        let left = (rect.min.x / self.px_to_ui).floor() as i32 - pad;
        let top = (rect.min.y / self.px_to_ui).floor() as i32 - pad;
        let right = (rect.max.x / self.px_to_ui).ceil() as i32 + pad;
        let bottom = (rect.max.y / self.px_to_ui).ceil() as i32 + pad;
        Some(Region {
            x: at.0 + left,
            y: at.1 + top,
            width: (right - left).max(12) as u32,
            height: (bottom - top).max(12) as u32,
        })
    }
}

pub fn hit_text(shapes: &[Shape], p: (i32, i32), text_layout: TextLayout<'_>) -> Option<usize> {
    shapes.iter().enumerate().rev().find_map(|(index, shape)| {
        let b = text_layout.bounds(shape)?;
        (p.0 >= b.x && p.0 < b.x + b.width as i32 && p.1 >= b.y && p.1 < b.y + b.height as i32)
            .then_some(index)
    })
}

/// Compute tight bounding box of any shape in pixel coordinates.
pub fn shape_bounds(shape: &Shape, text_layout: TextLayout<'_>) -> Option<Region> {
    match shape {
        Shape::Arrow { a, b, .. }
        | Shape::Line { a, b, .. }
        | Shape::Rect { a, b, .. }
        | Shape::Ellipse { a, b, .. }
        | Shape::Triangle { a, b, .. }
        | Shape::Blur { a, b, .. } => Some(norm_rect(*a, *b)),
        Shape::Pen { points, .. } | Shape::Marker { points, .. } => {
            let first = *points.first()?;
            let (mut min_x, mut min_y) = first;
            let (mut max_x, mut max_y) = first;
            for &(x, y) in points {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
            Some(Region {
                x: min_x,
                y: min_y,
                width: (max_x - min_x).max(4) as u32,
                height: (max_y - min_y).max(4) as u32,
            })
        }
        Shape::Text { .. } => text_layout.bounds(shape),
        Shape::Number { at, .. } => Some(Region {
            x: at.0 - 13,
            y: at.1 - 13,
            width: 26,
            height: 26,
        }),
    }
}

fn rect_eight_handles(r: Region) -> Vec<(ShapeHandle, (i32, i32))> {
    let l = r.x;
    let t = r.y;
    let rr = r.x + r.width as i32;
    let b = r.y + r.height as i32;
    let mx = r.x + r.width as i32 / 2;
    let my = r.y + r.height as i32 / 2;
    vec![
        (ShapeHandle::TopLeft, (l, t)),
        (ShapeHandle::Top, (mx, t)),
        (ShapeHandle::TopRight, (rr, t)),
        (ShapeHandle::Right, (rr, my)),
        (ShapeHandle::BottomRight, (rr, b)),
        (ShapeHandle::Bottom, (mx, b)),
        (ShapeHandle::BottomLeft, (l, b)),
        (ShapeHandle::Left, (l, my)),
    ]
}

/// Interactive handles for a selected shape.
pub fn shape_handles(shape: &Shape, text_layout: TextLayout<'_>) -> Vec<(ShapeHandle, (i32, i32))> {
    match shape {
        Shape::Arrow { a, b, .. } | Shape::Line { a, b, .. } => {
            vec![(ShapeHandle::Start, *a), (ShapeHandle::End, *b)]
        }
        Shape::Rect { .. }
        | Shape::Ellipse { .. }
        | Shape::Triangle { .. }
        | Shape::Blur { .. }
        | Shape::Pen { .. }
        | Shape::Marker { .. } => shape_bounds(shape, text_layout)
            .map(rect_eight_handles)
            .unwrap_or_default(),
        Shape::Text { .. } => {
            let Some(r) = text_layout.bounds(shape) else {
                return Vec::new();
            };
            let l = r.x;
            let t = r.y;
            let rr = r.x + r.width as i32;
            let b = r.y + r.height as i32;
            vec![
                (ShapeHandle::TopLeft, (l, t)),
                (ShapeHandle::TopRight, (rr, t)),
                (ShapeHandle::BottomRight, (rr, b)),
                (ShapeHandle::BottomLeft, (l, b)),
            ]
        }
        Shape::Number { .. } => Vec::new(),
    }
}

pub fn hit_shape_handle(
    shape: &Shape,
    p: (i32, i32),
    radius: i32,
    text_layout: TextLayout<'_>,
) -> Option<ShapeHandle> {
    let r2 = (radius * radius).max(16);
    shape_handles(shape, text_layout)
        .into_iter()
        .find(|(_, (hx, hy))| {
            let dx = p.0 - *hx;
            let dy = p.1 - *hy;
            dx * dx + dy * dy <= r2
        })
        .map(|(h, _)| h)
}

fn dist_to_segment(p: (i32, i32), a: (i32, i32), b: (i32, i32)) -> f32 {
    let px = p.0 as f32;
    let py = p.1 as f32;
    let ax = a.0 as f32;
    let ay = a.1 as f32;
    let bx = b.0 as f32;
    let by = b.1 as f32;
    let vx = bx - ax;
    let vy = by - ay;
    let len2 = vx * vx + vy * vy;
    if len2 <= 1e-4 {
        return (px - ax).hypot(py - ay);
    }
    let t = (((px - ax) * vx + (py - ay) * vy) / len2).clamp(0.0, 1.0);
    let proj_x = ax + t * vx;
    let proj_y = ay + t * vy;
    (px - proj_x).hypot(py - proj_y)
}

fn point_in_triangle(p: (i32, i32), a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> bool {
    let edge = |p0: (i32, i32), p1: (i32, i32), q: (i32, i32)| -> i64 {
        i64::from(q.0 - p0.0) * i64::from(p1.1 - p0.1)
            - i64::from(q.1 - p0.1) * i64::from(p1.0 - p0.0)
    };
    let w0 = edge(b, c, p);
    let w1 = edge(c, a, p);
    let w2 = edge(a, b, p);
    (w0 >= 0 && w1 >= 0 && w2 >= 0) || (w0 <= 0 && w1 <= 0 && w2 <= 0)
}

/// Hit-test shapes from top to bottom. `tolerance` is in image/canvas pixels.
pub fn hit_shape(
    shapes: &[Shape],
    p: (i32, i32),
    tolerance: f32,
    text_layout: TextLayout<'_>,
) -> Option<usize> {
    let tol = tolerance.max(4.0);
    shapes.iter().enumerate().rev().find_map(|(idx, shape)| {
        let hit = match shape {
            Shape::Line { a, b, width, .. } => dist_to_segment(p, *a, *b) <= *width * 0.5 + tol,
            Shape::Arrow { a, b, width, .. } => {
                let on_shaft = dist_to_segment(p, *a, *b) <= *width * 0.5 + tol;
                let on_head = arrow_head(*a, *b, (*width * 4.0).max(10.0))
                    .is_some_and(|h| point_in_triangle(p, *b, h[0], h[1]));
                on_shaft || on_head
            }
            Shape::Rect {
                a,
                b,
                width,
                filled,
                ..
            } => {
                let r = norm_rect(*a, *b);
                let l = r.x;
                let t = r.y;
                let rr = r.x + r.width as i32;
                let bb = r.y + r.height as i32;
                if *filled {
                    p.0 >= l && p.0 <= rr && p.1 >= t && p.1 <= bb
                } else {
                    let band = *width * 0.5 + tol;
                    dist_to_segment(p, (l, t), (rr, t)) <= band
                        || dist_to_segment(p, (rr, t), (rr, bb)) <= band
                        || dist_to_segment(p, (rr, bb), (l, bb)) <= band
                        || dist_to_segment(p, (l, bb), (l, t)) <= band
                }
            }
            Shape::Ellipse {
                a,
                b,
                width,
                filled,
                ..
            } => {
                let r = norm_rect(*a, *b);
                let rx = (r.width as f32 * 0.5).max(1.0);
                let ry = (r.height as f32 * 0.5).max(1.0);
                let cx = r.x as f32 + rx;
                let cy = r.y as f32 + ry;
                let nx = (p.0 as f32 - cx) / rx;
                let ny = (p.1 as f32 - cy) / ry;
                let dist_norm = (nx * nx + ny * ny).sqrt();
                if *filled {
                    dist_norm <= 1.0 + tol / rx.min(ry)
                } else {
                    (dist_norm - 1.0).abs() * rx.min(ry) <= *width * 0.5 + tol
                }
            }
            Shape::Triangle {
                a,
                b,
                width,
                filled,
                ..
            } => {
                let pts = triangle_points(*a, *b);
                let band = *width * 0.5 + tol;
                let on_edge = dist_to_segment(p, pts[0], pts[1]) <= band
                    || dist_to_segment(p, pts[1], pts[2]) <= band
                    || dist_to_segment(p, pts[2], pts[0]) <= band;
                on_edge || (*filled && point_in_triangle(p, pts[0], pts[1], pts[2]))
            }
            Shape::Pen { points, width, .. } | Shape::Marker { points, width, .. } => {
                let band = *width * 0.5 + tol;
                if points.len() == 1 {
                    dist_to_segment(p, points[0], points[0]) <= band
                } else {
                    points
                        .windows(2)
                        .any(|pair| dist_to_segment(p, pair[0], pair[1]) <= band)
                }
            }
            Shape::Text { .. } => text_layout.bounds(shape).is_some_and(|b| {
                let pad = tol as i32;
                p.0 >= b.x - pad
                    && p.0 <= b.x + b.width as i32 + pad
                    && p.1 >= b.y - pad
                    && p.1 <= b.y + b.height as i32 + pad
            }),
            Shape::Number { at, .. } => {
                let dx = (p.0 - at.0) as f32;
                let dy = (p.1 - at.1) as f32;
                dx.hypot(dy) <= 13.0 + tol
            }
            Shape::Blur { a, b, .. } => {
                let r = norm_rect(*a, *b);
                p.0 >= r.x
                    && p.0 <= r.x + r.width as i32
                    && p.1 >= r.y
                    && p.1 <= r.y + r.height as i32
            }
        };
        hit.then_some(idx)
    })
}

pub fn translate_shape(shape: &mut Shape, dx: i32, dy: i32) {
    match shape {
        Shape::Arrow { a, b, .. }
        | Shape::Line { a, b, .. }
        | Shape::Rect { a, b, .. }
        | Shape::Ellipse { a, b, .. }
        | Shape::Triangle { a, b, .. }
        | Shape::Blur { a, b, .. } => {
            a.0 += dx;
            a.1 += dy;
            b.0 += dx;
            b.1 += dy;
        }
        Shape::Pen { points, .. } | Shape::Marker { points, .. } => {
            for pt in points {
                pt.0 += dx;
                pt.1 += dy;
            }
        }
        Shape::Text { at, .. } | Shape::Number { at, .. } => {
            at.0 += dx;
            at.1 += dy;
        }
    }
}

fn resize_region_by_handle(
    r: Region,
    handle: ShapeHandle,
    dx: i32,
    dy: i32,
) -> ((i32, i32), (i32, i32)) {
    let mut l = r.x;
    let mut rr = r.x + r.width as i32;
    let mut t = r.y;
    let mut b = r.y + r.height as i32;
    match handle {
        ShapeHandle::TopLeft => {
            l += dx;
            t += dy;
        }
        ShapeHandle::Top => t += dy,
        ShapeHandle::TopRight => {
            rr += dx;
            t += dy;
        }
        ShapeHandle::Right => rr += dx,
        ShapeHandle::BottomRight => {
            rr += dx;
            b += dy;
        }
        ShapeHandle::Bottom => b += dy,
        ShapeHandle::BottomLeft => {
            l += dx;
            b += dy;
        }
        ShapeHandle::Left => l += dx,
        ShapeHandle::Start | ShapeHandle::End => {}
    }
    let min_x = l.min(rr);
    let max_x = l.max(rr).max(min_x + 4);
    let min_y = t.min(b);
    let max_y = t.max(b).max(min_y + 4);
    ((min_x, min_y), (max_x, max_y))
}

/// Non-destructively derive a resized shape from `origin` and pointer delta.
pub fn resize_shape_from_origin(
    origin: &Shape,
    handle: ShapeHandle,
    start_mouse: (i32, i32),
    current_mouse: (i32, i32),
    text_layout: TextLayout<'_>,
) -> Shape {
    let dx = current_mouse.0 - start_mouse.0;
    let dy = current_mouse.1 - start_mouse.1;
    let mut out = origin.clone();
    match &mut out {
        Shape::Arrow { a, b, .. } | Shape::Line { a, b, .. } => match handle {
            ShapeHandle::Start => {
                a.0 += dx;
                a.1 += dy;
            }
            ShapeHandle::End => {
                b.0 += dx;
                b.1 += dy;
            }
            _ => {}
        },
        Shape::Rect { a, b, .. }
        | Shape::Ellipse { a, b, .. }
        | Shape::Triangle { a, b, .. }
        | Shape::Blur { a, b, .. } => {
            let r = norm_rect(*a, *b);
            let (na, nb) = resize_region_by_handle(r, handle, dx, dy);
            *a = na;
            *b = nb;
        }
        Shape::Pen { points, .. } | Shape::Marker { points, .. } => {
            if let Some(orig_r) = shape_bounds(origin, text_layout) {
                let (na, nb) = resize_region_by_handle(orig_r, handle, dx, dy);
                let ow = (orig_r.width as f32).max(1.0);
                let oh = (orig_r.height as f32).max(1.0);
                let nw = (nb.0 - na.0) as f32;
                let nh = (nb.1 - na.1) as f32;
                for pt in points {
                    let tx = (pt.0 - orig_r.x) as f32 / ow;
                    let ty = (pt.1 - orig_r.y) as f32 / oh;
                    pt.0 = (na.0 as f32 + tx * nw).round() as i32;
                    pt.1 = (na.1 as f32 + ty * nh).round() as i32;
                }
            }
        }
        Shape::Text { at, size, .. } => {
            if let Some(orig_r) = text_layout.bounds(origin) {
                let (na, nb) = resize_region_by_handle(orig_r, handle, dx, dy);
                let oh = (orig_r.height as f32).max(8.0);
                let nh = (nb.1 - na.1).max(10) as f32;
                let ratio = (nh / oh).clamp(0.25, 6.0);
                *size = (*size * ratio).clamp(8.0, 320.0);
                if matches!(handle, ShapeHandle::TopLeft | ShapeHandle::BottomLeft) {
                    at.0 += na.0 - orig_r.x;
                }
                if matches!(handle, ShapeHandle::TopLeft | ShapeHandle::TopRight) {
                    at.1 += na.1 - orig_r.y;
                }
            }
        }
        Shape::Number { .. } => {}
    }
    out
}

/// Crop top-down BGRA buffer to `crop` (clamped to `[0..width, 0..height]`).
pub fn crop_bgra(
    bgra: &[u8],
    width: u32,
    height: u32,
    crop: Region,
) -> Option<(Vec<u8>, [u32; 2])> {
    let x0 = crop.x.max(0) as u32;
    let y0 = crop.y.max(0) as u32;
    let x1 = (crop.x.saturating_add(crop.width as i32).max(0) as u32).min(width);
    let y1 = (crop.y.saturating_add(crop.height as i32).max(0) as u32).min(height);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let cw = (x1 - x0) as usize;
    let ch = (y1 - y0) as usize;
    let stride = width as usize * 4;
    let mut out = vec![0u8; cw * ch * 4];
    for row in 0..ch {
        let src = (y0 as usize + row) * stride + x0 as usize * 4;
        let dst = row * cw * 4;
        out[dst..dst + cw * 4].copy_from_slice(&bgra[src..src + cw * 4]);
    }
    Some((out, [cw as u32, ch as u32]))
}

pub fn toolbar_anchor(
    sel_local: (f32, f32, f32, f32),
    screen: (f32, f32),
    bar_h: f32,
    gap: f32,
) -> ((f32, f32), bool) {
    let (x, y, w, h) = sel_local;
    let sh = screen.1;
    let right = x + w;
    let below = y + h + gap;
    let above = y - gap;
    if below + bar_h <= sh {
        ((right, below), false)
    } else if above - bar_h >= 0.0 {
        ((right, above), true)
    } else {
        ((right, (sh - 4.0).max(0.0)), true)
    }
}

pub fn arrow_head(a: (i32, i32), b: (i32, i32), length: f32) -> Option<[(i32, i32); 2]> {
    let dx = (b.0 - a.0) as f32;
    let dy = (b.1 - a.1) as f32;
    let len = dx.hypot(dy);
    if len < 1.0 {
        return None;
    }
    let ux = dx / len;
    let uy = dy / len;
    let px = -uy;
    let py = ux;
    let spread = length * 0.45;
    let base_x = b.0 as f32 - ux * length;
    let base_y = b.1 as f32 - uy * length;
    Some([
        (
            (base_x + px * spread).round() as i32,
            (base_y + py * spread).round() as i32,
        ),
        (
            (base_x - px * spread).round() as i32,
            (base_y - py * spread).round() as i32,
        ),
    ])
}

pub fn box_blur(bgra: &mut [u8], width: usize, height: usize, radius: usize) {
    if width == 0 || height == 0 || bgra.len() < width * height * 4 || radius == 0 {
        return;
    }
    let src = bgra.to_vec();
    let mut tmp = vec![0u8; src.len()];
    let mut sums = [0u32; 4];

    for y in 0..height {
        let row = y * width * 4;
        let first_right = radius.min(width - 1);
        for (c, sum) in sums.iter_mut().enumerate() {
            *sum = (0..=first_right)
                .map(|x| u32::from(src[row + x * 4 + c]))
                .sum();
        }
        for x in 0..width {
            let left = x.saturating_sub(radius);
            let right = (x + radius + 1).min(width);
            let n = (right - left) as u32;
            for (c, sum) in sums.iter().enumerate() {
                tmp[row + x * 4 + c] = (sum / n) as u8;
            }
            let add = x + radius + 1;
            if add < width {
                for (c, sum) in sums.iter_mut().enumerate() {
                    *sum += u32::from(src[row + add * 4 + c]);
                }
            }
            let remove = x as isize - radius as isize;
            if remove >= 0 {
                let remove = remove as usize;
                for (c, sum) in sums.iter_mut().enumerate() {
                    *sum -= u32::from(src[row + remove * 4 + c]);
                }
            }
        }
    }

    for x in 0..width {
        let first_bottom = radius.min(height - 1);
        for (c, sum) in sums.iter_mut().enumerate() {
            *sum = (0..=first_bottom)
                .map(|y| u32::from(tmp[(y * width + x) * 4 + c]))
                .sum();
        }
        for y in 0..height {
            let top = y.saturating_sub(radius);
            let bottom = (y + radius + 1).min(height);
            let n = (bottom - top) as u32;
            let dst = (y * width + x) * 4;
            for (c, sum) in sums.iter().enumerate() {
                bgra[dst + c] = (sum / n) as u8;
            }
            let add = y + radius + 1;
            if add < height {
                for (c, sum) in sums.iter_mut().enumerate() {
                    *sum += u32::from(tmp[(add * width + x) * 4 + c]);
                }
            }
            let remove = y as isize - radius as isize;
            if remove >= 0 {
                let remove = remove as usize;
                for (c, sum) in sums.iter_mut().enumerate() {
                    *sum -= u32::from(tmp[(remove * width + x) * 4 + c]);
                }
            }
        }
    }
}

const BLUR_RADIUS: usize = 4;
const HEAVY_BLUR_RADIUS: usize = 10;
const MOSAIC_BLOCK: i32 = 12;

pub fn bake(bgra: &mut [u8], sel: Region, shapes: &[Shape]) {
    let _ = apply_shapes(
        bgra,
        sel.width as i32,
        sel.height as i32,
        (sel.x, sel.y),
        1,
        shapes,
        false,
    );
}

pub struct BlurPatch {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

#[cfg(test)]
pub fn blur_patches(
    source: &[u8],
    width: i32,
    height: i32,
    origin: (i32, i32),
    scale: i32,
    shapes: &[Shape],
) -> Vec<BlurPatch> {
    if width <= 0 || height <= 0 || source.len() < width as usize * height as usize * 4 {
        return Vec::new();
    }
    let mut bgra = source.to_vec();
    apply_shapes(&mut bgra, width, height, origin, scale, shapes, true)
}

pub fn apply_shapes_at_1x(
    bgra: &mut [u8],
    width: i32,
    height: i32,
    origin: (i32, i32),
    shapes: &[Shape],
) -> Vec<BlurPatch> {
    apply_shapes(bgra, width, height, origin, 1, shapes, true)
}

pub fn blur_patch_at_1x(
    canvas_bgra: &[u8],
    width: i32,
    height: i32,
    origin: (i32, i32),
    a: (i32, i32),
    b: (i32, i32),
    kind: BlurKind,
) -> Option<BlurPatch> {
    if width <= 0 || height <= 0 || canvas_bgra.len() < width as usize * height as usize * 4 {
        return None;
    }
    let r = mapped_blur_rect(a, b, origin, 1);
    let x0 = r.x.max(0);
    let y0 = r.y.max(0);
    let x1 = (r.x + r.width as i32).min(width);
    let y1 = (r.y + r.height as i32).min(height);
    if x1 - x0 < 2 || y1 - y0 < 2 {
        return None;
    }
    let pad = HEAVY_BLUR_RADIUS as i32;
    let wx0 = ((x0 - pad).max(0) / MOSAIC_BLOCK) * MOSAIC_BLOCK;
    let wy0 = ((y0 - pad).max(0) / MOSAIC_BLOCK) * MOSAIC_BLOCK;
    let wx1 = (x1 + pad).min(width);
    let wy1 = (y1 + pad).min(height);
    let ww = wx1 - wx0;
    let wh = wy1 - wy0;
    let stride = width as usize * 4;
    let w_stride = ww as usize * 4;
    let mut window = vec![0u8; wh as usize * w_stride];
    for row in 0..wh as usize {
        let src = (wy0 as usize + row) * stride + wx0 as usize * 4;
        window[row * w_stride..(row + 1) * w_stride]
            .copy_from_slice(&canvas_bgra[src..src + w_stride]);
    }
    let local_r = Region {
        x: x0 - wx0,
        y: y0 - wy0,
        width: (x1 - x0) as u32,
        height: (y1 - y0) as u32,
    };
    apply_blur(&mut window, ww, wh, local_r, kind, 1);
    Some(copy_patch(&window, ww, wh, local_r))
}

fn apply_shapes(
    bgra: &mut [u8],
    width: i32,
    height: i32,
    origin: (i32, i32),
    scale: i32,
    shapes: &[Shape],
    capture_blurs: bool,
) -> Vec<BlurPatch> {
    if width <= 0 || height <= 0 || bgra.len() < width as usize * height as usize * 4 {
        return Vec::new();
    }
    let scale = scale.max(1);
    let map = |p: (i32, i32)| {
        (
            (p.0 - origin.0).div_euclid(scale),
            (p.1 - origin.1).div_euclid(scale),
        )
    };
    let thick = |w: f32| {
        if scale == 1 {
            w
        } else {
            (w / scale as f32).max(1.0)
        }
    };
    let font = ui_font();
    let mut patches = Vec::new();
    for shape in shapes {
        match shape {
            Shape::Line {
                a,
                b,
                color,
                width: w,
            } => {
                stroke_line(bgra, width, height, map(*a), map(*b), *color, thick(*w));
            }
            Shape::Arrow {
                a,
                b,
                color,
                width: w,
            } => {
                let a = map(*a);
                let b = map(*b);
                let stroke = thick(*w);
                stroke_line(bgra, width, height, a, b, *color, stroke);
                let head = if scale == 1 {
                    (*w * 4.0).max(10.0)
                } else {
                    (stroke * 4.0).max(2.0)
                };
                if let Some(head) = arrow_head(a, b, head) {
                    fill_triangle(bgra, width, height, b, head[0], head[1], *color);
                }
            }
            Shape::Rect {
                a,
                b,
                color,
                width: w,
                filled,
            } => {
                let r = norm_rect(map(*a), map(*b));
                if *filled {
                    fill_rect(bgra, width, height, r, *color);
                } else {
                    stroke_rect(bgra, width, height, r, *color, thick(*w));
                }
            }
            Shape::Ellipse {
                a,
                b,
                color,
                width: w,
                filled,
            } => {
                let r = norm_rect(map(*a), map(*b));
                if *filled {
                    fill_ellipse(bgra, width, height, r, *color);
                } else {
                    stroke_ellipse(bgra, width, height, r, *color, thick(*w));
                }
            }
            Shape::Triangle {
                a,
                b,
                color,
                width: w,
                filled,
            } => {
                let pts = triangle_points(map(*a), map(*b));
                if *filled {
                    fill_triangle(bgra, width, height, pts[0], pts[1], pts[2], *color);
                } else {
                    let stroke = thick(*w);
                    stroke_line(bgra, width, height, pts[0], pts[1], *color, stroke);
                    stroke_line(bgra, width, height, pts[1], pts[2], *color, stroke);
                    stroke_line(bgra, width, height, pts[2], pts[0], *color, stroke);
                }
            }
            Shape::Pen {
                points,
                color,
                width: w,
            } => {
                let pts: Vec<_> = points.iter().copied().map(map).collect();
                let stroke = thick(*w);
                for pair in pts.windows(2) {
                    stroke_line(bgra, width, height, pair[0], pair[1], *color, stroke);
                }
            }
            Shape::Marker {
                points,
                color,
                width: w,
            } => {
                let pts: Vec<_> = points.iter().copied().map(map).collect();
                stroke_marker(bgra, width, height, &pts, *color, thick(*w));
            }
            Shape::Text {
                at,
                text,
                color,
                size,
            } => {
                if let Some(font) = font {
                    let size = if scale == 1 {
                        *size
                    } else {
                        (*size / scale as f32).max(6.0)
                    };
                    bake_text(bgra, width, height, map(*at), text, *color, size, font);
                }
            }
            Shape::Number { at, n, color } => {
                let c = map(*at);
                let radius = if scale == 1 { 12 } else { (12 / scale).max(2) };
                fill_circle(bgra, width, height, c, radius, *color);
                if scale == 1 {
                    stamp_digits(bgra, width, height, c, *n, color.contrast_text());
                }
            }
            Shape::Blur { a, b, kind } => {
                let r = mapped_blur_rect(*a, *b, origin, scale);
                apply_blur(bgra, width, height, r, *kind, scale);
                if capture_blurs {
                    patches.push(copy_patch(bgra, width, height, r));
                }
            }
        }
    }
    patches
}

fn mapped_blur_rect(a: (i32, i32), b: (i32, i32), origin: (i32, i32), scale: i32) -> Region {
    let r = norm_rect(a, b);
    if scale <= 1 {
        return Region {
            x: r.x - origin.0,
            y: r.y - origin.1,
            width: r.width,
            height: r.height,
        };
    }
    let f = scale as f32;
    let left = ((r.x - origin.0) as f32 / f).floor() as i32;
    let top = ((r.y - origin.1) as f32 / f).floor() as i32;
    let right = ((r.x + r.width as i32 - origin.0) as f32 / f).ceil() as i32;
    let bottom = ((r.y + r.height as i32 - origin.1) as f32 / f).ceil() as i32;
    if right <= left || bottom <= top {
        return Region {
            x: left,
            y: top,
            width: 0,
            height: 0,
        };
    }
    Region {
        x: left,
        y: top,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    }
}

fn apply_blur(bgra: &mut [u8], width: i32, height: i32, r: Region, kind: BlurKind, scale: i32) {
    if scale <= 1 {
        match kind {
            BlurKind::Soft => blur_region(bgra, width, height, r, BLUR_RADIUS),
            BlurKind::Heavy => blur_region(bgra, width, height, r, HEAVY_BLUR_RADIUS),
            BlurKind::Mosaic => pixelate_region(bgra, width, height, r, MOSAIC_BLOCK),
        }
    } else {
        match kind {
            BlurKind::Soft => blur_region(bgra, width, height, r, 1),
            BlurKind::Heavy => {
                blur_region(bgra, width, height, r, 1);
                blur_region(bgra, width, height, r, 1);
            }
            BlurKind::Mosaic => pixelate_region(bgra, width, height, r, 3),
        }
    }
}

fn copy_patch(bgra: &[u8], width: i32, height: i32, r: Region) -> BlurPatch {
    let x0 = r.x.max(0);
    let y0 = r.y.max(0);
    let x1 = (r.x + r.width as i32).min(width).max(x0);
    let y1 = (r.y + r.height as i32).min(height).max(y0);
    let pw = (x1 - x0) as usize;
    let ph = (y1 - y0) as usize;
    if pw == 0 || ph == 0 {
        return BlurPatch {
            width: 1,
            height: 1,
            bgra: vec![0, 0, 0, 255],
        };
    }
    let mut out = vec![0u8; pw * ph * 4];
    for row in 0..ph {
        let src = ((y0 as usize + row) * width as usize + x0 as usize) * 4;
        let dst = row * pw * 4;
        out[dst..dst + pw * 4].copy_from_slice(&bgra[src..src + pw * 4]);
    }
    BlurPatch {
        width: pw as u32,
        height: ph as u32,
        bgra: out,
    }
}

fn ui_font() -> Option<&'static FontArc> {
    static FONT: OnceLock<Option<FontArc>> = OnceLock::new();
    FONT.get_or_init(|| {
        std::fs::read(r"C:\Windows\Fonts\segoeui.ttf")
            .ok()
            .and_then(|data| FontArc::try_from_vec(data).ok())
    })
    .as_ref()
}

#[allow(clippy::too_many_arguments)]
fn bake_text(
    bgra: &mut [u8],
    width: i32,
    height: i32,
    at: (i32, i32),
    text: &str,
    color: Rgba,
    size: f32,
    font: &FontArc,
) {
    let scale = PxScale::from(size.max(6.0));
    let scaled = font.as_scaled(scale);
    let baseline = at.1 as f32 + scaled.ascent();
    let mut caret = at.0 as f32;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        let glyph = id.with_scale_and_position(scale, point(caret, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|x, y, coverage| {
                put_coverage(
                    bgra,
                    width,
                    height,
                    bounds.min.x as i32 + x as i32,
                    bounds.min.y as i32 + y as i32,
                    color,
                    coverage,
                );
            });
        }
        caret += scaled.h_advance(id);
    }
}

fn put_coverage(
    bgra: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    color: Rgba,
    coverage: f32,
) {
    if x < 0 || y < 0 || x >= width || y >= height || coverage <= 0.0 {
        return;
    }
    let alpha = (f32::from(color.a) * coverage.clamp(0.0, 1.0)).round() as u32;
    if alpha == 0 {
        return;
    }
    let i = (y as usize * width as usize + x as usize) * 4;
    let inv = 255 - alpha;
    let src = [color.b, color.g, color.r];
    for c in 0..3 {
        bgra[i + c] = ((u32::from(src[c]) * alpha + u32::from(bgra[i + c]) * inv) / 255) as u8;
    }
    bgra[i + 3] = 255;
}

fn put(bgra: &mut [u8], width: i32, height: i32, x: i32, y: i32, color: Rgba) {
    if x < 0 || y < 0 || x >= width || y >= height || color.a == 0 {
        return;
    }
    let i = (y as usize * width as usize + x as usize) * 4;
    let a = u32::from(color.a);
    if a == 255 {
        bgra[i] = color.b;
        bgra[i + 1] = color.g;
        bgra[i + 2] = color.r;
        bgra[i + 3] = 255;
        return;
    }
    let inv = 255 - a;
    let src = [color.b, color.g, color.r];
    for c in 0..3 {
        bgra[i + c] = ((u32::from(src[c]) * a + u32::from(bgra[i + c]) * inv) / 255) as u8;
    }
    bgra[i + 3] = 255;
}

/// Fast single-pass capsule segment rasterizer (zero pixel overdraw, smooth AA outer edge).
fn stroke_line(
    bgra: &mut [u8],
    width: i32,
    height: i32,
    a: (i32, i32),
    b: (i32, i32),
    color: Rgba,
    thick: f32,
) {
    if width <= 0 || height <= 0 || color.a == 0 {
        return;
    }
    let r = (thick * 0.5).max(0.5);
    let pad = r.ceil() as i32 + 1;
    let x0 = (a.0.min(b.0) - pad).max(0);
    let y0 = (a.1.min(b.1) - pad).max(0);
    let x1 = (a.0.max(b.0) + pad).min(width - 1);
    let y1 = (a.1.max(b.1) + pad).min(height - 1);
    if x1 < x0 || y1 < y0 {
        return;
    }
    let ax = a.0 as f32;
    let ay = a.1 as f32;
    let vx = (b.0 - a.0) as f32;
    let vy = (b.1 - a.1) as f32;
    let len2 = vx * vx + vy * vy;
    let r_outer = r + 0.5;
    let r_inner = (r - 0.5).max(0.0);
    let r_outer2 = r_outer * r_outer;

    for y in y0..=y1 {
        let py = y as f32;
        for x in x0..=x1 {
            let px = x as f32;
            let d2 = if len2 <= 1e-5 {
                (px - ax) * (px - ax) + (py - ay) * (py - ay)
            } else {
                let t = (((px - ax) * vx + (py - ay) * vy) / len2).clamp(0.0, 1.0);
                let dx = px - (ax + t * vx);
                let dy = py - (ay + t * vy);
                dx * dx + dy * dy
            };
            if d2 <= r_outer2 {
                let d = d2.sqrt();
                if d <= r_inner {
                    put(bgra, width, height, x, y, color);
                } else {
                    let cov = (r_outer - d).clamp(0.0, 1.0);
                    put_coverage(bgra, width, height, x, y, color, cov);
                }
            }
        }
    }
}

fn stroke_marker(
    bgra: &mut [u8],
    width: i32,
    height: i32,
    points: &[(i32, i32)],
    color: Rgba,
    thick: f32,
) {
    if width <= 0 || height <= 0 || points.is_empty() || color.a == 0 {
        return;
    }
    let r = (thick * 0.5).max(0.5);
    let r2 = r * r;
    let pad = r.ceil() as i32 + 1;
    let (mut min_x, mut min_y) = points[0];
    let (mut max_x, mut max_y) = points[0];
    for p in points {
        min_x = min_x.min(p.0);
        min_y = min_y.min(p.1);
        max_x = max_x.max(p.0);
        max_y = max_y.max(p.1);
    }
    let x0 = (min_x - pad).max(0);
    let y0 = (min_y - pad).max(0);
    let x1 = (max_x + pad + 1).min(width);
    let y1 = (max_y + pad + 1).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let pw = (x1 - x0) as usize;
    let ph = (y1 - y0) as usize;
    let mut mask = vec![0u8; pw * ph];
    {
        let mut stamp = |p: (i32, i32)| {
            let cx = p.0 - x0;
            let cy = p.1 - y0;
            for y in (cy - pad).max(0)..(cy + pad + 1).min(ph as i32) {
                for x in (cx - pad).max(0)..(cx + pad + 1).min(pw as i32) {
                    let dx = x - cx;
                    let dy = y - cy;
                    if (dx * dx + dy * dy) as f32 <= r2 {
                        mask[y as usize * pw + x as usize] = 255;
                    }
                }
            }
        };
        for p in points {
            stamp(*p);
        }
        for pair in points.windows(2) {
            let dx = pair[1].0 - pair[0].0;
            let dy = pair[1].1 - pair[0].1;
            let steps = dx.abs().max(dy.abs()).max(1);
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                stamp((
                    (pair[0].0 as f32 + dx as f32 * t).round() as i32,
                    (pair[0].1 as f32 + dy as f32 * t).round() as i32,
                ));
            }
        }
    }
    let a = u32::from(color.a);
    let inv = 255 - a;
    let src = [color.b, color.g, color.r];
    for y in 0..ph {
        for x in 0..pw {
            if mask[y * pw + x] == 0 {
                continue;
            }
            let i = ((y0 as usize + y) * width as usize + (x0 as usize + x)) * 4;
            for c in 0..3 {
                bgra[i + c] = ((u32::from(src[c]) * a + u32::from(bgra[i + c]) * inv) / 255) as u8;
            }
            bgra[i + 3] = 255;
        }
    }
}

fn fill_rect(bgra: &mut [u8], width: i32, height: i32, r: Region, color: Rgba) {
    let x0 = r.x.max(0);
    let y0 = r.y.max(0);
    let x1 = (r.x + r.width as i32).min(width);
    let y1 = (r.y + r.height as i32).min(height);
    for y in y0..y1 {
        for x in x0..x1 {
            put(bgra, width, height, x, y, color);
        }
    }
}

fn stroke_rect(bgra: &mut [u8], width: i32, height: i32, r: Region, color: Rgba, thick: f32) {
    let t = thick.round().max(1.0) as i32;
    let right = r.x + r.width as i32;
    let bottom = r.y + r.height as i32;
    for y in r.y.max(0)..bottom.min(height) {
        for x in r.x.max(0)..right.min(width) {
            if x < r.x + t || x >= right - t || y < r.y + t || y >= bottom - t {
                put(bgra, width, height, x, y, color);
            }
        }
    }
}

fn fill_ellipse(bgra: &mut [u8], width: i32, height: i32, r: Region, color: Rgba) {
    if r.width < 2 || r.height < 2 {
        return;
    }
    let rx = r.width as f32 * 0.5;
    let ry = r.height as f32 * 0.5;
    let cx = r.x as f32 + rx;
    let cy = r.y as f32 + ry;
    let right = (r.x + r.width as i32).min(width);
    let bottom = (r.y + r.height as i32).min(height);
    for y in r.y.max(0)..bottom {
        for x in r.x.max(0)..right {
            let nx = (x as f32 + 0.5 - cx) / rx;
            let ny = (y as f32 + 0.5 - cy) / ry;
            if nx * nx + ny * ny <= 1.0 {
                put(bgra, width, height, x, y, color);
            }
        }
    }
}

fn stroke_ellipse(bgra: &mut [u8], width: i32, height: i32, r: Region, color: Rgba, thick: f32) {
    if r.width < 2 || r.height < 2 {
        return;
    }
    let rx = r.width as f32 * 0.5;
    let ry = r.height as f32 * 0.5;
    let cx = r.x as f32 + rx;
    let cy = r.y as f32 + ry;
    let inner = (1.0 - thick / rx.min(ry)).max(0.0);
    let right = (r.x + r.width as i32).min(width);
    let bottom = (r.y + r.height as i32).min(height);
    for y in r.y.max(0)..bottom {
        for x in r.x.max(0)..right {
            let nx = (x as f32 + 0.5 - cx) / rx;
            let ny = (y as f32 + 0.5 - cy) / ry;
            let d = nx * nx + ny * ny;
            if d <= 1.0 && d >= inner * inner {
                put(bgra, width, height, x, y, color);
            }
        }
    }
}

fn fill_circle(bgra: &mut [u8], width: i32, height: i32, c: (i32, i32), radius: i32, color: Rgba) {
    let r = radius as f32;
    let r_outer = r + 0.5;
    let r_inner = (r - 0.5).max(0.0);
    let pad = radius + 1;
    for y in (c.1 - pad).max(0)..=(c.1 + pad).min(height - 1) {
        for x in (c.0 - pad).max(0)..=(c.0 + pad).min(width - 1) {
            let d = ((x - c.0) as f32).hypot((y - c.1) as f32);
            if d <= r_inner {
                put(bgra, width, height, x, y, color);
            } else if d <= r_outer {
                put_coverage(bgra, width, height, x, y, color, r_outer - d);
            }
        }
    }
}

fn fill_triangle(
    bgra: &mut [u8],
    width: i32,
    height: i32,
    a: (i32, i32),
    b: (i32, i32),
    c: (i32, i32),
    color: Rgba,
) {
    let min_x = a.0.min(b.0).min(c.0).max(0);
    let max_x = a.0.max(b.0).max(c.0).min(width - 1);
    let min_y = a.1.min(b.1).min(c.1).max(0);
    let max_y = a.1.max(b.1).max(c.1).min(height - 1);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if point_in_triangle((x, y), a, b, c) {
                put(bgra, width, height, x, y, color);
            }
        }
    }
}

pub fn blur_region(bgra: &mut [u8], width: i32, height: i32, r: Region, radius: usize) {
    if r.width < 2 || r.height < 2 || radius == 0 {
        return;
    }
    let x0 = r.x.max(0);
    let y0 = r.y.max(0);
    let x1 = (r.x + r.width as i32).min(width);
    let y1 = (r.y + r.height as i32).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let pad = radius as i32;
    let sx0 = (x0 - pad).max(0);
    let sy0 = (y0 - pad).max(0);
    let sx1 = (x1 + pad).min(width);
    let sy1 = (y1 + pad).min(height);
    let sw = (sx1 - sx0) as usize;
    let sh = (sy1 - sy0) as usize;
    let stride = width as usize * 4;
    let mut patch = vec![0u8; sw * sh * 4];
    for row in 0..sh {
        let src = (sy0 as usize + row) * stride + sx0 as usize * 4;
        patch[row * sw * 4..(row + 1) * sw * 4].copy_from_slice(&bgra[src..src + sw * 4]);
    }
    box_blur(&mut patch, sw, sh, radius);
    let rw = (x1 - x0) as usize;
    let rh = (y1 - y0) as usize;
    let off_x = (x0 - sx0) as usize;
    let off_y = (y0 - sy0) as usize;
    for row in 0..rh {
        let src = ((off_y + row) * sw + off_x) * 4;
        let dst = (y0 as usize + row) * stride + x0 as usize * 4;
        bgra[dst..dst + rw * 4].copy_from_slice(&patch[src..src + rw * 4]);
    }
}

pub fn pixelate_region(bgra: &mut [u8], width: i32, height: i32, r: Region, block: i32) {
    if block < 2 || r.width < 2 || r.height < 2 {
        return;
    }
    let x0 = r.x.max(0);
    let y0 = r.y.max(0);
    let x1 = (r.x + r.width as i32).min(width);
    let y1 = (r.y + r.height as i32).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let stride = width as usize * 4;
    let start_y = y0 - y0.rem_euclid(block);
    let start_x = x0 - x0.rem_euclid(block);
    let mut by = start_y;
    while by < y1 {
        let top = by.max(y0);
        let bottom = (by + block).min(y1);
        if bottom > top {
            let mut bx = start_x;
            while bx < x1 {
                let left = bx.max(x0);
                let right = (bx + block).min(x1);
                if right > left {
                    let mut sums = [0u32; 3];
                    let mut count = 0u32;
                    for y in top..bottom {
                        for x in left..right {
                            let i = y as usize * stride + x as usize * 4;
                            for (c, sum) in sums.iter_mut().enumerate() {
                                *sum += u32::from(bgra[i + c]);
                            }
                            count += 1;
                        }
                    }
                    let count = count.max(1);
                    let avg = [
                        (sums[0] / count) as u8,
                        (sums[1] / count) as u8,
                        (sums[2] / count) as u8,
                    ];
                    for y in top..bottom {
                        for x in left..right {
                            let i = y as usize * stride + x as usize * 4;
                            bgra[i..i + 3].copy_from_slice(&avg);
                            bgra[i + 3] = 255;
                        }
                    }
                }
                bx += block;
            }
        }
        by += block;
    }
}

fn stamp_digits(bgra: &mut [u8], width: i32, height: i32, center: (i32, i32), n: u32, ink: Rgba) {
    let digits = n.to_string();
    let glyph_w = 4;
    let total = digits.len() as i32 * glyph_w - 1;
    let mut x = center.0 - total / 2;
    let y = center.1 - 2;
    for ch in digits.bytes() {
        let bits = DIGITS[usize::from(ch - b'0')];
        for (row, bits) in bits.iter().enumerate() {
            for col in 0..3i32 {
                if bits & (1 << (2 - col)) != 0 {
                    put(bgra, width, height, x + col, y + row as i32, ink);
                }
            }
        }
        x += glyph_w;
    }
}

const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_anchor_flips_above_when_below_clips() {
        let below = toolbar_anchor((10.0, 10.0, 80.0, 40.0), (400.0, 300.0), 44.0, 8.0);
        assert_eq!(below, ((90.0, 58.0), false));
        let above = toolbar_anchor((10.0, 260.0, 80.0, 30.0), (400.0, 300.0), 44.0, 8.0);
        assert_eq!(above, ((90.0, 252.0), true));
        let pinned = toolbar_anchor((10.0, 5.0, 380.0, 290.0), (400.0, 300.0), 44.0, 8.0);
        assert_eq!(pinned, ((390.0, 296.0), true));
    }

    #[test]
    fn undo_redo_caps_at_limit_and_drops_redo_on_push() {
        let mut ed = Editor::default();
        for n in 0..HISTORY_LIMIT + 3 {
            ed.push(Shape::Number {
                at: (n as i32, 0),
                n: n as u32,
                color: PALETTE[0],
            });
        }
        assert_eq!(ed.shapes.len(), HISTORY_LIMIT);
        ed.undo();
        assert!(ed.can_redo());
        ed.push(Shape::Arrow {
            a: (0, 0),
            b: (1, 1),
            color: PALETTE[0],
            width: 2.0,
        });
        assert!(!ed.can_redo());
    }

    #[test]
    fn select_move_resize_recolor_delete_and_undo() {
        let ctx = egui::Context::default();
        let text_layout = TextLayout {
            ctx: &ctx,
            px_to_ui: 1.0,
        };
        let mut ed = Editor::default();
        ed.push(Shape::Line {
            a: (20, 20),
            b: (120, 20),
            color: Rgba::rgb(255, 0, 0),
            width: 4.0,
        });
        ed.push(Shape::Rect {
            a: (40, 50),
            b: (140, 110),
            color: Rgba::rgb(0, 255, 0),
            width: 3.0,
            filled: false,
        });

        // Hit-test line and rect edge
        assert_eq!(hit_shape(&ed.shapes, (70, 21), 5.0, text_layout), Some(0));
        assert_eq!(hit_shape(&ed.shapes, (40, 80), 5.0, text_layout), Some(1));
        // Hollow interior should not hit rect
        assert_eq!(hit_shape(&ed.shapes, (90, 80), 5.0, text_layout), None);

        // Select line, drag End handle, recolor, and delete; verify full undo chain
        ed.select_shape_index(Some(0));
        assert_eq!(
            hit_shape_handle(&ed.shapes[0], (120, 20), 8, text_layout),
            Some(ShapeHandle::End)
        );
        ed.record_snapshot();
        let origin = ed.shapes[0].clone();
        ed.shapes[0] =
            resize_shape_from_origin(&origin, ShapeHandle::End, (120, 20), (160, 60), text_layout);
        assert!(matches!(ed.shapes[0], Shape::Line { b: (160, 60), .. }));

        ed.apply_color(Rgba::rgb(0, 100, 250));
        assert_eq!(ed.shapes[0].color(), Some(Rgba::rgb(0, 100, 250)));

        assert!(ed.delete_selected());
        assert_eq!(ed.shapes.len(), 1);
        ed.undo();
        assert_eq!(ed.shapes.len(), 2);
        ed.undo();
        assert_eq!(ed.shapes[0].color(), Some(Rgba::rgb(255, 0, 0)));
        ed.undo();
        assert!(matches!(ed.shapes[0], Shape::Line { b: (120, 20), .. }));
    }

    #[test]
    fn pen_thins_points_before_commit() {
        let mut ed = Editor {
            tool: Tool::Pen,
            ..Editor::default()
        };
        ed.begin((0, 0));
        ed.extend((1, 0));
        ed.extend((8, 0));
        ed.commit();
        let Shape::Pen { points, .. } = &ed.shapes[0] else {
            panic!("pen");
        };
        assert_eq!(points, &[(0, 0), (8, 0)]);
    }

    #[test]
    fn selecting_marker_switches_to_yellow() {
        let mut ed = Editor::default();
        ed.select_tool(Tool::Marker);
        assert_eq!(ed.color, MARKER_YELLOW);
        assert_eq!(ed.tool, Tool::Marker);
    }

    #[test]
    fn marker_is_wide_and_translucent() {
        let mut ed = Editor {
            tool: Tool::Marker,
            color: MARKER_YELLOW,
            ..Editor::default()
        };
        ed.begin((10, 10));
        ed.extend((30, 10));
        assert!(ed.commit());
        let Shape::Marker {
            points,
            color,
            width,
        } = &ed.shapes[0]
        else {
            panic!("marker");
        };
        assert_eq!(points, &[(10, 10), (30, 10)]);
        assert_eq!(*width, MARKER_WIDTH);
        assert_eq!(color.a, MARKER_ALPHA);
        assert_eq!((color.r, color.g, color.b), (255, 255, 0));
    }

    #[test]
    fn marker_bakes_uniform_alpha_once() {
        let sel = Region {
            x: 0,
            y: 0,
            width: 32,
            height: 8,
        };
        let mut px = vec![0u8; 32 * 8 * 4];
        bake(
            &mut px,
            sel,
            &[Shape::Marker {
                points: vec![(0, 4), (31, 4), (0, 4)],
                color: Rgba {
                    a: MARKER_ALPHA,
                    ..MARKER_YELLOW
                },
                width: 6.0,
            }],
        );
        let i = (4 * 32 + 10) * 4;
        assert_eq!(&px[i..i + 3], &[0, 128, 128]);
        assert_eq!(px[i + 3], 255);
    }

    #[test]
    fn redo_restores_multiple_shapes() {
        let mut ed = Editor::default();
        ed.place_number((1, 1));
        ed.place_number((2, 2));
        ed.undo();
        ed.undo();
        assert!(ed.shapes.is_empty());
        ed.redo();
        ed.redo();
        assert_eq!(ed.shapes.len(), 2);
        assert!(!ed.can_redo());
    }

    #[test]
    fn number_resets_counter_on_undo() {
        let mut ed = Editor::default();
        ed.place_number((1, 1));
        ed.place_number((2, 2));
        assert_eq!(ed.next_number, 3);
        ed.undo();
        assert_eq!(ed.next_number, 2);
    }

    #[test]
    fn blur_patch_keeps_stroke_underneath_and_excludes_later_stroke() {
        let (w, h) = (16, 16);
        let source = vec![255u8; w * h * 4];
        let under = Shape::Line {
            a: (0, 8),
            b: (15, 8),
            color: Rgba::rgb(0, 255, 0),
            width: 2.0,
        };
        let blur = Shape::Blur {
            a: (0, 4),
            b: (16, 12),
            kind: BlurKind::Soft,
        };
        let over = Shape::Line {
            a: (0, 8),
            b: (15, 8),
            color: Rgba::rgb(255, 0, 0),
            width: 2.0,
        };
        let kept = blur_patches(
            &source,
            w as i32,
            h as i32,
            (0, 0),
            1,
            &[under, blur.clone()],
        );
        assert_eq!(kept.len(), 1);
        assert!(
            kept[0].bgra.chunks(4).any(|p| p[1] > p[0]),
            "blur must keep the green stroke, not replace it with the source"
        );
        let later = blur_patches(&source, w as i32, h as i32, (0, 0), 1, &[blur, over]);
        assert!(
            later[0]
                .bgra
                .chunks(4)
                .all(|p| p[0] == 255 && p[1] == 255 && p[2] == 255),
            "a stroke drawn after the blur must not be baked into that blur"
        );
    }

    #[test]
    fn bake_strokes_line_and_blurs_rect() {
        let sel = Region {
            x: 10,
            y: 20,
            width: 8,
            height: 8,
        };
        let mut px = vec![255u8; 8 * 8 * 4];
        px[2 * 8 * 4..2 * 8 * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
        bake(
            &mut px,
            sel,
            &[
                Shape::Line {
                    a: (10, 20),
                    b: (17, 20),
                    color: Rgba::rgb(0, 255, 0),
                    width: 1.0,
                },
                Shape::Blur {
                    a: (10, 22),
                    b: (18, 28),
                    kind: BlurKind::Soft,
                },
            ],
        );
        assert_eq!(&px[1..3], &[255, 0]);
        assert!(px[2 * 8 * 4] > 0);
    }

    #[test]
    fn box_blur_averages_neighbors() {
        let mut px = vec![0u8; 3 * 3 * 4];
        px[4 * 4..4 * 5].copy_from_slice(&[255, 0, 0, 255]);
        box_blur(&mut px, 3, 3, 1);
        assert!(px[0] > 0 && px[0] < 255);
        assert!(px[4 * 4] > 0 && px[4 * 4] < 255);
    }

    #[test]
    fn mosaic_makes_each_block_uniform() {
        let mut px = vec![0u8; 20 * 20 * 4];
        for y in 0..20 {
            for x in 0..20 {
                let i = (y * 20 + x) * 4;
                let v = if (x + y) % 2 == 0 { 0 } else { 255 };
                px[i..i + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        pixelate_region(
            &mut px,
            20,
            20,
            Region {
                x: 0,
                y: 0,
                width: 20,
                height: 20,
            },
            5,
        );
        for block_y in 0..4 {
            for block_x in 0..4 {
                let first = ((block_y * 5 * 20) + block_x * 5) * 4;
                for y in block_y * 5..block_y * 5 + 5 {
                    for x in block_x * 5..block_x * 5 + 5 {
                        let i = (y * 20 + x) * 4;
                        assert_eq!(&px[i..i + 3], &px[first..first + 3]);
                    }
                }
            }
        }
    }

    #[test]
    fn text_bounds_and_handles_follow_the_full_annotation() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            for px_to_ui in [0.4, 2.0 / 3.0, 1.0, 2.5] {
                let layout = TextLayout { ctx, px_to_ui };
                let at = (-100, 100);
                let size = 60.0;
                let text = "ENESSSSSSSSSSSSSSSSSSSS";
                let shapes = [Shape::Text {
                    at,
                    text: text.into(),
                    color: Rgba::rgb(255, 255, 255),
                    size,
                }];
                let b = layout.bounds(&shapes[0]).expect("text box");
                let ink = layout.galley(text, size).mesh_bounds;
                let last_glyph = (
                    at.0 + ((ink.right() - 2.0) / px_to_ui).floor() as i32,
                    at.1 + (ink.center().y / px_to_ui).round() as i32,
                );
                assert_eq!(hit_text(&shapes, last_glyph, layout), Some(0));
                assert_eq!(hit_shape(&shapes, last_glyph, 4.0, layout), Some(0));
                let handle = (b.x + b.width as i32, b.y + b.height as i32);
                assert_eq!(
                    hit_shape_handle(&shapes[0], handle, 4, layout),
                    Some(ShapeHandle::BottomRight)
                );
                let resized = resize_shape_from_origin(
                    &shapes[0],
                    ShapeHandle::BottomRight,
                    handle,
                    (handle.0, handle.1 + 40),
                    layout,
                );
                assert!(layout.bounds(&resized).unwrap().width > b.width);
            }
        });
    }

    #[test]
    fn width_updates_the_draft_shape() {
        let mut ed = Editor {
            tool: Tool::Line,
            ..Editor::default()
        };
        ed.begin((10, 10));
        ed.set_width(12.0);
        let Shape::Line { width, .. } = ed.draft.as_ref().unwrap() else {
            panic!("line draft");
        };
        assert_eq!(*width, 12.0);
        assert_eq!(ed.text_size(), 60.0);
    }

    #[test]
    fn shape_group_tracks_triangle_and_bakes_edges() {
        let mut ed = Editor::default();
        assert_eq!(ed.shape_tool, Tool::Rect);
        ed.select_shape(Tool::Triangle);
        assert_eq!(ed.shape_tool, Tool::Triangle);
        assert_eq!(ed.tool, Tool::Triangle);

        ed.begin((10, 10));
        ed.extend((30, 30));
        assert!(ed.commit());
        assert!(matches!(ed.shapes[0], Shape::Triangle { .. }));

        let sel = Region {
            x: 0,
            y: 0,
            width: 40,
            height: 40,
        };
        let mut px = vec![0u8; 40 * 40 * 4];
        bake(&mut px, sel, &ed.shapes);
        let at = |x: usize, y: usize| {
            let i = (y * 40 + x) * 4;
            px[i + 2] > 0
        };
        assert!(at(20, 10));
        assert!(at(10, 30));
        assert!(at(30, 30));
    }

    #[test]
    fn crop_bgra_extracts_exact_subregion() {
        let mut src = vec![0u8; 4 * 4 * 4];
        // Mark pixel (2, 1) with red (B=0, G=0, R=200, A=255)
        let idx = (4 + 2) * 4;
        src[idx..idx + 4].copy_from_slice(&[0, 0, 200, 255]);
        let (cropped, sz) = crop_bgra(
            &src,
            4,
            4,
            Region {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
        )
        .unwrap();
        assert_eq!(sz, [2, 2]);
        assert_eq!(&cropped[4..8], &[0, 0, 200, 255]);
    }
}
