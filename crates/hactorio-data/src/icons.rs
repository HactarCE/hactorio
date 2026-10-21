//! Reads icon files from game data and mods.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Resolves `__base__/...`, `__core__/...`, and `__mod__/...` icon paths.
#[derive(Debug, Clone)]
pub struct IconSource {
    data_dir: PathBuf,
    mod_dir: PathBuf,
}

/// A decoded icon, ready to be uploaded as a texture or glyph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconImage {
    pub width: u32,
    pub height: u32,
    /// Unmultiplied RGBA, row-major.
    pub rgba: Vec<u8>,
}

impl IconSource {
    pub fn new(data_dir: impl Into<PathBuf>, mod_dir: impl Into<PathBuf>) -> Self {
        IconSource {
            data_dir: data_dir.into(),
            mod_dir: mod_dir.into(),
        }
    }

    /// Returns the PNG bytes for an icon path, if the file exists.
    pub fn read(&self, icon: &str) -> Option<Vec<u8>> {
        let rest = icon.strip_prefix("__")?;
        let (name, relative) = rest.split_once("__/")?;
        // Built-in mods (base, core, space-age, recycler, quality, ...) are
        // directories next to the executable's data directory.
        if let Ok(bytes) = std::fs::read(self.data_dir.join(name).join(relative)) {
            return Some(bytes);
        }
        self.read_from_mods(name, relative)
    }

    /// Decodes an icon to RGBA.
    ///
    /// Factorio 2.x icon files are mip strips: the full-size square is on the
    /// left, followed by smaller copies. Only the full-size square is kept.
    pub fn read_rgba(&self, icon: &str) -> Option<IconImage> {
        let bytes = self.read(icon)?;
        let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let (width, height) = image.dimensions();
        let image = if width > height {
            image::imageops::crop_imm(&image, 0, 0, height, height).to_image()
        } else {
            image
        };
        let (width, height) = image.dimensions();
        Some(IconImage {
            width,
            height,
            rgba: image.into_raw(),
        })
    }

    fn read_from_mods(&self, name: &str, relative: &str) -> Option<Vec<u8>> {
        let prefix = format!("{name}_");
        for entry in std::fs::read_dir(&self.mod_dir).ok()?.flatten() {
            let path = entry.path();
            let file_name = path.file_name()?.to_str()?;
            if !file_name.starts_with(&prefix) {
                continue;
            }
            if path.is_dir() {
                if let Ok(bytes) = std::fs::read(path.join(relative)) {
                    return Some(bytes);
                }
            } else if path.extension().is_some_and(|extension| extension == "zip")
                && let Some(bytes) = read_zip(&path, relative)
            {
                return Some(bytes);
            }
        }
        None
    }
}

fn read_zip(path: &Path, relative: &str) -> Option<Vec<u8>> {
    let file = File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let suffix = format!("/{relative}");
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).ok()?;
        let matches = entry
            .name()
            .map(|name| name.ends_with(&suffix))
            .unwrap_or(false);
        if matches {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).ok()?;
            return Some(bytes);
        }
    }
    None
}
