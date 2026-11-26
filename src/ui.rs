use super::{App, Page};
use crate::{
    capture::Region,
    edit::{self, BlurKind, Rgba, Shape, Tool},
    i18n::{Lang, Strings},
    studio::{self, CropDrag, ShapeDrag},
};
use eframe::egui::{self, Color32, CornerRadius, Margin, Pos2, Rect, RichText, Stroke, Vec2};
use std::path::PathBuf;
use std::time::SystemTime;

const SECTION_SIZE: f32 = 13.5;
const BODY_SIZE: f32 = 12.5;
const LABEL_SIZE: f32 = 11.5;
const CAPTION_SIZE: f32 = 11.0;

const CONTROL_H: f32 = 26.0;
const TOP_BAR_H: f32 = 38.0;
const EDITOR_BAR_H: f32 = 34.0;
const PROP_BAR_H: f32 = 32.0;
const STATUS_BAR_H: f32 = 24.0;

const RADIUS_SM: f32 = 3.0;
const RADIUS_SM_U8: u8 = 3;
const RADIUS_MD: f32 = 4.0;
const RADIUS_MD_U8: u8 = 4;

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub light: bool,
    pub canvas: Color32,
    pub bg: Color32,
    pub bar: Color32,
    pub panel: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub hover: Color32,
    pub select: Color32,
    pub accent: Color32,
    pub accent_bright: Color32,
    pub accent_fg: Color32,
    pub success: Color32,
    pub err: Color32,
    pub err_soft: Color32,
}

pub(super) fn palette(light: bool) -> Palette {
    let c = Color32::from_rgb;
    if light {
        Palette {
            light: true,
            canvas: c(222, 226, 232),
            bg: c(243, 244, 246),
            bar: c(255, 255, 255),
            panel: c(248, 249, 251),
            text: c(22, 25, 30),
            muted: c(82, 88, 102),
            line: c(208, 213, 221),
            hover: c(233, 236, 241),
            select: c(219, 234, 254),
            accent: c(0, 103, 192),
            accent_bright: c(0, 103, 192),
            accent_fg: c(255, 255, 255),
            success: c(26, 127, 55),
            err: c(207, 34, 46),
            err_soft: Color32::from_rgba_unmultiplied(207, 34, 46, 20),
        }
    } else {
        Palette {
            light: false,
            canvas: c(21, 22, 25),
            bg: c(28, 29, 33),
            bar: c(36, 38, 43),
            panel: c(43, 46, 52),
            text: c(236, 238, 242),
            muted: c(150, 157, 168),
            line: c(56, 60, 69),
            hover: c(54, 58, 67),
            select: c(30, 66, 104),
            accent: c(14, 112, 192),
            accent_bright: c(64, 160, 245),
            accent_fg: c(255, 255, 255),
            success: c(35, 134, 54),
            err: c(229, 83, 75),
            err_soft: Color32::from_rgba_unmultiplied(229, 83, 75, 28),
        }
    }
}

pub(super) fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
        fonts
            .font_data
            .insert("Segoe UI".into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "Segoe UI".into());
    }
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\consola.ttf") {
        fonts
            .font_data
            .insert("Consolas".into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "Consolas".into());
    }
    ctx.set_fonts(fonts);
}

fn style(ctx: &egui::Context, light: bool) {
    let p = palette(light);
    ctx.style_mut(|s| {
        s.spacing.interact_size.y = CONTROL_H;
        s.spacing.button_padding = egui::vec2(9.0, 4.0);
        s.spacing.item_spacing = egui::vec2(6.0, 6.0);
        s.spacing.icon_width = 15.0;
        s.spacing.icon_width_inner = 9.0;
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(BODY_SIZE));
        s.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::proportional(LABEL_SIZE),
        );
        s.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::proportional(CAPTION_SIZE),
        );
        s.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::monospace(CAPTION_SIZE),
        );
        s.visuals = if light {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        let v = &mut s.visuals;
        v.override_text_color = Some(p.text);
        v.panel_fill = p.bg;
        v.window_fill = p.bar;
        v.extreme_bg_color = p.canvas;
        v.faint_bg_color = p.bar;
        v.selection.bg_fill = p.select;
        v.selection.stroke = Stroke::new(1.0_f32, p.accent_bright);
        v.window_corner_radius = CornerRadius::same(RADIUS_MD_U8);
        v.menu_corner_radius = CornerRadius::same(RADIUS_MD_U8);
        v.window_stroke = Stroke::new(1.0_f32, p.line);
        v.widgets.noninteractive.bg_fill = p.panel;
        v.widgets.noninteractive.weak_bg_fill = p.panel;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.line);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.muted);
        v.widgets.noninteractive.corner_radius = CornerRadius::same(RADIUS_SM_U8);
        for (w, fill, border) in [
            (&mut v.widgets.inactive, p.panel, p.line),
            (&mut v.widgets.hovered, p.hover, p.muted),
            (&mut v.widgets.active, p.select, p.accent_bright),
            (&mut v.widgets.open, p.select, p.accent_bright),
        ] {
            w.bg_fill = fill;
            w.weak_bg_fill = fill;
            w.bg_stroke = Stroke::new(1.0_f32, border);
            w.fg_stroke = Stroke::new(1.0_f32, p.text);
            w.corner_radius = CornerRadius::same(RADIUS_SM_U8);
            w.expansion = 0.0;
        }
    });
}

#[derive(Clone, Copy)]
pub(super) enum UiIcon {
    Album,
    Editor,
    Settings,
    Capture,
    Search,
    Refresh,
    Folder,
    Copy,
    Save,
    SaveCopy,
    Paste,
    Open,
    Delete,
    Sun,
    Moon,
    Clear,
    Crop,
    Undo,
    Redo,
    Check,
    Duplicate,
    Front,
}

pub(super) fn draw_ui_icon(painter: &egui::Painter, rect: Rect, icon: UiIcon, color: Color32) {
    let s = (rect.width().min(rect.height()) / 16.0).max(0.75);
    let c = rect.center();
    let stroke = Stroke::new(1.45 * s, color);
    let pt = |dx: f32, dy: f32| c + egui::vec2(dx * s, dy * s);

    match icon {
        UiIcon::Album => {
            let gap = 1.3 * s;
            let cell = 4.5 * s;
            for (ox, oy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let min = egui::pos2(
                    c.x + if ox < 0.0 {
                        -gap * 0.5 - cell
                    } else {
                        gap * 0.5
                    },
                    c.y + if oy < 0.0 {
                        -gap * 0.5 - cell
                    } else {
                        gap * 0.5
                    },
                );
                painter.rect_stroke(
                    Rect::from_min_size(min, egui::vec2(cell, cell)),
                    1.0 * s,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
        }
        UiIcon::Editor => {
            // Frame + pen nib
            let box_r = Rect::from_min_max(pt(-5.5, -5.0), pt(3.5, 4.5));
            painter.rect_stroke(box_r, 1.2 * s, stroke, egui::StrokeKind::Middle);
            painter.line_segment([pt(0.5, 1.5), pt(5.8, -3.8)], Stroke::new(1.8 * s, color));
            painter.circle_filled(pt(-0.2, 2.2), 1.0 * s, color);
        }
        UiIcon::Settings => {
            for (y, kx) in [(-4.0, -2.0), (0.0, 2.4), (4.0, -1.0)] {
                painter.line_segment([pt(-5.5, y), pt(5.5, y)], stroke);
                painter.circle_filled(pt(kx, y), 1.75 * s, color);
            }
        }
        UiIcon::Capture => {
            let d = 5.2;
            let arm = 2.5;
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = pt(sx * d, sy * d);
                painter.line_segment([corner, pt(sx * (d - arm), sy * d)], stroke);
                painter.line_segment([corner, pt(sx * d, sy * (d - arm))], stroke);
            }
            painter.circle_filled(c, 1.7 * s, color);
        }
        UiIcon::Search => {
            painter.circle_stroke(pt(-1.2, -1.2), 4.1 * s, stroke);
            painter.line_segment([pt(1.8, 1.8), pt(5.4, 5.4)], stroke);
        }
        UiIcon::Refresh => {
            let r = 4.7 * s;
            let pts: Vec<_> = (0..=12)
                .map(|i| {
                    let a = -0.35 + (i as f32 / 12.0) * std::f32::consts::PI * 1.58;
                    c + egui::vec2(a.cos() * r, a.sin() * r)
                })
                .collect();
            painter.add(egui::Shape::line(pts, stroke));
            let tip = pt(4.8, -1.8);
            painter.line_segment([tip, pt(2.1, -2.8)], stroke);
            painter.line_segment([tip, pt(5.3, -4.5)], stroke);
        }
        UiIcon::Folder => {
            let box_rect = Rect::from_center_size(pt(0.0, 0.9), egui::vec2(11.4 * s, 8.2 * s));
            painter.rect_stroke(box_rect, 1.2 * s, stroke, egui::StrokeKind::Middle);
            painter.line_segment(
                [pt(-5.0, -3.4), pt(-1.4, -3.4)],
                Stroke::new(1.9 * s, color),
            );
        }
        UiIcon::Copy => {
            let front = Rect::from_min_size(pt(-4.8, -2.2), egui::vec2(7.4 * s, 7.6 * s));
            painter.rect_stroke(front, 1.2 * s, stroke, egui::StrokeKind::Middle);
            painter.line_segment([pt(-2.2, -4.8), pt(4.8, -4.8)], stroke);
            painter.line_segment([pt(4.8, -4.8), pt(4.8, 2.6)], stroke);
        }
        UiIcon::Save => {
            // Floppy / diskette crisp icon
            let outer = Rect::from_center_size(c, egui::vec2(10.6 * s, 10.6 * s));
            painter.rect_stroke(outer, 1.3 * s, stroke, egui::StrokeKind::Middle);
            let shutter = Rect::from_min_max(pt(-3.0, -5.2), pt(2.6, -1.4));
            painter.rect_stroke(shutter, 0.6 * s, stroke, egui::StrokeKind::Middle);
            let label = Rect::from_min_max(pt(-3.4, 0.8), pt(3.4, 5.2));
            painter.rect_stroke(label, 0.6 * s, stroke, egui::StrokeKind::Middle);
        }
        UiIcon::SaveCopy => {
            let outer = Rect::from_min_size(pt(-5.4, -3.6), egui::vec2(8.8 * s, 8.8 * s));
            painter.rect_stroke(outer, 1.2 * s, stroke, egui::StrokeKind::Middle);
            painter.line_segment([pt(-2.4, -5.4), pt(5.2, -5.4)], stroke);
            painter.line_segment([pt(5.2, -5.4), pt(5.2, 2.2)], stroke);
            painter.line_segment([pt(-1.0, -1.2), pt(-1.0, 2.8)], stroke);
            painter.line_segment([pt(-3.0, 0.8), pt(1.0, 0.8)], stroke);
        }
        UiIcon::Paste => {
            let board = Rect::from_min_max(pt(-4.6, -3.8), pt(4.6, 5.4));
            painter.rect_stroke(board, 1.2 * s, stroke, egui::StrokeKind::Middle);
            let clip = Rect::from_min_max(pt(-2.2, -5.4), pt(2.2, -2.8));
            painter.rect_filled(clip, 0.8 * s, color);
        }
        UiIcon::Open => {
            painter.line_segment([pt(-0.5, -4.8), pt(-5.0, -4.8)], stroke);
            painter.line_segment([pt(-5.0, -4.8), pt(-5.0, 4.8)], stroke);
            painter.line_segment([pt(-5.0, 4.8), pt(4.8, 4.8)], stroke);
            painter.line_segment([pt(4.8, 4.8), pt(4.8, 0.5)], stroke);
            painter.line_segment([pt(-0.5, 0.5), pt(5.0, -5.0)], stroke);
            painter.line_segment([pt(1.2, -5.0), pt(5.0, -5.0)], stroke);
            painter.line_segment([pt(5.0, -5.0), pt(5.0, -1.2)], stroke);
        }
        UiIcon::Delete => {
            painter.line_segment([pt(-5.0, -3.5), pt(5.0, -3.5)], stroke);
            painter.line_segment([pt(-2.0, -5.2), pt(2.0, -5.2)], stroke);
            let body = Rect::from_min_max(pt(-3.8, -3.5), pt(3.8, 5.2));
            painter.rect_stroke(body, 1.1 * s, stroke, egui::StrokeKind::Middle);
            painter.line_segment([pt(-1.3, -1.2), pt(-1.3, 3.0)], stroke);
            painter.line_segment([pt(1.3, -1.2), pt(1.3, 3.0)], stroke);
        }
        UiIcon::Sun => {
            painter.circle_stroke(c, 2.7 * s, stroke);
            for i in 0..8 {
                let a = (i as f32) * std::f32::consts::FRAC_PI_4;
                let dir = egui::vec2(a.cos(), a.sin());
                painter.line_segment([c + dir * (4.3 * s), c + dir * (5.8 * s)], stroke);
            }
        }
        UiIcon::Moon => {
            // Exact geometric crescent from two intersecting circles (zero stray segments)
            let r1 = 4.6 * s;
            let d_vec = egui::vec2(2.3 * s, -1.8 * s);
            let d = d_vec.length();
            let r2 = 4.1 * s;
            let base = d_vec.y.atan2(d_vec.x);
            let cos_a1 = ((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1)).clamp(-1.0, 1.0);
            let a1 = cos_a1.acos();
            let p_top = c + egui::vec2((base - a1).cos() * r1, (base - a1).sin() * r1);
            let p_bot = c + egui::vec2((base + a1).cos() * r1, (base + a1).sin() * r1);
            let c2 = c + d_vec;
            let t_top = (p_top.y - c2.y).atan2(p_top.x - c2.x);
            let t_bot = (p_bot.y - c2.y).atan2(p_bot.x - c2.x);
            let inner_delta = (t_bot - t_top + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let outer_start = base + a1;
            let outer_end = base - a1 + std::f32::consts::TAU;
            let mut pts = Vec::with_capacity(28);
            for i in 0..=14 {
                let a = egui::lerp(outer_start..=outer_end, i as f32 / 14.0);
                pts.push(c + egui::vec2(a.cos() * r1, a.sin() * r1));
            }
            for i in 1..12 {
                let a = t_top + inner_delta * (i as f32 / 12.0);
                pts.push(c2 + egui::vec2(a.cos() * r2, a.sin() * r2));
            }
            painter
                .with_clip_rect(rect)
                .add(egui::Shape::closed_line(pts, stroke));
        }
        UiIcon::Clear => {
            painter.line_segment([pt(-3.6, -3.6), pt(3.6, 3.6)], stroke);
            painter.line_segment([pt(3.6, -3.6), pt(-3.6, 3.6)], stroke);
        }
        UiIcon::Crop => {
            painter.line_segment([pt(-3.4, -6.0), pt(-3.4, 3.4)], stroke);
            painter.line_segment([pt(-3.4, 3.4), pt(6.0, 3.4)], stroke);
            painter.line_segment([pt(-6.0, -3.4), pt(3.4, -3.4)], stroke);
            painter.line_segment([pt(3.4, -3.4), pt(3.4, 6.0)], stroke);
        }
        UiIcon::Undo => super::return_arrow(painter, c, true, color),
        UiIcon::Redo => super::return_arrow(painter, c, false, color),
        UiIcon::Check => {
            painter.add(egui::Shape::line(
                vec![pt(-4.2, 0.2), pt(-1.2, 3.2), pt(4.6, -3.2)],
                Stroke::new(1.7 * s, color),
            ));
        }
        UiIcon::Duplicate => {
            let a = Rect::from_min_max(pt(-5.2, -5.2), pt(1.8, 1.8));
            let b = Rect::from_min_max(pt(-1.8, -1.8), pt(5.2, 5.2));
            painter.rect_stroke(a, 1.0 * s, stroke, egui::StrokeKind::Middle);
            painter.rect_stroke(b, 1.0 * s, stroke, egui::StrokeKind::Middle);
        }
        UiIcon::Front => {
            painter.line_segment([pt(0.0, 4.5), pt(0.0, -4.2)], stroke);
            painter.line_segment([pt(-3.2, -1.0), pt(0.0, -4.4)], stroke);
            painter.line_segment([pt(3.2, -1.0), pt(0.0, -4.4)], stroke);
            painter.line_segment([pt(-4.5, -5.4), pt(4.5, -5.4)], stroke);
        }
    }
}

fn focus_ring(ui: &egui::Ui, response: &egui::Response, p: Palette) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(1.5),
            RADIUS_SM + 1.0,
            Stroke::new(1.5_f32, p.accent_bright),
            egui::StrokeKind::Outside,
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BtnKind {
    Subtle,
    SolidAccent,
    SolidSuccess,
    Danger,
    ActiveTool,
}

fn compact_btn(
    ui: &mut egui::Ui,
    icon: Option<UiIcon>,
    label: Option<&str>,
    a11y: &str,
    enabled: bool,
    kind: BtnKind,
    p: Palette,
) -> egui::Response {
    let font = egui::FontId::proportional(LABEL_SIZE);
    let fg = match kind {
        _ if !enabled => with_alpha(p.muted, 130),
        BtnKind::SolidAccent | BtnKind::SolidSuccess | BtnKind::ActiveTool => p.accent_fg,
        BtnKind::Danger => p.err,
        BtnKind::Subtle => p.text,
    };
    let galley = label.map(|l| ui.painter().layout_no_wrap(l.to_owned(), font, fg));
    let w = match (&icon, &galley) {
        (Some(_), Some(g)) => g.size().x + 30.0,
        (None, Some(g)) => g.size().x + 16.0,
        _ => CONTROL_H,
    };
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, CONTROL_H), sense);
    let hovered = enabled && resp.hovered();

    let (fill, border) = if !enabled {
        (with_alpha(p.panel, 140), with_alpha(p.line, 140))
    } else {
        match kind {
            BtnKind::SolidAccent => (
                if hovered {
                    lerp_color(p.accent, Color32::WHITE, 0.12)
                } else {
                    p.accent
                },
                p.accent_bright,
            ),
            BtnKind::SolidSuccess => (
                if hovered {
                    lerp_color(p.success, Color32::WHITE, 0.12)
                } else {
                    p.success
                },
                p.success,
            ),
            BtnKind::ActiveTool => (p.accent, p.accent_bright),
            BtnKind::Danger => (
                if hovered { p.err_soft } else { p.panel },
                if hovered { p.err } else { p.line },
            ),
            BtnKind::Subtle => (
                if hovered { p.hover } else { p.panel },
                if hovered { p.muted } else { p.line },
            ),
        }
    };

    ui.painter().rect_filled(rect, RADIUS_SM, fill);
    ui.painter().rect_stroke(
        rect,
        RADIUS_SM,
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );

    if let Some(ic) = icon {
        let ic_x = if galley.is_some() {
            rect.left() + 12.5
        } else {
            rect.center().x
        };
        let ic_rect =
            Rect::from_center_size(egui::pos2(ic_x, rect.center().y), egui::vec2(13.5, 13.5));
        let ic_col = if kind == BtnKind::Subtle && enabled && !hovered && galley.is_some() {
            p.muted
        } else {
            fg
        };
        draw_ui_icon(ui.painter(), ic_rect, ic, ic_col);
    }
    if let Some(g) = galley {
        let tx = if icon.is_some() {
            rect.left() + 23.0
        } else {
            rect.left() + 8.0
        };
        ui.painter()
            .galley(egui::pos2(tx, rect.center().y - g.size().y * 0.5), g, fg);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, a11y));
    if enabled && hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    focus_ring(ui, &resp, p);
    resp
}

