use egui::{ColorImage, TextureHandle, TextureOptions};

pub fn load_embedded_logo(ctx: &egui::Context) -> Option<TextureHandle> {
    let image_bytes = include_bytes!("../../../images/logo.jpg");
    if let Ok(dynamic_img) = image::load_from_memory(image_bytes) {
        let rgba = dynamic_img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba.into_raw());
        Some(ctx.load_texture("app_logo", color_image, TextureOptions::LINEAR))
    } else {
        None
    }
}
