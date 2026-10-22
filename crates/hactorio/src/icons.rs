//! Icon glyphs: one PUA codepoint per namespace entry, rasterized from PNGs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use egui::ColorImage;
use egui::epaint::text::{
    FontPriority, GlyphBitmap, GlyphRasterizer, GlyphRasterizerRequest, RasterizedGlyph,
};
use hactorio_data::{IconImage, IconSource, Registry, STACK_SIZE_GLYPH, STACK_SIZE_ICON};

/// Icons that are not namespace entries, as `(glyph, data path)`.
const EXTRAS: &[(char, &str)] = &[(STACK_SIZE_GLYPH, STACK_SIZE_ICON)];

/// Installs a highest-priority rasterizer that draws namespace glyphs as icons.
///
/// Replacing the rasterizer list drops every cached glyph, which is what a
/// game-data reload needs.
pub fn install(ctx: &egui::Context, registry: Arc<Registry>, source: &IconSource) {
    let mut icons: HashMap<char, IconImage> = HashMap::new();
    for entry in registry.entries() {
        let Some(path) = &entry.icon else {
            continue;
        };
        if let Some(image) = source.read_rgba(path) {
            icons.insert(entry.glyph, image);
        }
    }
    for &(glyph, path) in EXTRAS {
        if let Some(image) = source.read_rgba(path) {
            icons.insert(glyph, image);
        }
    }
    let icons = Arc::new(icons);
    type ScaleCache = Arc<Mutex<HashMap<(char, u32), Arc<ColorImage>>>>;
    let scaled: ScaleCache = Arc::new(Mutex::new(HashMap::new()));

    let rasterizer = GlyphRasterizer::new(
        "hactorio::icons",
        move |request: &GlyphRasterizerRequest<'_>| {
            // Highest priority means this is asked about every cluster: reject fast.
            let mut chars = request.cluster.chars();
            let glyph = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            let source = icons.get(&glyph)?;

            let size = (request.font_size_px.round() as u32).clamp(1, 256);
            let image = {
                let mut scaled = scaled.lock().ok()?;
                Arc::clone(
                    scaled
                        .entry((glyph, size))
                        .or_insert_with(|| Arc::new(resize(source, size))),
                )
            };

            let advance = request.font_size_px;
            Some(RasterizedGlyph {
                bitmap: GlyphBitmap {
                    image: (*image).clone(),
                    offset_px: egui::Vec2::new(0.0, -advance * 0.85),
                    is_color: true,
                },
                advance_px: advance,
            })
        },
    )
    .with_priority(FontPriority::Highest);

    ctx.set_glyph_rasterizers(vec![rasterizer]);
}

fn resize(source: &IconImage, size: u32) -> ColorImage {
    let Some(buffer) = image::RgbaImage::from_raw(source.width, source.height, source.rgba.clone())
    else {
        return ColorImage::default();
    };
    let resized =
        image::imageops::resize(&buffer, size, size, image::imageops::FilterType::Triangle);
    ColorImage::from_rgba_unmultiplied([size as usize, size as usize], resized.as_raw())
}
