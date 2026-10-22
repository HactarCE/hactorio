//! System font discovery for the editor and results monospace face.

use std::sync::{Arc, LazyLock};

use egui::{Context, FontData, FontDefinitions, FontFamily};
use fontdb::{Database, FaceInfo, Style, Weight};

/// The system's monospace font families and their data.
pub struct SystemFonts {
    db: Database,
    families: Vec<String>,
}

static SYSTEM_FONTS: LazyLock<SystemFonts> = LazyLock::new(SystemFonts::load);

/// Loads the system font database once per process.
pub fn system_fonts() -> &'static SystemFonts {
    &SYSTEM_FONTS
}

impl SystemFonts {
    fn load() -> Self {
        let mut db = Database::new();
        db.load_system_fonts();

        let mut families: Vec<String> = db
            .faces()
            .filter(|face| face.monospaced)
            .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
            .collect();
        families.sort_unstable();
        families.dedup();

        SystemFonts { db, families }
    }

    /// The monospace family names available on this system, sorted.
    pub fn families(&self) -> &[String] {
        &self.families
    }

    pub fn contains(&self, family: &str) -> bool {
        self.families.iter().any(|known| known == family)
    }

    /// Installs `family` as the primary monospace font, keeping egui's
    /// monospace fonts as fallbacks. `None` restores the default.
    pub fn apply(&self, ctx: &Context, family: Option<&str>) {
        let mut definitions = FontDefinitions::default();
        if let Some(family) = family
            && let Some(face) = self.face(family)
            && let Some((bytes, index)) = self
                .db
                .with_face_data(face.id, |data, index| (data.to_vec(), index))
        {
            let key = format!("system-font:{family}");
            definitions.font_data.insert(
                key.clone(),
                Arc::new(FontData::from_blob(Arc::new(bytes), index)),
            );
            definitions
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .insert(0, key);
        }
        ctx.set_fonts(definitions);
    }

    /// The regular face of `family`, or its first face if it has no regular.
    fn face(&self, family: &str) -> Option<&FaceInfo> {
        let mut fallback = None;
        for face in self.db.faces() {
            if !face.monospaced || !face.families.iter().any(|(name, _)| name == family) {
                continue;
            }
            if face.style == Style::Normal && face.weight == Weight::NORMAL {
                return Some(face);
            }
            fallback.get_or_insert(face);
        }
        fallback
    }
}
