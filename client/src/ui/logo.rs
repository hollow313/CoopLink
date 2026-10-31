use egui::{ColorImage, TextureHandle, TextureOptions};

/// Charge l'icône officielle CoopLink.png avec détourage automatique du fond blanc
pub fn load_embedded_logo(ctx: &egui::Context) -> Option<TextureHandle> {
    let image_bytes = include_bytes!("../../../images/CoopLink.png");
    if let Ok(dynamic_img) = image::load_from_memory(image_bytes) {
        let mut rgba = dynamic_img.to_rgba8();
        for pixel in rgba.pixels_mut() {
            let [r, g, b, a] = pixel.0;
            if r >= 246 && g >= 246 && b >= 246 {
                pixel.0 = [255, 255, 255, 0];
            } else if r >= 235 && g >= 235 && b >= 235 {
                // Adoucissement anti-aliasing sur les bords
                let lum = ((r as u16 + g as u16 + b as u16) / 3) as u8;
                let alpha = ((246u8.saturating_sub(lum) as u16 * 255) / 11).min(a as u16) as u8;
                pixel.0 = [r, g, b, alpha];
            }
        }
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba.into_raw());
        Some(ctx.load_texture("app_logo", color_image, TextureOptions::LINEAR))
    } else {
        None
    }
}
