use egui::{Color32, Rounding, Stroke, Visuals};

pub struct ModernTheme;

#[allow(dead_code)]
impl ModernTheme {
    // 1. Fond Dark Carbon / Obsidian 2026 (Zéro fond bleu criard, finition mate pure gamer)
    pub const BG_DARK: Color32 = Color32::from_rgb(11, 12, 14);          // #0B0C0E
    pub const PANEL_BG: Color32 = Color32::from_rgb(17, 19, 23);         // #111317
    pub const CARD_BG: Color32 = Color32::from_rgb(23, 26, 33);          // #171A21
    pub const CARD_BG_HOVER: Color32 = Color32::from_rgb(32, 36, 46);    // #20242E
    pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(38, 43, 55);    // #262B37

    // 2. Accents Technologiques & Charte Graphique du Logo
    pub const CYAN_VIBRANT: Color32 = Color32::from_rgb(0, 210, 255);    // #00D2FF
    pub const BLUE_ELECTRIC: Color32 = Color32::from_rgb(0, 87, 255);    // #0057FF
    pub const BLUE_DEEP: Color32 = Color32::from_rgb(0, 56, 168);        // #0038A8
    pub const GREEN_VIVID: Color32 = Color32::from_rgb(0, 229, 153);     // #00E599
    pub const GREEN_MINT: Color32 = Color32::from_rgb(0, 191, 165);      // #00BFA5
    pub const ORANGE_CORAL: Color32 = Color32::from_rgb(255, 109, 0);    // #FF6D00
    pub const INDIGO_ACCENT: Color32 = Color32::from_rgb(101, 31, 255);  // #651FFF
    pub const RED_ERROR: Color32 = Color32::from_rgb(255, 68, 68);       // #FF4444

    // 3. Typographie Haute Lisibilité
    pub const ICON_WHITE: Color32 = Color32::from_rgb(255, 255, 255);
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(240, 244, 248);  // #F0F4F8
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(149, 161, 178); // #95A1B2
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(101, 113, 130);    // #657182

    // Alias de compatibilité
    pub const NEON_GREEN: Color32 = Self::GREEN_VIVID;
    pub const NEON_CYAN: Color32 = Self::CYAN_VIBRANT;
    pub const NEON_PURPLE: Color32 = Self::INDIGO_ACCENT;
    pub const NEON_MAGENTA: Color32 = Self::ORANGE_CORAL;
    pub const RETRO_AMBER: Color32 = Self::ORANGE_CORAL;
    pub const RETRO_RED: Color32 = Self::RED_ERROR;

    pub fn apply(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        let mut visuals = Visuals::dark();
        visuals.panel_fill = Self::BG_DARK;
        visuals.window_fill = Self::PANEL_BG;
        visuals.faint_bg_color = Self::CARD_BG;
        visuals.extreme_bg_color = Color32::from_rgb(8, 9, 11);
        
        let rounding = Rounding::same(7.0);
        
        visuals.widgets.noninteractive.bg_fill = Self::PANEL_BG;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Self::BORDER_SUBTLE);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);
        visuals.widgets.noninteractive.rounding = rounding;

        visuals.widgets.inactive.bg_fill = Self::CARD_BG;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Self::BORDER_SUBTLE);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);
        visuals.widgets.inactive.rounding = rounding;

        visuals.widgets.hovered.bg_fill = Self::CARD_BG_HOVER;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.2_f32, Self::CYAN_VIBRANT);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Self::ICON_WHITE);
        visuals.widgets.hovered.rounding = rounding;

        visuals.widgets.active.bg_fill = Color32::from_rgb(26, 32, 44);
        visuals.widgets.active.bg_stroke = Stroke::new(1.8_f32, Self::CYAN_VIBRANT);
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

pub type RetroTheme = ModernTheme;
