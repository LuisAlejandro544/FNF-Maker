//! Archivo: engine/crates/pulse_ui/src/theme.rs
//! Propósito: Paleta de colores profesional, cómoda para la vista y de alto contraste.
//! Evita colores estridentes o amarillos incómodos; combina azul pizarra profundo con
//! acentos en cian sereno, índigo suave, esmeralda y coral-rosa según el rol.

use eframe::egui::{self, Color32, Stroke};
use pulse_core::EntityRole;

pub const BG_CANVAS: Color32 = Color32::from_rgb(16, 20, 29);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(23, 29, 41);
pub const BG_CARD: Color32 = Color32::from_rgb(30, 38, 54);
pub const BG_WELL: Color32 = Color32::from_rgb(12, 16, 23);

pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(45, 57, 79);
pub const BORDER_FOCUS: Color32 = Color32::from_rgb(56, 189, 248);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(232, 238, 247);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(142, 158, 184);

pub const ACCENT_TEAL: Color32 = Color32::from_rgb(34, 211, 238);
pub const ACCENT_EMERALD: Color32 = Color32::from_rgb(52, 211, 153);
pub const ACCENT_INDIGO: Color32 = Color32::from_rgb(129, 140, 248);
pub const ACCENT_CORAL: Color32 = Color32::from_rgb(244, 114, 182);
pub const ACCENT_DANGER: Color32 = Color32::from_rgb(248, 113, 113);

pub fn role_color(role: EntityRole) -> Color32 {
    match role {
        EntityRole::Background => ACCENT_INDIGO,
        EntityRole::Player => ACCENT_EMERALD,
        EntityRole::Opponent => ACCENT_CORAL,
        EntityRole::Decoration => ACCENT_TEAL,
    }
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(BG_ELEVATED)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
        .corner_radius(10.0)
        .inner_margin(12.0)
}

pub fn apply_studio_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = BG_ELEVATED;
    visuals.panel_fill = BG_CANVAS;
    visuals.extreme_bg_color = BG_WELL;
    visuals.faint_bg_color = BG_CARD;
    visuals.widgets.noninteractive.bg_fill = BG_CARD;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.inactive.bg_fill = BG_CARD;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(41, 52, 74);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.2, ACCENT_TEAL);
    visuals.widgets.active.bg_fill = Color32::from_rgb(49, 64, 92);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(34, 211, 238, 60);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT_TEAL);
    ctx.set_visuals(visuals);
}
