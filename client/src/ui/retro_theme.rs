use egui::{Color32, Rounding, Stroke, Visuals};

pub struct ModernTheme;

#[allow(dead_code)]
impl ModernTheme {
    // 1. Nuances dominantes (Bleu & Cyan)
    pub const BLUE_ELECTRIC: Color32 = Color32::from_rgb(0, 87, 255);   // #0057FF
    pub const CYAN_VIBRANT: Color32 = Color32::from_rgb(0, 210, 255);   // #00D2FF
    pub const BLUE_DEEP: Color32 = Color32::from_rgb(0, 56, 168);       // #0038A8

    // 2. Nuances secondaires (Vert réseau & Accents)
    pub const GREEN_MINT: Color32 = Color32::from_rgb(0, 191, 165);     // #00BFA5
    pub const GREEN_VIVID: Color32 = Color32::from_rgb(0, 229, 153);    // #00E599
    pub const INDIGO_ACCENT: Color32 = Color32::from_rgb(101, 31, 255); // #651FFF
    pub const ORANGE_CORAL: Color32 = Color32::from_rgb(255, 109, 0);   // #FF6D00

    // 3. Fonds & Contraste
    pub const ICON_WHITE: Color32 = Color32::from_rgb(255, 255, 255);   // #FFFFFF
    pub const GRAY_NEUTRAL: Color32 = Color32::from_rgb(142, 142, 147); // #8E8E93

    // Fonds d'application sombres & modernes
    pub const BG_DARK: Color32 = Color32::from_rgb(9, 13, 22);
    pub const PANEL_BG: Color32 = Color32::from_rgb(15, 22, 38);
    pub const CARD_BG: Color32 = Color32::from_rgb(20, 30, 52);
    pub const CARD_BG_HOVER: Color32 = Color32::from_rgb(28, 40, 68);

    // Compatibilité
    pub const NEON_GREEN: Color32 = Self::GREEN_VIVID;
    pub const NEON_CYAN: Color32 = Self::CYAN_VIBRANT;
    pub const NEON_PURPLE: Color32 = Self::INDIGO_ACCENT;
    pub const NEON_MAGENTA: Color32 = Self::ORANGE_CORAL;
    pub const RETRO_AMBER: Color32 = Self::ORANGE_CORAL;
    pub const RETRO_RED: Color32 = Color32::from_rgb(255, 68, 68);
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(248, 250, 255);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(175, 185, 205);
    pub const TEXT_MUTED: Color32 = Self::GRAY_NEUTRAL;

    pub fn apply(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        let mut visuals = Visuals::dark();
        visuals.panel_fill = Self::BG_DARK;
        visuals.window_fill = Self::PANEL_BG;
        visuals.faint_bg_color = Self::CARD_BG;
        visuals.extreme_bg_color = Color32::from_rgb(6, 9, 16);
        
        let rounding = Rounding::same(6.0);
        
        visuals.widgets.noninteractive.bg_fill = Self::PANEL_BG;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(30, 42, 70));
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);
        visuals.widgets.noninteractive.rounding = rounding;

        visuals.widgets.inactive.bg_fill = Self::CARD_BG;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(38, 52, 86));
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);
        visuals.widgets.inactive.rounding = rounding;

        visuals.widgets.hovered.bg_fill = Self::CARD_BG_HOVER;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, Self::CYAN_VIBRANT);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Self::ICON_WHITE);
        visuals.widgets.hovered.rounding = rounding;

        visuals.widgets.active.bg_fill = Color32::from_rgb(32, 48, 82);
        visuals.widgets.active.bg_stroke = Stroke::new(2.0_f32, Self::BLUE_ELECTRIC);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Self::CYAN_VIBRANT);
        visuals.widgets.active.rounding = rounding;

        visuals.selection.bg_fill = Self::BLUE_ELECTRIC.gamma_multiply(0.25);
        visuals.selection.stroke = Stroke::new(1.0_f32, Self::CYAN_VIBRANT);

        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(14.0, 8.0);

        ctx.set_style(style);
    }
}

// Alias de rétrocompatibilité
pub type RetroTheme = ModernTheme;