fn compact_btn_width(ui: &egui::Ui, has_icon: bool, label: Option<&str>) -> f32 {
    let font = egui::FontId::proportional(LABEL_SIZE);
    match (has_icon, label) {
        (true, Some(l)) => {
            ui.painter()
                .layout_no_wrap(l.to_owned(), font, Color32::WHITE)
                .size()
                .x
                + 30.0
        }
        (false, Some(l)) => {
            ui.painter()
                .layout_no_wrap(l.to_owned(), font, Color32::WHITE)
                .size()
                .x
                + 16.0
        }
        _ => CONTROL_H,
    }
}

fn compact_slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    width: f32,
    a11y: &str,
    p: Palette,
) -> egui::Response {
    let (rect, mut resp) =
        ui.allocate_exact_size(egui::vec2(width, 20.0), egui::Sense::click_and_drag());
    let min_v = *range.start();
    let max_v = *range.end();
    let pad = 6.0;
    let x0 = rect.left() + pad;
    let x1 = (rect.right() - pad).max(x0 + 1.0);

    if (resp.dragged() || resp.clicked())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let t = ((pos.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
        let next = (min_v + t * (max_v - min_v)).round();
        if (next - *value).abs() > 1e-3 {
            *value = next;
            resp.mark_changed();
        }
    }

    let frac = ((*value - min_v) / (max_v - min_v)).clamp(0.0, 1.0);
    let thumb_x = egui::lerp(x0..=x1, frac);
    let track_rect = Rect::from_min_max(
        egui::pos2(x0, rect.center().y - 2.0),
        egui::pos2(x1, rect.center().y + 2.0),
    );
    ui.painter().rect_filled(track_rect, 2.0, p.panel);
    ui.painter().rect_stroke(
        track_rect,
        2.0,
        Stroke::new(1.0_f32, p.line),
        egui::StrokeKind::Inside,
    );
    let fill_rect = Rect::from_min_max(
        track_rect.min,
        egui::pos2(thumb_x.max(x0 + 2.0), track_rect.max.y),
    );
    ui.painter().rect_filled(fill_rect, 2.0, p.accent);

    let thumb_center = egui::pos2(thumb_x, rect.center().y);
    let thumb_r = if resp.hovered() || resp.dragged() {
        6.0
    } else {
        5.2
    };
    ui.painter()
        .circle_filled(thumb_center, thumb_r, Color32::WHITE);
    ui.painter()
        .circle_stroke(thumb_center, thumb_r, Stroke::new(1.5_f32, p.accent_bright));
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.widget_info(|| egui::WidgetInfo::slider(true, *value as f64, a11y));
    focus_ring(ui, &resp, p);
    resp
}

fn editor_tool_cell(
    ui: &mut egui::Ui,
    tool: Tool,
    active: bool,
    tip: &str,
    p: Palette,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::click());
    let hovered = resp.hovered();
    if active {
        ui.painter().rect_filled(rect, RADIUS_SM, p.accent);
        ui.painter().rect_stroke(
            rect,
            RADIUS_SM,
            Stroke::new(1.0_f32, p.accent_bright),
            egui::StrokeKind::Inside,
        );
    } else if hovered {
        ui.painter().rect_filled(rect, RADIUS_SM, p.hover);
        ui.painter().rect_stroke(
            rect,
            RADIUS_SM,
            Stroke::new(1.0_f32, p.line),
            egui::StrokeKind::Inside,
        );
    }
    let ink = if active {
        p.accent_fg
    } else if hovered {
        p.text
    } else {
        lerp_color(p.text, p.muted, 0.2)
    };
    super::draw_tool(&ui.painter().with_clip_rect(rect), rect, tool, ink);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, tip));
    focus_ring(ui, &resp, p);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tip)
}

fn bar_divider(ui: &mut egui::Ui, height: f32, p: Palette) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(7.0, height), egui::Sense::hover());
    let x = rect.center().x;
    ui.painter().line_segment(
        [
            egui::pos2(x, rect.top() + 5.0),
            egui::pos2(x, rect.bottom() - 5.0),
        ],
        Stroke::new(1.0_f32, p.line),
    );
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix =
        |x: u8, y: u8| -> u8 { (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8 };
    Color32::from_rgba_unmultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}

fn delete_confirm_id() -> egui::Id {
    egui::Id::new("xscreen_tile_delete_confirm")
}

pub(super) enum EditorDiscardAction {
    ReturnToAlbum,
    OpenFile(PathBuf),
    Paste,
}

fn run_editor_action(app: &mut App, action: EditorDiscardAction, ctx: &egui::Context) {
    match action {
        EditorDiscardAction::ReturnToAlbum => {
            app.page = Page::Album;
            app.album.refresh(app.config.save_folder(), ctx, false);
        }
        EditorDiscardAction::OpenFile(path) => {
            app.studio.open_file(path, ctx);
            app.page = Page::Editor;
        }
        EditorDiscardAction::Paste => {
            app.studio.paste_from_clipboard(app.config.lang, ctx);
            app.page = Page::Editor;
        }
    }
}

fn request_editor_action(app: &mut App, action: EditorDiscardAction, ctx: &egui::Context) {
    if app.editor_discard.is_some() {
        return;
    }
    app.studio.editor.commit_text();
    if app.studio.is_modified() {
        app.studio.shape_drag = None;
        app.editor_discard = Some(action);
    } else {
        run_editor_action(app, action, ctx);
    }
}

fn fresh_escape(ctx: &egui::Context) -> bool {
    // Holding Esc must not cascade through editor states or dismiss a newly opened dialog.
    ctx.input_mut(|i| {
        let mut fresh = false;
        i.events.retain(|event| {
            if let egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                repeat,
                modifiers,
                ..
            } = event
                && modifiers.matches_logically(egui::Modifiers::NONE)
            {
                fresh |= !repeat;
                false
            } else {
                true
            }
        });
        fresh
    })
}

