use egui::{Color32, Stroke, Visuals};

pub struct RetroTheme;

impl RetroTheme {
    pub const BG_DARK: Color32 = Color32::from_rgb(10, 11, 18);
    pub const PANEL_BG: Color32 = Color32::from_rgb(18, 22, 36);
    pub const CARD_BG: Color32 = Color32::from_rgb(25, 30, 48);
    
    // Accents néon rétro gaming
    pub const NEON_GREEN: Color32 = Color32::from_rgb(0, 255, 140);
    pub const NEON_CYAN: Color32 = Color32::from_rgb(0, 229, 255);
    pub const NEON_MAGENTA: Color32 = Color32::from_rgb(255, 0, 119);
    pub const RETRO_AMBER: Color32 = Color32::from_rgb(255, 176, 0);
    pub const RETRO_RED: Color32 = Color32::from_rgb(255, 59, 59);
    
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(240, 246, 252);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(140, 158, 180);

    pub fn apply(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        let mut visuals = Visuals::dark();
        visuals.panel_fill = Self::BG_DARK;
        visuals.window_fill = Self::PANEL_BG;
        visuals.faint_bg_color = Self::CARD_BG;
        visuals.extreme_bg_color = Color32::from_rgb(6, 7, 12);
        
        // Bordures et contours rétro style borne d'arcade
        visuals.widgets.noninteractive.bg_fill = Self::PANEL_BG;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(40, 50, 75));
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);

        visuals.widgets.inactive.bg_fill = Self::CARD_BG;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(50, 65, 95));
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);

        visuals.widgets.hovered.bg_fill = Color32::from_rgb(35, 45, 70);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, Self::NEON_CYAN);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);

        visuals.widgets.active.bg_fill = Color32::from_rgb(45, 60, 90);
        visuals.widgets.active.bg_stroke = Stroke::new(2.0_f32, Self::NEON_GREEN);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Self::NEON_GREEN);

        visuals.selection.bg_fill = Self::NEON_GREEN.gamma_multiply(0.25);
        visuals.selection.stroke = Stroke::new(1.0_f32, Self::NEON_GREEN);

        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);

        ctx.set_style(style);
    }
}
