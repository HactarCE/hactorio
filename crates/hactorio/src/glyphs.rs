//! Glyphs: one PUA codepoint per namespace entry, rasterized from icon PNGs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use egui::ColorImage;
use egui::epaint::text::{
    FontPriority, GlyphBitmap, GlyphRasterizer, GlyphRasterizerRequest, RasterizedGlyph,
};
use hactorio_data::{EMPTY_GLYPH, IconImage, IconSource, QUALITIES, Registry, SIGNALS};

/// Icon path of the variable's virtual signal.
const VARIABLE_ICON: &str = "__base__/graphics/icons/signal/signal-item-parameter.png";

/// Glyph for a variable in the completion popup.
pub use hactorio_data::VARIABLE_GLYPH as VARIABLE;

/// Installs a highest-priority rasterizer that renders namespace glyphs from
/// icon PNGs.
///
/// Replacing the rasterizer list drops every cached glyph, which is what a
/// game-data reload needs.
pub fn install(ctx: &egui::Context, registry: Arc<Registry>, source: &IconSource) {
    let mut images: HashMap<char, IconImage> = HashMap::new();
    for entry in registry.entries() {
        let Some(path) = &entry.icon else {
            continue;
        };
        if let Some(image) = source.read_rgba(path) {
            images.insert(entry.glyph, image);
        }
    }
    for signal in SIGNALS.iter().chain(QUALITIES.iter()) {
        if let Some(image) = source.read_rgba(signal.icon) {
            images.insert(signal.glyph, image);
        }
    }
    if let Some(image) = source.read_rgba(VARIABLE_ICON) {
        images.insert(VARIABLE, image);
    }
    images.insert(EMPTY_GLYPH, empty_image());
    let images = Arc::new(images);
    type ScaleCache = Arc<Mutex<HashMap<(char, u32), Arc<ColorImage>>>>;
    let scaled: ScaleCache = Arc::new(Mutex::new(HashMap::new()));

    let rasterizer = GlyphRasterizer::new(
        "hactorio::glyphs",
        move |request: &GlyphRasterizerRequest<'_>| {
            // Highest priority means this is asked about every cluster: reject fast.
            let mut chars = request.cluster.chars();
            let glyph = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            let source = images.get(&glyph)?;

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

fn empty_image() -> IconImage {
    IconImage {
        width: 64,
        height: 64,
        rgba: vec![0; 64 * 64 * 4],
    }
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