fn render_editor_discard(
    app: &mut App,
    ctx: &egui::Context,
    p: Palette,
    s: &Strings,
    opening: bool,
) {
    let Some(action) = app.editor_discard.as_ref() else {
        return;
    };
    let message = match action {
        EditorDiscardAction::ReturnToAlbum => s.discard_to_album(),
        EditorDiscardAction::OpenFile(_) | EditorDiscardAction::Paste => s.discard_to_replace(),
    };
    let cancel_key = !opening && fresh_escape(ctx);
    let (yes, no) = egui::Modal::new(egui::Id::new("xscreen_editor_discard"))
        .frame(
            egui::Frame::NONE
                .fill(p.bar)
                .stroke(Stroke::new(1.0_f32, p.line))
                .corner_radius(CornerRadius::same(RADIUS_MD_U8))
                .inner_margin(Margin::same(16)),
        )
        .show(ctx, |ui| {
            ui.set_width(320.0);
            ui.heading(RichText::new(s.unsaved_changes()).size(15.0).color(p.text));
            ui.add_space(4.0);
            ui.label(RichText::new(message).size(BODY_SIZE).color(p.text));
            if app.studio.saving {
                ui.label(
                    RichText::new(s.saving_image())
                        .size(LABEL_SIZE)
                        .color(p.muted),
                );
            }
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let no = compact_btn(
                    ui,
                    None,
                    Some(s.no()),
                    s.no(),
                    true,
                    BtnKind::SolidAccent,
                    p,
                );
                let yes = compact_btn(
                    ui,
                    None,
                    Some(s.yes()),
                    s.yes(),
                    !app.studio.saving,
                    BtnKind::Danger,
                    p,
                );
                // egui clears focus on Esc before widgets run, even on key repeat.
                if opening || (!no.has_focus() && !yes.has_focus()) {
                    no.request_focus();
                    focus_ring(ui, &no, p);
                }
                let enter_on_no = no.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                (yes.clicked(), no.clicked() || enter_on_no)
            })
            .inner
        })
        .inner;
    if opening {
        return;
    }
    if cancel_key || no {
        app.editor_discard = None;
    } else if yes && let Some(action) = app.editor_discard.take() {
        if matches!(action, EditorDiscardAction::ReturnToAlbum) && app.studio.is_modified() {
            app.studio.discard_image();
        }
        run_editor_action(app, action, ctx);
    }
}

pub(super) fn show(app: &mut App, ctx: &egui::Context) {
    style(ctx, app.config.light_theme);
    let p = palette(app.config.light_theme);
    let s = Strings::new(app.config.lang);
    let confirmation_was_open = app.editor_discard.is_some();

    app.album.poll(ctx, &s);
    if app.page == Page::Album && app.album.entries.is_empty() && !app.album.loading {
        app.album.refresh(app.config.save_folder(), ctx, false);
    }

    let studio_out = app.studio.poll(ctx, &s);
    if let Some(inv) = studio_out.invalidate_album_path {
        app.album.invalidate_path(&inv);
    }
    if studio_out.refresh_album {
        app.album.refresh(app.config.save_folder(), ctx, true);
    }
    if let Some(notice) = studio_out.notice {
        app.album.set_notice(notice);
    }
    if app.album.active_notice().is_some() {
        ctx.request_repaint_after(std::time::Duration::from_secs(4));
    }

    // Global window Escape behavior (when not caught by Editor canvas/crop/text)
    if !confirmation_was_open
        && app.page != Page::Editor
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        if ctx.data_mut(|d| d.get_temp::<PathBuf>(delete_confirm_id()).is_some()) {
            ctx.data_mut(|d| d.remove::<PathBuf>(delete_confirm_id()));
        } else {
            app.settings = false;
        }
    }

    // Pre-sync fit zoom before status bar so initial frame zoom readout is immediately accurate
    if app.page == Page::Editor && app.studio.fit_mode && app.studio.has_image() {
        let win = ctx.content_rect();
        let approx_w = (win.width() - 56.0).max(32.0);
        let approx_h =
            (win.height() - TOP_BAR_H - STATUS_BAR_H - EDITOR_BAR_H - PROP_BAR_H - 56.0).max(32.0);
        app.studio.zoom = ((approx_w / app.studio.width as f32)
            .min(approx_h / app.studio.height as f32))
        .clamp(0.05, 1.0);
    }

    render_top_bar(app, ctx, p, &s);
    render_status_bar(app, ctx, p, &s);

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(if app.page == Page::Editor {
            p.canvas
        } else {
            p.bg
        }))
        .show(ctx, |ui| {
            let active_err = app.error.clone().or_else(|| {
                if app.page == Page::Editor {
                    app.studio.error.clone()
                } else {
                    None
                }
            });
            if let Some(error) = active_err {
                egui::Frame::NONE
                    .fill(p.err_soft)
                    .stroke(Stroke::new(1.0_f32, p.err))
                    .inner_margin(Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(error).size(LABEL_SIZE).color(p.err));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if compact_btn(
                                        ui,
                                        Some(UiIcon::Clear),
                                        Some(s.dismiss()),
                                        s.dismiss(),
                                        true,
                                        BtnKind::Subtle,
                                        p,
                                    )
                                    .clicked()
                                    {
                                        app.error = None;
                                        app.studio.error = None;
                                    }
                                },
                            );
                        });
                    });
            }

            match app.page {
                Page::Album => {
                    egui::Frame::NONE
                        .inner_margin(Margin::symmetric(12, 10))
                        .show(ui, |ui| gallery_view(app, ui, p, &s));
                }
                Page::Editor => {
                    editor_view(app, ui, p, &s);
                }
                Page::Settings => {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Frame::NONE
                                .inner_margin(Margin::symmetric(18, 14))
                                .show(ui, |ui| {
                                    let avail_w = ui.available_width();
                                    let col_w = avail_w.min(660.0);
                                    let pad_x = ((avail_w - col_w) * 0.5).max(0.0);
                                    ui.horizontal(|ui| {
                                        ui.add_space(pad_x);
                                        ui.allocate_ui_with_layout(
                                            egui::vec2(col_w, ui.available_height()),
                                            egui::Layout::top_down(egui::Align::LEFT),
                                            |ui| settings_view(app, ui, p, &s),
                                        );
                                    });
                                });
                        });
                }
            }
        });
    render_editor_discard(app, ctx, p, &s, !confirmation_was_open);
}

fn render_top_bar(app: &mut App, ctx: &egui::Context, p: Palette, s: &Strings) {
    let resp = egui::TopBottomPanel::top("top_command_bar")
        .exact_height(TOP_BAR_H)
        .frame(
            egui::Frame::NONE
                .fill(p.bar)
                .inner_margin(Margin::symmetric(10, 5)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;

                // Compact viewfinder mark + brand text
                let (mark_rect, _) =
                    ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
                draw_ui_icon(ui.painter(), mark_rect, UiIcon::Capture, p.accent_bright);
                ui.label(RichText::new("xscreen").size(13.5).strong().color(p.text));
                bar_divider(ui, CONTROL_H, p);

                // Segmented top tabs: Album · Editor · Settings
                let album_label = if app.album.entries.is_empty() {
                    s.tab_album().to_owned()
                } else {
                    format!("{} ({})", s.tab_album(), app.album.entries.len())
                };
                let editor_label = if app.studio.is_modified() {
                    format!("{} •", s.tab_editor())
                } else {
                    s.tab_editor().to_owned()
                };

                for (page, icon, text) in [
                    (Page::Album, UiIcon::Album, album_label),
                    (Page::Editor, UiIcon::Editor, editor_label),
                    (
                        Page::Settings,
                        UiIcon::Settings,
                        s.tab_settings().to_owned(),
                    ),
                ] {
                    let active = app.page == page;
                    let resp = compact_btn(
                        ui,
                        Some(icon),
                        Some(&text),
                        &text,
                        true,
                        if active {
                            BtnKind::ActiveTool
                        } else {
                            BtnKind::Subtle
                        },
                        p,
                    );
                    if resp.clicked() && app.page != page {
                        app.studio.editor.commit_text();
                        app.page = page;
                        if page == Page::Album {
                            app.album.refresh(app.config.save_folder(), ui.ctx(), false);
                        } else if page == Page::Settings {
                            app.config.run_at_startup = crate::startup::is_set();
                        }
                    }
                }

                // Right-aligned global actions first so center gets remaining space cleanly
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;

                    let cap_tip = match &app.hotkey {
                        Some(hk) => format!("{} ({})", s.new_capture_tip(), hk.label()),
                        None => s.new_capture_tip().to_owned(),
                    };
                    if compact_btn(
                        ui,
                        Some(UiIcon::Capture),
                        Some(s.new_capture()),
                        s.new_capture(),
                        true,
                        BtnKind::SolidAccent,
                        p,
                    )
                    .on_hover_text(cap_tip)
                    .clicked()
                    {
                        app.toggle(ui.ctx());
                    }

                    let theme_icon = if app.config.light_theme {
                        UiIcon::Moon
                    } else {
                        UiIcon::Sun
                    };
                    if compact_btn(
                        ui,
                        Some(theme_icon),
                        None,
                        s.switch_theme_tip(),
                        true,
                        BtnKind::Subtle,
                        p,
                    )
                    .on_hover_text(s.switch_theme_tip())
                    .clicked()
                    {
                        app.config.light_theme = !app.config.light_theme;
                        app.native.set_theme(app.config.light_theme);
                        app.persist();
                    }

                    let lang_badge = match app.config.lang {
                        Lang::En => "EN",
                        Lang::Tr => "TR",
                    };
                    if compact_btn(
                        ui,
                        None,
                        Some(lang_badge),
                        s.lbl_language(),
                        true,
                        BtnKind::Subtle,
                        p,
                    )
                    .on_hover_text("English / Türkçe")
                    .clicked()
                    {
                        app.config.lang = match app.config.lang {
                            Lang::En => Lang::Tr,
                            Lang::Tr => Lang::En,
                        };
                        app.persist();
                    }

                    // Contextual middle controls per page
                    if app.page == Page::Album {
                        bar_divider(ui, CONTROL_H, p);
                        if compact_btn(
                            ui,
                            Some(UiIcon::Folder),
                            None,
                            s.open_folder_tip(),
                            true,
                            BtnKind::Subtle,
                            p,
                        )
                        .on_hover_text(s.open_folder_tip())
                        .clicked()
                        {
                            app.native.open_folder(app.config.save_folder().as_deref());
                        }
                        if compact_btn(
                            ui,
                            Some(UiIcon::Refresh),
                            None,
                            s.refresh(),
                            true,
                            BtnKind::Subtle,
                            p,
                        )
                        .on_hover_text(s.refresh())
                        .clicked()
                        {
                            app.album.refresh(app.config.save_folder(), ui.ctx(), true);
                        }
                        let search_w = (ui.available_width() - 6.0).clamp(90.0, 220.0);
                        if search_w >= 90.0 {
                            search_input(ui, &mut app.album.query, search_w, p, s);
                        }
                    } else if app.page == Page::Editor && app.studio.has_image() {
                        bar_divider(ui, CONTROL_H, p);
                        let src_name = app
                            .studio
                            .source_path
                            .as_ref()
                            .and_then(|path| path.file_name())
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| {
                                if app.studio.from_capture {
                                    s.new_capture_source().to_owned()
                                } else {
                                    s.clipboard_source().to_owned()
                                }
                            });
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width().max(40.0), CONTROL_H),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(src_name).size(LABEL_SIZE).color(p.muted),
                                    )
                                    .truncate(),
                                );
                                if app.studio.is_modified() {
                                    ui.label(
                                        RichText::new(format!("· {}", s.unsaved_badge()))
                                            .size(CAPTION_SIZE)
                                            .color(p.accent_bright),
                                    );
                                }
                            },
                        );
                    }
                });
            });
        });

    let r = resp.response.rect;
    ctx.layer_painter(resp.response.layer_id).line_segment(
        [r.left_bottom(), r.right_bottom()],
        Stroke::new(1.0_f32, p.line),
    );
}

