use egui::{Color32, Rounding, Stroke, Visuals};

pub struct RetroTheme;

#[allow(dead_code)]
impl RetroTheme {
    // Palette Sombre Profonde
    pub const BG_DARK: Color32 = Color32::from_rgb(10, 13, 20);
    pub const PANEL_BG: Color32 = Color32::from_rgb(15, 20, 32);
    pub const CARD_BG: Color32 = Color32::from_rgb(20, 27, 44);
    pub const CARD_BG_ELEVATED: Color32 = Color32::from_rgb(26, 35, 58);
    
    // Néons Rétro Gaming Haut de Gamme
    pub const NEON_GREEN: Color32 = Color32::from_rgb(0, 255, 148);
    pub const NEON_CYAN: Color32 = Color32::from_rgb(0, 230, 255);
    pub const NEON_PURPLE: Color32 = Color32::from_rgb(180, 70, 255);
    pub const NEON_MAGENTA: Color32 = Color32::from_rgb(255, 45, 130);
    pub const NEON_PINK: Color32 = Color32::from_rgb(255, 45, 130);
    pub const RETRO_AMBER: Color32 = Color32::from_rgb(255, 185, 20);
    pub const RETRO_RED: Color32 = Color32::from_rgb(255, 60, 60);
    
    // Typographie
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(245, 248, 255);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(165, 180, 205);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(115, 130, 155);

    pub fn apply(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        let mut visuals = Visuals::dark();
        visuals.panel_fill = Self::BG_DARK;
        visuals.window_fill = Self::PANEL_BG;
        visuals.faint_bg_color = Self::CARD_BG;
        visuals.extreme_bg_color = Color32::from_rgb(7, 9, 14);
        
        // Bordures soignées avec arrondis modernes
        let rounding = Rounding::same(6.0);
        
        visuals.widgets.noninteractive.bg_fill = Self::PANEL_BG;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(32, 42, 68));
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);
        visuals.widgets.noninteractive.rounding = rounding;

        visuals.widgets.inactive.bg_fill = Self::CARD_BG;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(40, 54, 85));
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);
        visuals.widgets.inactive.rounding = rounding;

        visuals.widgets.hovered.bg_fill = Self::CARD_BG_ELEVATED;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, Self::NEON_CYAN);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.hovered.rounding = rounding;

        visuals.widgets.active.bg_fill = Color32::from_rgb(35, 48, 76);
        visuals.widgets.active.bg_stroke = Stroke::new(2.0_f32, Self::NEON_GREEN);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Self::NEON_GREEN);
        visuals.widgets.active.rounding = rounding;

        visuals.selection.bg_fill = Self::NEON_GREEN.gamma_multiply(0.20);
        visuals.selection.stroke = Stroke::new(1.0_f32, Self::NEON_GREEN);

        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(14.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(12.0);

        ctx.set_style(style);
    }
}
