use egui::{Color32, FontFamily, FontId, Rounding, Stroke, TextStyle, Visuals};

pub struct ModernTheme;

#[allow(dead_code)]
impl ModernTheme {
    // 1. Surfaces Modernes Windows 11 Fluent (Clair, épuré, zéro bleu nuit, zéro rétro)
    pub const BG_APP: Color32 = Color32::from_rgb(244, 247, 251);        // #F4F7FB (Mica Cloud)
    pub const PANEL_BG: Color32 = Color32::from_rgb(255, 255, 255);      // #FFFFFF (Blanc pur)
    pub const CARD_BG: Color32 = Color32::from_rgb(255, 255, 255);       // #FFFFFF (Carte blanche)
    pub const SUBTLE_BG: Color32 = Color32::from_rgb(248, 250, 252);     // #F8FAFC (Fond secondaire)
    pub const CARD_BG_HOVER: Color32 = Color32::from_rgb(238, 244, 252); // #EEF4FC (Survol doux)
    pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(226, 232, 240); // #E2E8F0 (Bordure fine)

    // 2. Palette Exacte de la Nouvelle Icône CoopLink.png
    pub const BLUE_ELECTRIC: Color32 = Color32::from_rgb(0, 102, 255);   // #0066FF (Bleu principal icône)
    pub const CYAN_VIBRANT: Color32 = Color32::from_rgb(0, 188, 242);    // #00BCF2 (Turquoise/Cyan icône)
    pub const GREEN_VIVID: Color32 = Color32::from_rgb(0, 186, 136);     // #00BA88 (Vert émeraude icône)
    pub const GREEN_MINT: Color32 = Color32::from_rgb(0, 191, 165);      // #00BFA5 (Menthe réseau)
    pub const INDIGO_ACCENT: Color32 = Color32::from_rgb(123, 63, 228);  // #7B3FE4 (Violet dégradé icône)
    pub const ORANGE_CORAL: Color32 = Color32::from_rgb(255, 122, 40);   // #FF7A28 (Sphère orange centrale)
    pub const RED_ERROR: Color32 = Color32::from_rgb(239, 68, 68);       // #EF4444 (Rouge alerte)

    // 3. Typographie Haute Lisibilité (Slate Moderne sur fond clair)
    pub const ICON_WHITE: Color32 = Color32::from_rgb(255, 255, 255);
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(15, 23, 42);     // #0F172A (Noir ardoise profond)
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(71, 85, 105);  // #475569 (Gris ardoise)
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(100, 116, 139);    // #64748B (Gris doux)

    // Alias de compatibilité
    pub const BG_DARK: Color32 = Self::BG_APP;
    pub const NEON_GREEN: Color32 = Self::GREEN_VIVID;
    pub const NEON_CYAN: Color32 = Self::BLUE_ELECTRIC;
    pub const NEON_PURPLE: Color32 = Self::INDIGO_ACCENT;
    pub const NEON_MAGENTA: Color32 = Self::ORANGE_CORAL;
    pub const RETRO_AMBER: Color32 = Self::ORANGE_CORAL;
    pub const RETRO_RED: Color32 = Self::RED_ERROR;

    pub fn apply(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();

        let mut visuals = Visuals::light();
        visuals.panel_fill = Self::BG_APP;
        visuals.window_fill = Self::PANEL_BG;
        visuals.faint_bg_color = Self::SUBTLE_BG;
        visuals.extreme_bg_color = Self::SUBTLE_BG;
        visuals.override_text_color = Some(Self::TEXT_PRIMARY);

        let rounding = Rounding::same(8.0);

        visuals.widgets.noninteractive.bg_fill = Self::PANEL_BG;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Self::BORDER_SUBTLE);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);
        visuals.widgets.noninteractive.rounding = rounding;

        visuals.widgets.inactive.bg_fill = Self::SUBTLE_BG;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225));
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);
        visuals.widgets.inactive.rounding = rounding;

        visuals.widgets.hovered.bg_fill = Self::CARD_BG_HOVER;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, Self::BLUE_ELECTRIC);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Self::BLUE_ELECTRIC);
        visuals.widgets.hovered.rounding = rounding;

        visuals.widgets.active.bg_fill = Color32::from_rgb(224, 236, 255);
        visuals.widgets.active.bg_stroke = Stroke::new(1.8_f32, Self::BLUE_ELECTRIC);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Self::BLUE_ELECTRIC);
        visuals.widgets.active.rounding = rounding;

        visuals.selection.bg_fill = Self::BLUE_ELECTRIC.gamma_multiply(0.18);
        visuals.selection.stroke = Stroke::new(1.0_f32, Self::BLUE_ELECTRIC);

        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(17.5, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Body,
            FontId::new(14.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(12.5, FontFamily::Proportional),
        );

        style.visuals = visuals;
        style.interaction.selectable_labels = false;
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(11.0, 7.5);

        ctx.set_style(style);
    }

    /// Dessine une vraie pastille vectorielle circulaire (avec léger halo lumineux), sans dépendre de caractères Unicode
    pub fn draw_dot(ui: &mut egui::Ui, color: Color32, radius: f32) {
        let size = egui::vec2(radius * 2.8, radius * 2.8);
        let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());
        if ui.is_rect_visible(rect) {
            let center = rect.center();
            let halo = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 48);
            ui.painter().circle_filled(center, radius * 1.45, halo);
            ui.painter().circle_filled(center, radius, color);
        }
    }

    /// Affiche une pastille de statut moderne (ex: Pastille verte "Prêt")
    pub fn status_pill(ui: &mut egui::Ui, color: Color32, text: &str) {
        let bg = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 22);
        let border = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 95);
        egui::Frame::none()
            .fill(bg)
            .rounding(Rounding::same(20.0))
            .inner_margin(egui::Margin::symmetric(12.0, 5.0))
            .stroke(Stroke::new(1.0_f32, border))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    Self::draw_dot(ui, color, 4.5);
                    ui.label(egui::RichText::new(text).color(color).strong().size(13.0));
                });
            });
    }
}

#[allow(dead_code)]
pub type RetroTheme = ModernTheme;