fn search_input(ui: &mut egui::Ui, query: &mut String, width: f32, p: Palette, s: &Strings) {
    let id = ui.make_persistent_id("album_search_focus");
    let was_focused = ui
        .ctx()
        .data_mut(|d| d.get_temp::<bool>(id))
        .unwrap_or(false);
    let border = if was_focused {
        Stroke::new(1.2_f32, p.accent_bright)
    } else {
        Stroke::new(1.0_f32, p.line)
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, CONTROL_H), egui::Sense::hover());
    ui.painter().rect_filled(rect, RADIUS_SM, p.panel);
    ui.painter()
        .rect_stroke(rect, RADIUS_SM, border, egui::StrokeKind::Inside);

    let ic_rect = Rect::from_center_size(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::vec2(12.0, 12.0),
    );
    draw_ui_icon(
        ui.painter(),
        ic_rect,
        UiIcon::Search,
        if was_focused {
            p.accent_bright
        } else {
            p.muted
        },
    );

    let clear_w = if query.is_empty() { 0.0 } else { 20.0 };
    let text_rect = Rect::from_min_max(
        egui::pos2(rect.left() + 22.0, rect.top() + 2.0),
        egui::pos2(rect.right() - 4.0 - clear_w, rect.bottom() - 2.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(text_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let te_resp = child.add_sized(
        text_rect.size(),
        egui::TextEdit::singleline(query)
            .frame(false)
            .hint_text(s.search_hint())
            .font(egui::FontId::proportional(LABEL_SIZE))
            .vertical_align(egui::Align::Center)
            .margin(Margin::symmetric(2, 1)),
    );
    let focused = te_resp.has_focus();
    ui.ctx().data_mut(|d| d.insert_temp(id, focused));

    if !query.is_empty() {
        let clear_rect = Rect::from_center_size(
            egui::pos2(rect.right() - 11.0, rect.center().y),
            egui::vec2(16.0, 16.0),
        );
        let clear_resp = ui.interact(
            clear_rect,
            ui.make_persistent_id("album_search_clear"),
            egui::Sense::click(),
        );
        if clear_resp.hovered() {
            ui.painter().rect_filled(clear_rect, 2.0, p.hover);
        }
        draw_ui_icon(
            ui.painter(),
            clear_rect.shrink(3.5),
            UiIcon::Clear,
            if clear_resp.hovered() {
                p.text
            } else {
                p.muted
            },
        );
        if clear_resp.on_hover_text(s.clear_search()).clicked() {
            query.clear();
        }
    }
}

fn render_status_bar(app: &mut App, ctx: &egui::Context, p: Palette, s: &Strings) {
    let resp = egui::TopBottomPanel::bottom("status_bar")
        .exact_height(STATUS_BAR_H)
        .frame(
            egui::Frame::NONE
                .fill(p.bar)
                .inner_margin(Margin::symmetric(10, 2)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if let Some(notice) = app.album.active_notice() {
                    let (dot, _) =
                        ui.allocate_exact_size(egui::vec2(6.0, 12.0), egui::Sense::hover());
                    ui.painter()
                        .circle_filled(dot.center(), 2.5, p.accent_bright);
                    ui.label(RichText::new(notice).size(CAPTION_SIZE).color(p.text));
                } else {
                    let hk = app
                        .hotkey
                        .map(|h| h.label())
                        .unwrap_or_else(|| s.none().to_owned());
                    ui.label(
                        RichText::new(format!(
                            "{} {}  ·  v{}  ·  {}",
                            s.active_prefix(),
                            hk,
                            env!("CARGO_PKG_VERSION"),
                            s.status_tray_hint()
                        ))
                        .size(CAPTION_SIZE)
                        .color(p.muted),
                    );
                }

                if app.page == Page::Editor && app.studio.has_image() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if status_chip(ui, s.fit_view(), app.studio.fit_mode, p).clicked() {
                            app.studio.fit_mode = true;
                            app.studio.pan = Vec2::ZERO;
                        }
                        if status_chip(ui, "+", false, p).clicked() {
                            app.studio.fit_mode = false;
                            app.studio.zoom = (app.studio.zoom * 1.25).clamp(0.10, 8.0);
                        }
                        let zoom_lbl = format!("{:.0}%", app.studio.zoom * 100.0);
                        if status_chip(
                            ui,
                            &zoom_lbl,
                            !app.studio.fit_mode && (app.studio.zoom - 1.0).abs() < 0.01,
                            p,
                        )
                        .on_hover_text("100%")
                        .clicked()
                        {
                            app.studio.fit_mode = false;
                            app.studio.zoom = 1.0;
                            app.studio.pan = Vec2::ZERO;
                        }
                        if status_chip(ui, "−", false, p).clicked() {
                            app.studio.fit_mode = false;
                            app.studio.zoom = (app.studio.zoom / 1.25).clamp(0.10, 8.0);
                        }
                        ui.label(
                            RichText::new(format!(
                                "{} × {} px",
                                app.studio.width, app.studio.height
                            ))
                            .monospace()
                            .size(CAPTION_SIZE)
                            .color(p.muted),
                        );
                        if let Some((cx, cy)) = app.studio.cursor_img_pos {
                            ui.label(
                                RichText::new(format!("({cx}, {cy})"))
                                    .monospace()
                                    .size(CAPTION_SIZE)
                                    .color(p.muted),
                            );
                        }
                    });
                }
            });
        });

    let r = resp.response.rect;
    ctx.layer_painter(resp.response.layer_id)
        .line_segment([r.left_top(), r.right_top()], Stroke::new(1.0_f32, p.line));
}

fn status_chip(ui: &mut egui::Ui, text: &str, active: bool, p: Palette) -> egui::Response {
    let font = egui::FontId::proportional(10.5);
    let fg = if active { p.accent_fg } else { p.text };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let w = (galley.size().x + 12.0).max(18.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 18.0), egui::Sense::click());
    let fill = if active {
        p.accent
    } else if resp.hovered() {
        p.hover
    } else {
        p.panel
    };
    let border = if active { p.accent_bright } else { p.line };
    ui.painter().rect_filled(rect, 2.5, fill);
    ui.painter().rect_stroke(
        rect,
        2.5,
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, fg);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

fn gallery_view(app: &mut App, ui: &mut egui::Ui, p: Palette, s: &Strings) {
    if let Some(error) = &app.album.error {
        egui::Frame::NONE
            .fill(p.err_soft)
            .stroke(Stroke::new(1.0_f32, p.err))
            .corner_radius(CornerRadius::same(RADIUS_MD_U8))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.colored_label(p.err, error);
            });
        return;
    }
    if app.album.loading && app.album.entries.is_empty() {
        ui.add_space(48.0);
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(s.loading_captures()).color(p.muted));
            });
        });
        return;
    }
    if app.album.entries.is_empty() {
        ui.add_space((ui.available_height() * 0.25).max(20.0));
        ui.vertical_centered(|ui| {
            let (mark, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
            draw_ui_icon(ui.painter(), mark, UiIcon::Capture, p.muted);
            ui.add_space(10.0);
            ui.label(
                RichText::new(s.empty_album_title())
                    .size(15.0)
                    .strong()
                    .color(p.text),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(s.empty_album_sub())
                    .size(LABEL_SIZE)
                    .color(p.muted),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let total_w = compact_btn_width(ui, true, Some(s.new_capture()))
                    + ui.spacing().item_spacing.x
                    + compact_btn_width(ui, true, Some(s.paste_short()));
                ui.add_space(((ui.available_width() - total_w) * 0.5).max(0.0));
                if compact_btn(
                    ui,
                    Some(UiIcon::Capture),
                    Some(s.new_capture()),
                    s.new_capture(),
                    true,
                    BtnKind::SolidAccent,
                    p,
                )
                .clicked()
                {
                    app.toggle(ui.ctx());
                }
                if compact_btn(
                    ui,
                    Some(UiIcon::Paste),
                    Some(s.paste_short()),
                    s.paste_clipboard(),
                    true,
                    BtnKind::Subtle,
                    p,
                )
                .clicked()
                {
                    request_editor_action(app, EditorDiscardAction::Paste, ui.ctx());
                }
            });
        });
        return;
    }

    let query_lower = app.album.query.trim().to_lowercase();
    let filtered_indices: Vec<usize> = if query_lower.is_empty() {
        (0..app.album.entries.len()).collect()
    } else {
        app.album
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.name_lower.contains(&query_lower).then_some(i))
            .collect()
    };

    if filtered_indices.is_empty() {
        ui.add_space(48.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(s.no_matches())
                    .size(SECTION_SIZE)
                    .strong()
                    .color(p.text),
            );
            ui.add_space(8.0);
            if compact_btn(
                ui,
                Some(UiIcon::Clear),
                Some(s.clear_search()),
                s.clear_search(),
                true,
                BtnKind::Subtle,
                p,
            )
            .clicked()
            {
                app.album.query.clear();
            }
        });
        return;
    }

    let gap = 8.0;
    let columns = ((ui.available_width() + gap) / 188.0).floor().max(1.0) as usize;
    let width =
        ((ui.available_width() - (columns - 1) as f32 * gap - 8.0) / columns as f32).max(120.0);
    let meta_h = 30.0;
    let well_h = (width * 0.58).round();
    let height = well_h + meta_h;
    let rows = filtered_indices.len().div_ceil(columns);

    let mut open_in_editor: Option<PathBuf> = None;
    let mut copy_path: Option<PathBuf> = None;
    let mut open_ext_path: Option<PathBuf> = None;
    let mut delete_path: Option<PathBuf> = None;

    let armed_delete = ui
        .ctx()
        .data_mut(|d| d.get_temp::<PathBuf>(delete_confirm_id()));

    ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, height, rows, |ui, range| {
            for row in range {
                ui.horizontal(|ui| {
                    for &entry_idx in filtered_indices.iter().skip(row * columns).take(columns) {
                        let entry = app.album.entries[entry_idx].clone();
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

                        let well_rect = Rect::from_min_size(
                            rect.min + egui::vec2(1.0, 1.0),
                            egui::vec2(width - 2.0, well_h - 1.0),
                        );

                        // Hover quick actions at top-right of thumbnail
                        let pointer_in_tile = ui
                            .input(|i| i.pointer.hover_pos())
                            .is_some_and(|pos| rect.contains(pos));
                        let is_armed = armed_delete.as_deref() == Some(entry.path.as_path());

                        let border_col = if response.has_focus() || pointer_in_tile {
                            p.accent_bright
                        } else {
                            p.line
                        };
                        ui.painter().rect_filled(rect, RADIUS_MD, p.bar);
                        ui.painter().rect_filled(well_rect, RADIUS_SM, p.canvas);

                        match app.album.thumbnails.get(&entry.path) {
                            Some(Ok(picture)) => {
                                let fit = picture.texture.size_vec2().max(egui::vec2(1.0, 1.0));
                                let scale = ((well_rect.width() - 6.0) / fit.x)
                                    .min((well_rect.height() - 6.0) / fit.y)
                                    .max(0.01);
                                let dest = Rect::from_center_size(well_rect.center(), fit * scale);
                                egui::Image::new(egui::load::SizedTexture::new(
                                    picture.texture.id(),
                                    dest.size(),
                                ))
                                .paint_at(ui, dest);
                            }
                            Some(Err(_)) => {
                                ui.painter().text(
                                    well_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    s.preview_unavailable(),
                                    egui::FontId::proportional(CAPTION_SIZE),
                                    p.muted,
                                );
                            }
                            None => {
                                ui.painter().text(
                                    well_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    s.loading_thumb(),
                                    egui::FontId::proportional(CAPTION_SIZE),
                                    p.muted,
                                );
                                app.album.request_thumb(&entry.path, ui.ctx());
                            }
                        }

                        // Compact single-line footer inside tile
                        let footer_rect = Rect::from_min_max(
                            egui::pos2(rect.left() + 7.0, well_rect.bottom()),
                            egui::pos2(rect.right() - 7.0, rect.bottom()),
                        );
                        let size_str = file_size(entry.bytes);
                        let size_galley = ui.painter().layout_no_wrap(
                            size_str,
                            egui::FontId::proportional(CAPTION_SIZE),
                            p.muted,
                        );
                        let size_w = size_galley.size().x;
                        ui.painter().galley(
                            egui::pos2(
                                footer_rect.right() - size_w,
                                footer_rect.center().y - size_galley.size().y * 0.5,
                            ),
                            size_galley,
                            p.muted,
                        );
                        let name_max_w = (footer_rect.width() - size_w - 8.0).max(24.0);
                        let name_galley = ui.painter().layout_no_wrap(
                            entry.name.clone(),
                            egui::FontId::proportional(LABEL_SIZE),
                            p.text,
                        );
                        ui.painter()
                            .with_clip_rect(Rect::from_min_size(
                                egui::pos2(footer_rect.left(), footer_rect.top()),
                                egui::vec2(name_max_w, footer_rect.height()),
                            ))
                            .galley(
                                egui::pos2(
                                    footer_rect.left(),
                                    footer_rect.center().y - name_galley.size().y * 0.5,
                                ),
                                name_galley,
                                p.text,
                            );

                        // Hover quick action overlay bar (Copy, Open, Delete)
                        let mut quick_clicked = false;
                        if pointer_in_tile || is_armed {
                            let btn_sz = 22.0;
                            let strip_w = btn_sz * 3.0 + 6.0;
                            let strip_rect = Rect::from_min_size(
                                egui::pos2(
                                    well_rect.right() - strip_w - 4.0,
                                    well_rect.top() + 4.0,
                                ),
                                egui::vec2(strip_w, btn_sz + 4.0),
                            );
                            ui.painter().rect_filled(
                                strip_rect,
                                RADIUS_SM,
                                Color32::from_rgba_unmultiplied(20, 22, 26, 225),
                            );
                            ui.painter().rect_stroke(
                                strip_rect,
                                RADIUS_SM,
                                Stroke::new(1.0_f32, Color32::from_white_alpha(40)),
                                egui::StrokeKind::Inside,
                            );
                            for (slot, (ic, tip, danger)) in [
                                (UiIcon::Copy, s.copy_image(), false),
                                (UiIcon::Open, s.open_external(), false),
                                (
                                    if is_armed {
                                        UiIcon::Check
                                    } else {
                                        UiIcon::Delete
                                    },
                                    if is_armed {
                                        s.confirm_delete()
                                    } else {
                                        s.delete()
                                    },
                                    true,
                                ),
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                let cell_r = Rect::from_min_size(
                                    egui::pos2(
                                        strip_rect.left() + 2.0 + slot as f32 * (btn_sz + 1.0),
                                        strip_rect.top() + 2.0,
                                    ),
                                    egui::vec2(btn_sz, btn_sz),
                                );
                                let q_resp = ui.interact(
                                    cell_r,
                                    ui.make_persistent_id(("tile_q", &entry.path, slot)),
                                    egui::Sense::click(),
                                );
                                if q_resp.hovered() || (danger && is_armed) {
                                    ui.painter().rect_filled(
                                        cell_r,
                                        2.0,
                                        if danger {
                                            Color32::from_rgb(190, 45, 45)
                                        } else {
                                            Color32::from_white_alpha(38)
                                        },
                                    );
                                }
                                draw_ui_icon(
                                    ui.painter(),
                                    cell_r.shrink(4.0),
                                    ic,
                                    Color32::from_rgb(240, 242, 245),
                                );
                                if q_resp
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .on_hover_text(tip)
                                    .clicked()
                                {
                                    quick_clicked = true;
                                    match slot {
                                        0 => copy_path = Some(entry.path.clone()),
                                        1 => open_ext_path = Some(entry.path.clone()),
                                        2 => {
                                            if is_armed {
                                                delete_path = Some(entry.path.clone());
                                            } else {
                                                ui.ctx().data_mut(|d| {
                                                    d.insert_temp(
                                                        delete_confirm_id(),
                                                        entry.path.clone(),
                                                    )
                                                });
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }

                        ui.painter().rect_stroke(
                            rect,
                            RADIUS_MD,
                            Stroke::new(
                                if response.has_focus() || response.hovered() {
                                    1.6_f32
                                } else {
                                    1.0_f32
                                },
                                border_col,
                            ),
                            egui::StrokeKind::Inside,
                        );

                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &entry.name)
                        });

                        // Context menu
                        response.context_menu(|ui| {
                            if ui.button(s.edit_image()).clicked() {
                                open_in_editor = Some(entry.path.clone());
                                ui.close();
                            }
                            if ui.button(s.copy_image()).clicked() {
                                copy_path = Some(entry.path.clone());
                                ui.close();
                            }
                            if ui.button(s.open_external()).clicked() {
                                open_ext_path = Some(entry.path.clone());
                                ui.close();
                            }
                            ui.separator();
                            if ui
                                .add(egui::Button::new(RichText::new(s.delete()).color(p.err)))
                                .clicked()
                            {
                                delete_path = Some(entry.path.clone());
                                ui.close();
                            }
                        });

                        let dims_label = match app.album.thumbnails.get(&entry.path) {
                            Some(Ok(pic)) => format!(" · {}×{}", pic.size[0], pic.size[1]),
                            _ => String::new(),
                        };
                        let hover_tip = format!(
                            "{}{} · {} · {}",
                            entry.name,
                            dims_label,
                            format_modified(entry.modified, s),
                            s.edit_image()
                        );
                        if !quick_clicked && response.on_hover_text(hover_tip).clicked() {
                            open_in_editor = Some(entry.path.clone());
                        }
                    }
                });
            }
        });

    if let Some(path) = copy_path {
        app.album.copy(path, app.config.copy_png, ui.ctx());
    }
    if let Some(path) = open_ext_path {
        app.native.open_path(&path);
    }
    if let Some(path) = delete_path {
        ui.ctx()
            .data_mut(|d| d.remove::<PathBuf>(delete_confirm_id()));
        match std::fs::remove_file(&path) {
            Ok(()) => {
                app.album.invalidate_path(&path);
                app.album.set_notice(s.deleted_notice());
                app.album.refresh(app.config.save_folder(), ui.ctx(), true);
            }
            Err(e) => app.error = Some(s.err_delete_file(&e)),
        }
    }
    if let Some(path) = open_in_editor {
        ui.ctx()
            .data_mut(|d| d.remove::<PathBuf>(delete_confirm_id()));
        request_editor_action(app, EditorDiscardAction::OpenFile(path), ui.ctx());
    }
}

fn editor_view(app: &mut App, ui: &mut egui::Ui, p: Palette, s: &Strings) {
    let ctx = ui.ctx().clone();
    let text_editing = app.studio.editor.text_at.is_some();
    if app.editor_discard.is_none() && text_editing && fresh_escape(&ctx) {
        app.studio.editor.commit_text();
    }

    // Editor keyboard shortcuts (active whenever not typing inside a text box or popup)
    if app.editor_discard.is_none()
        && !app.studio.loading
        && !text_editing
        && !egui::Popup::is_any_open(&ctx)
    {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::V)) {
            request_editor_action(app, EditorDiscardAction::Paste, &ctx);
        }
        if app.editor_discard.is_none() && !app.studio.loading && app.studio.has_image() {
            if ctx.input_mut(|i| {
                i.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::S,
                )
            }) {
                app.studio
                    .save_image(true, app.config.save_folder(), app.config.lang, &ctx);
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
                app.studio
                    .save_image(false, app.config.save_folder(), app.config.lang, &ctx);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::C)) {
                app.studio
                    .copy_to_clipboard(app.config.copy_png, app.config.lang, &ctx);
            }
            if ctx.input_mut(|i| {
                i.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                ) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
            }) {
                app.studio.redo(&ctx);
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
                app.studio.undo(&ctx);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::D))
                && app.studio.editor.duplicate_selected()
            {
                app.studio.sync_blurs(&ctx, true);
            }
            if ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
            }) && app.studio.editor.delete_selected()
            {
                app.studio.sync_blurs(&ctx, true);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
                && app.studio.crop_mode
                && app.studio.crop_rect.is_some()
            {
                app.studio.apply_crop(&ctx);
            }
            if fresh_escape(&ctx) {
                if app.studio.crop_mode {
                    app.studio.crop_mode = false;
                    app.studio.crop_rect = None;
                    app.studio.crop_drag = None;
                } else if app.studio.editor.draft.is_some() {
                    app.studio.editor.cancel_draft();
                } else if app.studio.is_modified() {
                    request_editor_action(app, EditorDiscardAction::ReturnToAlbum, &ctx);
                } else if app.studio.editor.selected.is_some() {
                    app.studio.editor.select_shape_index(None);
                } else if app.studio.editor.tool != Tool::Select {
                    app.studio.editor.select_tool(Tool::Select);
                } else {
                    run_editor_action(app, EditorDiscardAction::ReturnToAlbum, &ctx);
                }
            }

            // Single-key tool hotkeys (ShareX / standard graphics editor shortcuts)
            for (key, tool) in [
                (egui::Key::V, Some(Tool::Select)),
                (egui::Key::A, Some(Tool::Arrow)),
                (egui::Key::L, Some(Tool::Line)),
                (egui::Key::R, Some(Tool::Rect)),
                (egui::Key::E, Some(Tool::Ellipse)),
                (egui::Key::P, Some(Tool::Pen)),
                (egui::Key::H, Some(Tool::Marker)),
                (egui::Key::T, Some(Tool::Text)),
                (egui::Key::N, Some(Tool::Number)),
                (egui::Key::B, Some(Tool::Blur)),
            ] {
                if app.editor_discard.is_none()
                    && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key))
                    && let Some(t) = tool
                {
                    app.studio.crop_mode = false;
                    app.studio.crop_rect = None;
                    app.studio.editor.select_tool(t);
                }
            }
            if app.editor_discard.is_none()
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::C))
            {
                app.studio.editor.select_shape_index(None);
                app.studio.crop_mode = !app.studio.crop_mode;
                app.studio.crop_rect = None;
            }
        }
    }

    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
    // ShareX-style Toolbar Row 1 (Tools & File Actions) — exact fixed height, never vertically stretched!
    let row1 = egui::Frame::NONE
        .fill(p.bar)
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), EDITOR_BAR_H - 8.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    let has_img = app.studio.has_image() && !app.studio.loading;

                    for tool in [
                        Tool::Select,
                        Tool::Rect,
                        Tool::Ellipse,
                        Tool::Triangle,
                        Tool::Line,
                        Tool::Arrow,
                        Tool::Pen,
                        Tool::Marker,
                        Tool::Text,
                        Tool::Number,
                        Tool::Blur,
                    ] {
                        let active =
                            has_img && !app.studio.crop_mode && app.studio.editor.tool == tool;
                        let resp = editor_tool_cell(ui, tool, active, tool.label_i18n(s), p);
                        if resp.clicked() && has_img {
                            app.studio.editor.commit_text();
                            app.studio.crop_mode = false;
                            app.studio.crop_rect = None;
                            app.studio.editor.select_tool(tool);
                        }
                    }

                    bar_divider(ui, 26.0, p);

                    // Crop button
                    if compact_btn(
                        ui,
                        Some(UiIcon::Crop),
                        None,
                        s.tool_crop(),
                        has_img,
                        if app.studio.crop_mode {
                            BtnKind::ActiveTool
                        } else {
                            BtnKind::Subtle
                        },
                        p,
                    )
                    .on_hover_text(s.tool_crop())
                    .clicked()
                    {
                        app.studio.editor.commit_text();
                        app.studio.editor.select_shape_index(None);
                        app.studio.crop_mode = !app.studio.crop_mode;
                        app.studio.crop_rect = None;
                    }

                    bar_divider(ui, 26.0, p);

                    // Undo / Redo / Clear / Paste
                    if compact_btn(
                        ui,
                        Some(UiIcon::Undo),
                        None,
                        s.undo_tip(),
                        has_img && app.studio.can_undo(),
                        BtnKind::Subtle,
                        p,
                    )
                    .on_hover_text(s.undo_tip())
                    .clicked()
                    {
                        app.studio.undo(&ctx);
                    }
                    if compact_btn(
                        ui,
                        Some(UiIcon::Redo),
                        None,
                        s.redo_tip(),
                        has_img && app.studio.can_redo(),
                        BtnKind::Subtle,
                        p,
                    )
                    .on_hover_text(s.redo_tip())
                    .clicked()
                    {
                        app.studio.redo(&ctx);
                    }
                    if compact_btn(
                        ui,
                        Some(UiIcon::Delete),
                        None,
                        s.clear_annotations_tip(),
                        has_img && !app.studio.editor.shapes.is_empty(),
                        BtnKind::Subtle,
                        p,
                    )
                    .on_hover_text(s.clear_annotations_tip())
                    .clicked()
                    {
                        app.studio.editor.commit_text();
                        app.studio.editor.clear_all();
                        app.studio.sync_blurs(&ctx, true);
                    }
                    if compact_btn(
                        ui,
                        Some(UiIcon::Paste),
                        None,
                        s.paste_clipboard(),
                        !app.studio.loading,
                        BtnKind::Subtle,
                        p,
                    )
                    .on_hover_text(s.paste_clipboard())
                    .clicked()
                    {
                        request_editor_action(app, EditorDiscardAction::Paste, &ctx);
                    }

                    // Right-aligned Copy / Save / Save Copy (collapses to icon-only below 780px so 680px fits cleanly)
                    let compact_row1 = ui.max_rect().width() < 780.0;
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        if compact_btn(
                            ui,
                            Some(UiIcon::SaveCopy),
                            (!compact_row1).then_some(s.save_copy()),
                            s.save_copy(),
                            has_img && !app.studio.saving,
                            BtnKind::Subtle,
                            p,
                        )
                        .on_hover_text(s.save_copy_tip())
                        .clicked()
                        {
                            app.studio.save_image(
                                true,
                                app.config.save_folder(),
                                app.config.lang,
                                &ctx,
                            );
                        }
                        if compact_btn(
                            ui,
                            Some(UiIcon::Save),
                            (!compact_row1).then_some(s.save()),
                            s.save(),
                            has_img && !app.studio.saving,
                            BtnKind::SolidSuccess,
                            p,
                        )
                        .on_hover_text(s.save_tip())
                        .clicked()
                        {
                            app.studio.save_image(
                                false,
                                app.config.save_folder(),
                                app.config.lang,
                                &ctx,
                            );
                        }
                        if compact_btn(
                            ui,
                            Some(UiIcon::Copy),
                            (!compact_row1).then_some(s.copy_image()),
                            s.copy_image(),
                            has_img,
                            BtnKind::Subtle,
                            p,
                        )
                        .on_hover_text(s.copy_tip())
                        .clicked()
                        {
                            app.studio.copy_to_clipboard(
                                app.config.copy_png,
                                app.config.lang,
                                &ctx,
                            );
                        }
                    });
                },
            );
        });
    ui.painter().line_segment(
        [
            row1.response.rect.left_bottom(),
            row1.response.rect.right_bottom(),
        ],
        Stroke::new(1.0_f32, p.line),
    );

    if app.studio.loading {
        ui.centered_and_justified(|ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(s.loading_thumb()).color(p.muted));
            });
        });
        return;
    }

    if !app.studio.has_image() {
        ui.add_space((ui.available_height() * 0.25).max(20.0));
        ui.vertical_centered(|ui| {
            let (ic_r, _) = ui.allocate_exact_size(egui::vec2(38.0, 38.0), egui::Sense::hover());
            draw_ui_icon(ui.painter(), ic_r, UiIcon::Editor, p.muted);
            ui.add_space(10.0);
            ui.label(
                RichText::new(s.editor_empty_title())
                    .size(15.0)
                    .strong()
                    .color(p.text),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(s.editor_empty_sub())
                    .size(LABEL_SIZE)
                    .color(p.muted),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let row_w = compact_btn_width(ui, true, Some(s.browse_album()))
                    + ui.spacing().item_spacing.x
                    + compact_btn_width(ui, true, Some(s.paste_clipboard()));
                ui.add_space(((ui.available_width() - row_w) * 0.5).max(0.0));
                if compact_btn(
                    ui,
                    Some(UiIcon::Album),
                    Some(s.browse_album()),
                    s.browse_album(),
                    true,
                    BtnKind::SolidAccent,
                    p,
                )
                .clicked()
                {
                    app.page = Page::Album;
                }
                if compact_btn(
                    ui,
                    Some(UiIcon::Paste),
                    Some(s.paste_clipboard()),
                    s.paste_clipboard(),
                    true,
                    BtnKind::Subtle,
                    p,
                )
                .clicked()
                {
                    request_editor_action(app, EditorDiscardAction::Paste, &ctx);
                }
            });
        });
        return;
    }

    // ShareX-style Contextual Sub-Bar Row 2 (adapts to Crop, Selected Shape, or Active Tool) — exact fixed height!
    let row2 = egui::Frame::NONE
        .fill(lerp_color(p.bar, p.bg, 0.35))
        .inner_margin(Margin::symmetric(10, 3))
        .show(ui, |ui| {
            let full_w = ui.available_width();
            let compact_row2 = full_w < 880.0;
            ui.set_min_width(full_w);
            ui.allocate_ui_with_layout(
                egui::vec2(full_w, PROP_BAR_H - 6.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = if compact_row2 { 4.0 } else { 6.0 };

                    if app.studio.crop_mode {
                        if let Some(cr) = app.studio.crop_rect {
                            ui.label(
                                RichText::new(format!(
                                    "{}  {} × {} px",
                                    s.tool_crop(),
                                    cr.width,
                                    cr.height
                                ))
                                .monospace()
                                .size(LABEL_SIZE)
                                .color(p.text),
                            );
                            if compact_btn(
                                ui,
                                Some(UiIcon::Check),
                                Some(s.crop_apply()),
                                s.crop_apply(),
                                cr.width >= 4 && cr.height >= 4,
                                BtnKind::SolidSuccess,
                                p,
                            )
                            .clicked()
                            {
                                app.studio.apply_crop(&ctx);
                            }
                        } else {
                            ui.label(RichText::new(s.crop_hint()).size(LABEL_SIZE).color(p.muted));
                        }
                        if compact_btn(
                            ui,
                            Some(UiIcon::Clear),
                            Some(s.cancel()),
                            s.cancel(),
                            true,
                            BtnKind::Subtle,
                            p,
                        )
                        .clicked()
                        {
                            app.studio.crop_mode = false;
                            app.studio.crop_rect = None;
                            app.studio.crop_drag = None;
                        }
                        return;
                    }

                    let sel_shape = app
                        .studio
                        .editor
                        .selected
                        .and_then(|i| app.studio.editor.shapes.get(i).cloned());

                    // Context indicator badge
                    if let Some(sh) = &sel_shape {
                        let ctx_text = if compact_row2 {
                            sh.kind_name(s).to_owned()
                        } else {
                            format!("{} {}", s.prop_selected(), sh.kind_name(s))
                        };
                        ui.label(
                            RichText::new(ctx_text)
                                .size(LABEL_SIZE)
                                .strong()
                                .color(p.accent_bright),
                        );
                    } else {
                        ui.label(
                            RichText::new(app.studio.editor.tool.label_i18n(s))
                                .size(LABEL_SIZE)
                                .strong()
                                .color(p.text),
                        );
                    }

                    bar_divider(ui, 22.0, p);

                    // Determine which contextual controls apply
                    let show_color = match &sel_shape {
                        Some(sh) => sh.color().is_some(),
                        None => !matches!(app.studio.editor.tool, Tool::Select | Tool::Blur),
                    };
                    let show_width = match &sel_shape {
                        Some(sh) => sh.stroke_width().is_some(),
                        None => matches!(
                            app.studio.editor.tool,
                            Tool::Arrow
                                | Tool::Line
                                | Tool::Rect
                                | Tool::Ellipse
                                | Tool::Triangle
                                | Tool::Pen
                                | Tool::Text
                        ),
                    };
                    let is_text_ctx = match &sel_shape {
                        Some(Shape::Text { .. }) => true,
                        Some(_) => false,
                        None => app.studio.editor.tool == Tool::Text,
                    };
                    let show_fill = match &sel_shape {
                        Some(sh) => sh.filled().is_some(),
                        None => app.studio.editor.tool.is_shape(),
                    };
                    let show_blur = match &sel_shape {
                        Some(Shape::Blur { .. }) => true,
                        Some(_) => false,
                        None => app.studio.editor.tool == Tool::Blur,
                    };

                    if show_color {
                        // Full color popover button + quick swatches
                        let cap_pal = super::capture_palette(app.config.light_theme);
                        let mut picked_col = app.studio.editor.color;
                        super::color_chip(
                            ui,
                            &mut picked_col,
                            &mut app.studio.editor.custom,
                            &mut app.studio.editor.custom_open,
                            s.color_tip(),
                            cap_pal,
                        );
                        if picked_col != app.studio.editor.color {
                            app.studio.editor.apply_color(picked_col);
                        }
                        let sw_count = if full_w < 820.0 { 4 } else { 8 };
                        let unselected_edge = if p.light {
                            Color32::from_black_alpha(70)
                        } else {
                            Color32::from_white_alpha(70)
                        };
                        for sw in &edit::PALETTE[..sw_count] {
                            let (r, resp) = ui
                                .allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                            let selected_sw = app.studio.editor.color.r == sw.r
                                && app.studio.editor.color.g == sw.g
                                && app.studio.editor.color.b == sw.b;
                            ui.painter()
                                .rect_filled(r, 3.0, Color32::from_rgb(sw.r, sw.g, sw.b));
                            ui.painter().rect_stroke(
                                r,
                                3.0,
                                Stroke::new(
                                    if selected_sw { 1.8_f32 } else { 1.0_f32 },
                                    if selected_sw {
                                        p.accent_bright
                                    } else {
                                        unselected_edge
                                    },
                                ),
                                egui::StrokeKind::Inside,
                            );
                            if resp
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                app.studio.editor.break_coalesce();
                                app.studio.editor.apply_color(*sw);
                            }
                        }
                    }

                    if show_width {
                        if show_color {
                            bar_divider(ui, 22.0, p);
                        }
                        let size_lbl = if is_text_ctx {
                            s.prop_text_size()
                        } else {
                            s.prop_stroke()
                        };
                        if !compact_row2 {
                            ui.label(RichText::new(size_lbl).size(CAPTION_SIZE).color(p.muted));
                        }
                        let mut w = app.studio.editor.width;
                        let sl = compact_slider(
                            ui,
                            &mut w,
                            1.0..=24.0,
                            if compact_row2 { 64.0 } else { 88.0 },
                            size_lbl,
                            p,
                        );
                        if sl.changed() {
                            app.studio.editor.set_width(w);
                        }
                        if sl.drag_stopped() {
                            app.studio.editor.break_coalesce();
                        }
                        let unit_val = if is_text_ctx {
                            format!("{:.0} px", app.studio.editor.text_size())
                        } else {
                            format!("{:.0} px", app.studio.editor.width)
                        };
                        ui.label(
                            RichText::new(unit_val)
                                .monospace()
                                .size(CAPTION_SIZE)
                                .color(p.text),
                        );
                    }

                    if show_fill {
                        bar_divider(ui, 22.0, p);
                        let cur_filled = app.studio.editor.filled;
                        for (val, title) in [(false, s.prop_outline()), (true, s.prop_fill())] {
                            if compact_btn(
                                ui,
                                None,
                                Some(title),
                                title,
                                true,
                                if cur_filled == val {
                                    BtnKind::ActiveTool
                                } else {
                                    BtnKind::Subtle
                                },
                                p,
                            )
                            .clicked()
                                && cur_filled != val
                            {
                                app.studio.editor.break_coalesce();
                                app.studio.editor.apply_filled(val);
                            }
                        }
                    }

                    if show_blur {
                        let cur_blur = sel_shape
                            .as_ref()
                            .and_then(|sh| sh.blur_kind())
                            .unwrap_or(app.studio.editor.blur);
                        for kind in [BlurKind::Soft, BlurKind::Mosaic, BlurKind::Heavy] {
                            let lbl = kind.label_i18n(s);
                            if compact_btn(
                                ui,
                                None,
                                Some(lbl),
                                lbl,
                                true,
                                if cur_blur == kind {
                                    BtnKind::ActiveTool
                                } else {
                                    BtnKind::Subtle
                                },
                                p,
                            )
                            .clicked()
                                && cur_blur != kind
                            {
                                app.studio.editor.break_coalesce();
                                app.studio.editor.select_blur(kind);
                                app.studio.sync_blurs(&ctx, true);
                            }
                        }
                    }

                    if sel_shape.is_none() && app.studio.editor.tool == Tool::Select {
                        ui.label(
                            RichText::new(s.prop_select_hint())
                                .size(CAPTION_SIZE)
                                .color(p.muted),
                        );
                    }

                    // Right-side Selected Item quick actions (icon-only below 880px so 680px never clips)
                    if sel_shape.is_some() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            if compact_btn(
                                ui,
                                Some(UiIcon::Delete),
                                (!compact_row2).then_some(s.prop_delete_item()),
                                s.prop_delete_item(),
                                true,
                                BtnKind::Danger,
                                p,
                            )
                            .on_hover_text(s.prop_delete_item())
                            .clicked()
                                && app.studio.editor.delete_selected()
                            {
                                app.studio.sync_blurs(&ctx, true);
                            }
                            if compact_btn(
                                ui,
                                Some(UiIcon::Front),
                                (!compact_row2).then_some(s.prop_front()),
                                s.prop_front(),
                                true,
                                BtnKind::Subtle,
                                p,
                            )
                            .on_hover_text(s.prop_front())
                            .clicked()
                                && app.studio.editor.bring_selected_to_front()
                            {
                                app.studio.sync_blurs(&ctx, true);
                            }
                            if compact_btn(
                                ui,
                                Some(UiIcon::Duplicate),
                                (!compact_row2).then_some(s.prop_duplicate()),
                                s.prop_duplicate(),
                                true,
                                BtnKind::Subtle,
                                p,
                            )
                            .on_hover_text(s.prop_duplicate())
                            .clicked()
                                && app.studio.editor.duplicate_selected()
                            {
                                app.studio.sync_blurs(&ctx, true);
                            }
                        });
                    }
                },
            );
        });
    ui.painter().line_segment(
        [
            row2.response.rect.left_bottom(),
            row2.response.rect.right_bottom(),
        ],
        Stroke::new(1.0_f32, p.line),
    );

    // Interactive Zoomable & Pannable Canvas
    render_editor_canvas(app, ui, p, s);
}

fn render_editor_canvas(app: &mut App, ui: &mut egui::Ui, p: Palette, s: &Strings) {
    let ctx = ui.ctx().clone();
    let avail = ui.available_size().max(egui::vec2(64.0, 64.0));
    let (canvas_rect, canvas_resp) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());

    let iw = app.studio.width as f32;
    let ih = app.studio.height as f32;
    let pad = 28.0;
    let fit_zoom = (((canvas_rect.width() - pad * 2.0).max(32.0) / iw)
        .min((canvas_rect.height() - pad * 2.0).max(32.0) / ih))
    .clamp(0.05, 1.0);

    if app.studio.fit_mode {
        app.studio.zoom = fit_zoom;
        app.studio.pan = Vec2::ZERO;
    }

    let blocked = app.editor_discard.is_some() || egui::Popup::is_any_open(&ctx);
    let pointer_pos = ui.input(|i| i.pointer.interact_pos());
    let in_canvas = pointer_pos.is_some_and(|pos| canvas_rect.contains(pos)) && !blocked;

    // Zoom with Scroll wheel around pointer
    let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
    if in_canvas && scroll_y != 0.0 && app.studio.editor.text_at.is_none() {
        let factor = if scroll_y > 0.0 { 1.15 } else { 1.0 / 1.15 };
        let old_zoom = app.studio.zoom;
        let new_zoom = (old_zoom * factor).clamp(0.10, 8.0);
        if (new_zoom - old_zoom).abs() > 1e-4 {
            if let Some(cursor) = pointer_pos {
                let rel = cursor - (canvas_rect.center() + app.studio.pan);
                app.studio.pan += rel * (1.0 - new_zoom / old_zoom);
            }
            app.studio.zoom = new_zoom;
            app.studio.fit_mode = false;
        }
    }

    // Pan with Middle Mouse button or Space + Drag
    let space_down =
        ui.input(|i| i.key_down(egui::Key::Space)) && app.studio.editor.text_at.is_none();
    let middle_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle));
    let panning =
        in_canvas && (middle_down || (space_down && ui.input(|i| i.pointer.primary_down())));
    if panning {
        let delta = ui.input(|i| i.pointer.delta());
        app.studio.pan += delta;
        app.studio.fit_mode = false;
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if in_canvas && space_down {
        ctx.set_cursor_icon(egui::CursorIcon::Grab);
    }

    let zoom = app.studio.zoom;
    app.studio.sync_zoom_filter(&ctx);
    let img_size = egui::vec2(iw * zoom, ih * zoom);
    let img_rect = Rect::from_center_size(canvas_rect.center() + app.studio.pan, img_size);

    let to_img = |pos: Pos2| -> (i32, i32) {
        (
            ((pos.x - img_rect.min.x) / zoom).round() as i32,
            ((pos.y - img_rect.min.y) / zoom).round() as i32,
        )
    };
    let to_img_clamped = |pos: Pos2| -> (i32, i32) {
        let (x, y) = to_img(pos);
        (
            x.clamp(0, app.studio.width as i32),
            y.clamp(0, app.studio.height as i32),
        )
    };
    let to_screen = |pt: (i32, i32)| -> Pos2 {
        Pos2::new(
            img_rect.min.x + pt.0 as f32 * zoom,
            img_rect.min.y + pt.1 as f32 * zoom,
        )
    };

    app.studio.cursor_img_pos = pointer_pos.filter(|p| img_rect.contains(*p)).map(to_img);

    let primary_pressed = ui.input(|i| i.pointer.primary_pressed());
    let primary_down = ui.input(|i| i.pointer.primary_down());
    let primary_released = ui.input(|i| i.pointer.primary_released());
    let secondary_clicked = canvas_resp.secondary_clicked();
    let double_clicked = ui.input(|i| {
        i.pointer
            .button_double_clicked(egui::PointerButton::Primary)
    });

    let handle_tol_px = (7.5 / zoom).ceil().max(4.0) as i32;
    let hit_tol_px = (6.5 / zoom).max(3.0);
    let text_layout = edit::TextLayout {
        ctx: &ctx,
        px_to_ui: zoom,
    };

    let mut hovered_shape_handle = None;

    // Handle interaction when not panning
    if in_canvas
        && !panning
        && !space_down
        && let Some(pos) = pointer_pos
    {
        let pt = to_img(pos);
        let pt_clamped = to_img_clamped(pos);

        if app.studio.crop_mode {
            let hovered_crop_handle = app
                .studio
                .crop_rect
                .and_then(|cr| hit_crop_handle(cr, pt, handle_tol_px, text_layout));
            let inside_crop = app.studio.crop_rect.is_some_and(|cr| {
                pt.0 >= cr.x
                    && pt.0 <= cr.x + cr.width as i32
                    && pt.1 >= cr.y
                    && pt.1 <= cr.y + cr.height as i32
            });

            if let Some(ch) = hovered_crop_handle {
                ctx.set_cursor_icon(studio::cursor_for_shape_handle(ch));
            } else if inside_crop {
                ctx.set_cursor_icon(egui::CursorIcon::Move);
            } else {
                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
            }

            if primary_pressed {
                if let (Some(ch), Some(origin)) = (hovered_crop_handle, app.studio.crop_rect) {
                    app.studio.crop_drag = Some(CropDrag::Resizing {
                        handle: ch,
                        start_mouse: pt_clamped,
                        origin,
                    });
                } else if inside_crop && let Some(origin) = app.studio.crop_rect {
                    app.studio.crop_drag = Some(CropDrag::Moving {
                        start_mouse: pt_clamped,
                        origin,
                    });
                } else {
                    app.studio.crop_drag = Some(CropDrag::Creating { start: pt_clamped });
                    app.studio.crop_rect = None;
                }
            }
        } else {
            // Right-click anywhere on an annotation immediately selects it and switches to Select!
            if secondary_clicked
                && let Some(hit_idx) =
                    edit::hit_shape(&app.studio.editor.shapes, pt, hit_tol_px, text_layout)
            {
                app.studio.editor.commit_text();
                app.studio.editor.select_tool(Tool::Select);
                app.studio.editor.select_shape_index(Some(hit_idx));
            }

            hovered_shape_handle = app
                .studio
                .editor
                .selected
                .and_then(|idx| app.studio.editor.shapes.get(idx))
                .and_then(|sh| edit::hit_shape_handle(sh, pt, handle_tol_px, text_layout));

            let hovered_shape =
                edit::hit_shape(&app.studio.editor.shapes, pt, hit_tol_px, text_layout);

            // Cursor feedback
            if let Some(sh_h) = hovered_shape_handle {
                ctx.set_cursor_icon(studio::cursor_for_shape_handle(sh_h));
            } else if app.studio.editor.tool == Tool::Select {
                if hovered_shape.is_some() || app.studio.shape_drag.is_some() {
                    ctx.set_cursor_icon(egui::CursorIcon::Move);
                } else {
                    ctx.set_cursor_icon(egui::CursorIcon::Default);
                }
            } else if app.studio.editor.tool == Tool::Text {
                if edit::hit_text(&app.studio.editor.shapes, pt, text_layout).is_some() {
                    ctx.set_cursor_icon(egui::CursorIcon::Move);
                } else {
                    ctx.set_cursor_icon(egui::CursorIcon::Text);
                }
            } else if img_rect.contains(pos) {
                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
            }

            // Primary press interaction (only if not clicking inside active TextEdit)
            let over_text_input = app.studio.editor.text_at.is_some()
                && ctx
                    .data_mut(|d| d.get_temp::<Rect>(egui::Id::new("studio_text_edit_rect")))
                    .is_some_and(|r| r.expand(4.0).contains(pos));

            if primary_pressed && !over_text_input {
                if let (Some(handle), Some(sel_idx)) =
                    (hovered_shape_handle, app.studio.editor.selected)
                {
                    app.studio.editor.commit_text();
                    let origin = app.studio.editor.shapes[sel_idx].clone();
                    app.studio.shape_drag = Some(ShapeDrag::Resizing {
                        index: sel_idx,
                        handle,
                        start_mouse: pt,
                        origin,
                        moved: false,
                    });
                } else {
                    match app.studio.editor.tool {
                        Tool::Select => {
                            app.studio.editor.commit_text();
                            if double_clicked
                                && let Some(idx) = hovered_shape
                                && matches!(
                                    app.studio.editor.shapes.get(idx),
                                    Some(Shape::Text { .. })
                                )
                            {
                                app.studio.editor.begin_edit_existing_text(idx);
                            } else if let Some(idx) = hovered_shape {
                                app.studio.editor.select_shape_index(Some(idx));
                                app.studio.shape_drag = Some(ShapeDrag::Moving {
                                    index: idx,
                                    last_mouse: pt,
                                    moved: false,
                                });
                            } else {
                                app.studio.editor.select_shape_index(None);
                            }
                        }
                        Tool::Text => {
                            let had_open_text = app.studio.editor.text_at.is_some();
                            app.studio.editor.commit_text();
                            if let Some(idx) =
                                edit::hit_text(&app.studio.editor.shapes, pt, text_layout)
                            {
                                if double_clicked {
                                    app.studio.editor.begin_edit_existing_text(idx);
                                } else {
                                    app.studio.editor.select_shape_index(Some(idx));
                                    app.studio.shape_drag = Some(ShapeDrag::Moving {
                                        index: idx,
                                        last_mouse: pt,
                                        moved: false,
                                    });
                                }
                            } else if !had_open_text && img_rect.contains(pos) {
                                app.studio.editor.begin_text(pt_clamped);
                            }
                        }
                        Tool::Number => {
                            app.studio.editor.commit_text();
                            if img_rect.contains(pos) {
                                app.studio.editor.place_number(pt_clamped);
                            }
                        }
                        tool if tool.is_drag() => {
                            app.studio.editor.commit_text();
                            if img_rect.contains(pos) {
                                app.studio.editor.begin(pt_clamped);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Ongoing drag updates
    if app.editor_discard.is_none()
        && primary_down
        && let Some(pos) = pointer_pos
    {
        let pt = to_img(pos);
        let pt_clamped = to_img_clamped(pos);

        if let Some(cd) = app.studio.crop_drag.clone() {
            let max_w = app.studio.width as i32;
            let max_h = app.studio.height as i32;
            match cd {
                CropDrag::Creating { start } => {
                    app.studio.crop_rect = Some(edit::norm_rect(start, pt_clamped));
                }
                CropDrag::Moving {
                    start_mouse,
                    origin,
                } => {
                    let dx = pt_clamped.0 - start_mouse.0;
                    let dy = pt_clamped.1 - start_mouse.1;
                    let nx = (origin.x + dx).clamp(0, (max_w - origin.width as i32).max(0));
                    let ny = (origin.y + dy).clamp(0, (max_h - origin.height as i32).max(0));
                    app.studio.crop_rect = Some(Region {
                        x: nx,
                        y: ny,
                        width: origin.width,
                        height: origin.height,
                    });
                }
                CropDrag::Resizing {
                    handle,
                    start_mouse,
                    origin,
                } => {
                    let dummy = Shape::Rect {
                        a: (origin.x, origin.y),
                        b: (
                            origin.x + origin.width as i32,
                            origin.y + origin.height as i32,
                        ),
                        color: Rgba::rgb(0, 0, 0),
                        width: 1.0,
                        filled: false,
                    };
                    if let Shape::Rect { a, b, .. } = edit::resize_shape_from_origin(
                        &dummy,
                        handle,
                        start_mouse,
                        pt_clamped,
                        text_layout,
                    ) {
                        let ca = (a.0.clamp(0, max_w), a.1.clamp(0, max_h));
                        let cb = (b.0.clamp(0, max_w), b.1.clamp(0, max_h));
                        app.studio.crop_rect = Some(edit::norm_rect(ca, cb));
                    }
                }
            }
        } else if let Some(sd) = &mut app.studio.shape_drag {
            match sd {
                ShapeDrag::Moving {
                    index,
                    last_mouse,
                    moved,
                } => {
                    let dx = pt.0 - last_mouse.0;
                    let dy = pt.1 - last_mouse.1;
                    if dx != 0 || dy != 0 {
                        if !*moved {
                            app.studio.editor.record_snapshot();
                            *moved = true;
                        }
                        if let Some(sh) = app.studio.editor.shapes.get_mut(*index) {
                            edit::translate_shape(sh, dx, dy);
                            *last_mouse = pt;
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
                    if pt != *start_mouse {
                        if !*moved {
                            app.studio.editor.record_snapshot();
                            *moved = true;
                        }
                        let updated = edit::resize_shape_from_origin(
                            origin,
                            *handle,
                            *start_mouse,
                            pt,
                            text_layout,
                        );
                        if let Some(sh) = app.studio.editor.shapes.get_mut(*index) {
                            *sh = updated;
                        }
                    }
                }
            }
        } else if app.studio.editor.draft.is_some() {
            app.studio.editor.extend(pt_clamped);
        }
    }

    if app.editor_discard.is_none() && primary_released {
        app.studio.crop_drag = None;
        if app.studio.shape_drag.take().is_some() {
            app.studio.editor.break_coalesce();
        }
        if app.studio.editor.draft.is_some() {
            app.studio.editor.commit();
        }
    }

    app.studio.sync_blurs(&ctx, false);

    // Paint workspace, checkerboard, base image, annotations, selection handles, and crop overlay
    let painter = ui.painter().with_clip_rect(canvas_rect);
    painter.rect_filled(canvas_rect, 0.0, p.canvas);

    // Subtle drop shadow + 1px crisp border around the image canvas
    painter.add(egui::Shape::Rect(
        egui::epaint::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(if p.light { 36 } else { 110 }),
        }
        .as_shape(img_rect, CornerRadius::ZERO),
    ));

    let visible_img = img_rect.intersect(canvas_rect);
    if visible_img.is_positive() {
        studio::paint_checkerboard(&painter, visible_img, p.light);
    }
    if let Some(tex) = &app.studio.texture {
        painter.image(
            tex.id(),
            img_rect,
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    painter.rect_stroke(
        img_rect,
        0.0,
        Stroke::new(1.0_f32, p.line),
        egui::StrokeKind::Outside,
    );

    // Paint shapes clipped to image rect
    let img_painter = painter.with_clip_rect(visible_img);
    let img_bounds = Region {
        x: 0,
        y: 0,
        width: app.studio.width,
        height: app.studio.height,
    };
    super::paint_shapes_view(
        &img_painter,
        &img_bounds,
        img_rect.min,
        zoom,
        &app.studio.editor.shapes,
        app.studio.editor.editing_text_index,
        app.studio.editor.draft.as_ref(),
        &app.studio.blur_patches,
        app.studio.draft_blur.as_ref().map(|(_, t)| t),
    );

    // Draft blur outline
    if let Some(Shape::Blur { a, b, .. }) = &app.studio.editor.draft {
        let r = edit::norm_rect(*a, *b);
        let d_rect = Rect::from_min_max(
            to_screen((r.x, r.y)),
            to_screen((r.x + r.width as i32, r.y + r.height as i32)),
        );
        studio::paint_dashed_rect(&painter, d_rect, p.accent_bright);
    }

    // Selected shape dashed frame + white handles (ShareX-style)
    if !app.studio.crop_mode
        && app.studio.editor.text_at.is_none()
        && let Some(sel_idx) = app.studio.editor.selected
        && let Some(sel_shape) = app.studio.editor.shapes.get(sel_idx)
    {
        studio::paint_shape_selection(
            &painter,
            sel_shape,
            hovered_shape_handle,
            p.accent_bright,
            text_layout,
            to_screen,
        );
    }

    // Crop overlay
    if app.studio.crop_mode
        && let Some(cr) = app.studio.crop_rect.filter(|r| r.width > 0 && r.height > 0)
    {
        let cr_screen = Rect::from_min_max(
            to_screen((cr.x, cr.y)),
            to_screen((cr.x + cr.width as i32, cr.y + cr.height as i32)),
        );
        let scrim = Color32::from_rgba_unmultiplied(8, 10, 14, 155);
        for sub in [
            Rect::from_min_max(img_rect.min, Pos2::new(img_rect.max.x, cr_screen.min.y)),
            Rect::from_min_max(Pos2::new(img_rect.min.x, cr_screen.max.y), img_rect.max),
            Rect::from_min_max(
                Pos2::new(img_rect.min.x, cr_screen.min.y),
                Pos2::new(cr_screen.min.x, cr_screen.max.y),
            ),
            Rect::from_min_max(
                Pos2::new(cr_screen.max.x, cr_screen.min.y),
                Pos2::new(img_rect.max.x, cr_screen.max.y),
            ),
        ] {
            let clipped = sub.intersect(visible_img);
            if clipped.is_positive() {
                painter.rect_filled(clipped, 0.0, scrim);
            }
        }
        // Rule-of-thirds hairlines
        for frac in [1.0 / 3.0, 2.0 / 3.0] {
            let vx = cr_screen.min.x + cr_screen.width() * frac;
            let hy = cr_screen.min.y + cr_screen.height() * frac;
            painter.line_segment(
                [
                    Pos2::new(vx, cr_screen.min.y),
                    Pos2::new(vx, cr_screen.max.y),
                ],
                Stroke::new(1.0_f32, Color32::from_white_alpha(65)),
            );
            painter.line_segment(
                [
                    Pos2::new(cr_screen.min.x, hy),
                    Pos2::new(cr_screen.max.x, hy),
                ],
                Stroke::new(1.0_f32, Color32::from_white_alpha(65)),
            );
        }
        let dummy = Shape::Rect {
            a: (cr.x, cr.y),
            b: (cr.x + cr.width as i32, cr.y + cr.height as i32),
            color: Rgba::rgb(255, 255, 255),
            width: 1.0,
            filled: false,
        };
        studio::paint_shape_selection(
            &painter,
            &dummy,
            None,
            p.accent_bright,
            text_layout,
            to_screen,
        );
    }

    // Live Text input overlay on canvas
    if let Some(at) = app.studio.editor.text_at {
        let screen_pt = to_screen(at);
        if canvas_rect.expand(40.0).contains(screen_pt) {
            let font_sz = app.studio.editor.text_size() * zoom;
            let c = app.studio.editor.color;
            let ink = Color32::from_rgb(c.r, c.g, c.b);
            let resp = studio::live_text_input(
                &ctx,
                egui::Id::new("studio_text_edit"),
                screen_pt,
                canvas_rect,
                &mut app.studio.editor.text,
                font_sz,
                ink,
                p.accent_bright,
                s.text_placeholder(),
            );
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("studio_text_edit_rect"), resp.rect));
            if resp.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                app.studio.editor.commit_text();
            } else if app.studio.editor.text_at.is_some() {
                resp.request_focus();
            }
        }
    } else {
        ctx.data_mut(|d| d.remove::<Rect>(egui::Id::new("studio_text_edit_rect")));
    }
}

fn hit_crop_handle(
    cr: Region,
    p: (i32, i32),
    radius: i32,
    text_layout: edit::TextLayout<'_>,
) -> Option<edit::ShapeHandle> {
    let dummy = Shape::Rect {
        a: (cr.x, cr.y),
        b: (cr.x + cr.width as i32, cr.y + cr.height as i32),
        color: Rgba::rgb(0, 0, 0),
        width: 1.0,
        filled: false,
    };
    edit::hit_shape_handle(&dummy, p, radius, text_layout)
}

fn settings_view(app: &mut App, ui: &mut egui::Ui, p: Palette, s: &Strings) {
    ui.spacing_mut().item_spacing = egui::vec2(8.0, 14.0);
    let content_w = ui.available_width().min(660.0);
    ui.set_max_width(content_w);

    // Section 1: General
    settings_group(ui, s.sec_general(), p, |ui| {
        setting_row(ui, s.lbl_language(), s.desc_language(), p, |ui| {
            for (lang_opt, title) in [(Lang::En, "English"), (Lang::Tr, "Türkçe")] {
                let active = app.config.lang == lang_opt;
                if compact_btn(
                    ui,
                    None,
                    Some(title),
                    title,
                    true,
                    if active {
                        BtnKind::ActiveTool
                    } else {
                        BtnKind::Subtle
                    },
                    p,
                )
                .clicked()
                    && !active
                {
                    app.config.lang = lang_opt;
                    app.persist();
                }
            }
        });
        row_sep(ui, p);
        setting_row(ui, s.lbl_theme(), s.desc_theme(), p, |ui| {
            for (is_light, ic, title) in [
                (false, UiIcon::Moon, s.theme_dark()),
                (true, UiIcon::Sun, s.theme_light()),
            ] {
                let active = app.config.light_theme == is_light;
                if compact_btn(
                    ui,
                    Some(ic),
                    Some(title),
                    title,
                    true,
                    if active {
                        BtnKind::ActiveTool
                    } else {
                        BtnKind::Subtle
                    },
                    p,
                )
                .clicked()
                    && !active
                {
                    app.config.light_theme = is_light;
                    app.native.set_theme(app.config.light_theme);
                    app.persist();
                }
            }
        });
        row_sep(ui, p);
        let mut startup = app.config.run_at_startup;
        setting_row(ui, s.lbl_startup(), s.desc_startup(), p, |ui| {
            if toggle_switch(ui, &mut startup, s.lbl_startup(), p).changed() {
                match crate::startup::set(startup) {
                    Ok(()) => {
                        app.config.run_at_startup = startup;
                        app.persist();
                    }
                    Err(error) => {
                        tracing::error!(%error, "startup key write failed");
                        app.error = Some(s.err_startup_failed().into());
                    }
                }
            }
        });
    });

    // Section 2: Capture & Shortcut
    settings_group(ui, s.sec_capture(), p, |ui| {
        let active_hk = app
            .hotkey
            .map(|h| h.label())
            .unwrap_or_else(|| s.none().to_owned());
        let hk_desc = format!("{} {}", s.active_prefix(), active_hk);
        setting_row(ui, s.lbl_hotkey(), &hk_desc, p, |ui| {
            if compact_btn(
                ui,
                None,
                Some(if app.applying {
                    s.applying()
                } else {
                    s.apply()
                }),
                s.apply(),
                !app.applying,
                BtnKind::SolidAccent,
                p,
            )
            .clicked()
            {
                app.error = None;
                app.applying = true;
                app.native.set_hotkey(app.draft);
            }
            egui::ComboBox::from_id_salt("hotkey_combo")
                .selected_text(
                    super::Hotkey {
                        modifiers: 0,
                        ..app.draft
                    }
                    .label(),
                )
                .width(118.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.draft.key, 0x2c, "Print Screen");
                    for n in 0..12 {
                        ui.selectable_value(&mut app.draft.key, 0x70 + n, format!("F{}", n + 1));
                    }
                });
            for (flag, label) in [(4, "Shift"), (1, "Alt"), (2, "Ctrl")] {
                let checked = app.draft.modifiers & flag != 0;
                if compact_btn(
                    ui,
                    None,
                    Some(label),
                    label,
                    true,
                    if checked {
                        BtnKind::ActiveTool
                    } else {
                        BtnKind::Subtle
                    },
                    p,
                )
                .clicked()
                {
                    app.draft.modifiers ^= flag;
                }
            }
        });
        row_sep(ui, p);
        setting_row(ui, s.lbl_cursor(), s.desc_cursor(), p, |ui| {
            if toggle_switch(ui, &mut app.config.include_cursor, s.lbl_cursor(), p).changed() {
                app.persist();
            }
        });
        row_sep(ui, p);
        setting_row(ui, s.lbl_magnifier(), s.desc_magnifier(), p, |ui| {
            if toggle_switch(ui, &mut app.config.show_magnifier, s.lbl_magnifier(), p).changed() {
                app.persist();
            }
        });
    });

    // Section 3: Storage & Clipboard
    settings_group(ui, s.sec_storage(), p, |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(
                RichText::new(s.lbl_save_folder())
                    .size(BODY_SIZE)
                    .color(p.text),
            );
            ui.label(
                RichText::new(s.desc_save_folder())
                    .size(CAPTION_SIZE)
                    .color(p.muted),
            );
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let save_w = compact_btn_width(ui, false, Some(s.save()));
                let folder_w = compact_btn_width(ui, true, Some(s.open_folder()));
                let gap = ui.spacing().item_spacing.x * 2.0;
                let field_w = (ui.available_width() - save_w - folder_w - gap).max(120.0);
                ui.add_sized(
                    [field_w, CONTROL_H],
                    egui::TextEdit::singleline(&mut app.folder_draft)
                        .hint_text(r"Pictures\xscreen")
                        .vertical_align(egui::Align::Center)
                        .margin(Margin::symmetric(8, 4)),
                );
                if compact_btn(
                    ui,
                    None,
                    Some(s.save()),
                    s.save(),
                    true,
                    BtnKind::SolidAccent,
                    p,
                )
                .clicked()
                {
                    app.config.save_dir = app.folder_draft.trim().to_owned();
                    app.persist();
                    app.album.refresh(app.config.save_folder(), ui.ctx(), true);
                }
                if compact_btn(
                    ui,
                    Some(UiIcon::Folder),
                    Some(s.open_folder()),
                    s.open_folder_tip(),
                    true,
                    BtnKind::Subtle,
                    p,
                )
                .clicked()
                {
                    app.native.open_folder(app.config.save_folder().as_deref());
                }
            });
        });
        row_sep(ui, p);
        setting_row(ui, s.lbl_copy_png(), s.desc_copy_png(), p, |ui| {
            if toggle_switch(ui, &mut app.config.copy_png, s.lbl_copy_png(), p).changed() {
                app.persist();
            }
        });
        row_sep(ui, p);
        setting_row(ui, s.lbl_copy_and_save(), s.desc_copy_and_save(), p, |ui| {
            if toggle_switch(ui, &mut app.config.copy_and_save, s.lbl_copy_and_save(), p).changed()
            {
                app.persist();
            }
        });
    });
}

fn settings_group(ui: &mut egui::Ui, heading: &str, p: Palette, body: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.label(
            RichText::new(heading)
                .size(SECTION_SIZE)
                .strong()
                .color(p.text),
        );
        egui::Frame::NONE
            .fill(p.bar)
            .stroke(Stroke::new(1.0_f32, p.line))
            .corner_radius(CornerRadius::same(RADIUS_MD_U8))
            .inner_margin(Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 8.0;
                body(ui);
            });
    });
}

fn setting_row(
    ui: &mut egui::Ui,
    title: &str,
    desc: &str,
    p: Palette,
    controls_rtl: impl FnOnce(&mut egui::Ui),
) {
    if ui.available_width() < 460.0 {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.label(RichText::new(title).size(BODY_SIZE).color(p.text));
            ui.label(RichText::new(desc).size(CAPTION_SIZE).color(p.muted));
            ui.add_space(3.0);
            ui.horizontal_wrapped(|ui| {
                controls_rtl(ui);
            });
        });
    } else {
        ui.horizontal(|ui| {
            let left_w = (ui.available_width() * 0.50).max(160.0);
            ui.allocate_ui_with_layout(
                egui::vec2(left_w, CONTROL_H + 8.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new(title).size(BODY_SIZE).color(p.text));
                    ui.label(RichText::new(desc).size(CAPTION_SIZE).color(p.muted));
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                controls_rtl(ui);
            });
        });
    }
}

fn row_sep(ui: &mut egui::Ui, p: Palette) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 5.0), egui::Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.center().y),
            egui::pos2(rect.right(), rect.center().y),
        ],
        Stroke::new(1.0_f32, p.line),
    );
}

fn toggle_switch(ui: &mut egui::Ui, on: &mut bool, a11y: &str, p: Palette) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(36.0, 20.0), egui::Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let anim = ui.ctx().animate_bool_with_time(resp.id, *on, 0.12);
    let track = lerp_color(p.panel, p.accent, anim);
    let border = if *on {
        p.accent_bright
    } else if resp.hovered() {
        p.muted
    } else {
        p.line
    };
    ui.painter().rect_filled(rect, 10.0, track);
    ui.painter().rect_stroke(
        rect,
        10.0,
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    let knob_x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), anim);
    ui.painter()
        .circle_filled(egui::pos2(knob_x, rect.center().y), 6.8, Color32::WHITE);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, a11y));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    focus_ring(ui, &resp, p);
    resp
}

fn format_modified(modified: SystemTime, s: &Strings) -> String {
    let Ok(elapsed) = modified.elapsed() else {
        return s.time_now().into();
    };
    let secs = elapsed.as_secs();
    if secs < 60 {
        s.time_now().into()
    } else if secs < 3600 {
        s.time_mins_ago(secs / 60)
    } else if secs < 86_400 {
        s.time_hours_ago(secs / 3600)
    } else if secs < 30 * 86_400 {
        s.time_days_ago(secs / 86_400)
    } else {
        s.time_months_ago(secs / (30 * 86_400))
    }
}

fn file_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}
