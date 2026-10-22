//! The document: tabs, cursors, settings, and window geometry.

use std::fs;
use std::path::Path;

use hactorio_core::Settings;
use serde::{Deserialize, Serialize};

pub const DOCUMENT_VERSION: u32 = 1;

/// One tab's editable text.
#[derive(Debug, Clone)]
pub struct Tab {
    pub text: String,
    pub cursor_line: usize,
    pub scroll: f32,
}

impl Tab {
    pub fn from_text(text: &str) -> Self {
        Tab {
            text: text.to_string(),
            cursor_line: 0,
            scroll: 0.0,
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        let lines = self.text.split('\n').count();
        self.cursor_line = self.cursor_line.min(lines.saturating_sub(1));
    }

    /// The first comment line, without the `#`, or `Untitled`.
    pub fn title(&self) -> String {
        for line in self.text.split('\n') {
            if let Some(rest) = line.trim_start().strip_prefix('#') {
                let title = rest.trim();
                if !title.is_empty() {
                    return title.to_string();
                }
            }
        }
        "Untitled".to_string()
    }
}

/// The saved document, version first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    pub tabs: Vec<String>,
    pub active_tab: usize,
    #[serde(default)]
    pub cursors: Vec<Cursor>,
    #[serde(default)]
    pub window: Option<Window>,
    #[serde(default)]
    pub settings: Settings,
    /// Width of the results column.
    #[serde(default = "default_results_width")]
    pub results_width: f32,
    /// Global GUI zoom factor.
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    /// Monospace font size for the editor and outputs.
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    /// Monospace font family for the editor and outputs; `None` uses egui's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}

pub fn default_results_width() -> f32 {
    380.0
}

pub fn default_zoom() -> f32 {
    1.0
}

pub fn default_font_size() -> f32 {
    13.0
}

/// Zoom range accepted by egui's keyboard and scroll zooming.
pub const MIN_ZOOM: f32 = 0.2;
pub const MAX_ZOOM: f32 = 5.0;

pub const MIN_FONT_SIZE: f32 = 8.0;
pub const MAX_FONT_SIZE: f32 = 48.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct Cursor {
    pub line: usize,
    pub column: usize,
    pub scroll: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Window {
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

impl Default for Document {
    fn default() -> Self {
        Document {
            version: DOCUMENT_VERSION,
            tabs: vec!["# Main\n".to_string()],
            active_tab: 0,
            cursors: Vec::new(),
            window: None,
            settings: Settings::default(),
            results_width: default_results_width(),
            zoom: default_zoom(),
            font_size: default_font_size(),
            font: None,
        }
    }
}

impl Document {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
        serde_json::from_str(&text).map_err(|error| error.to_string())
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        let temp = path.with_extension("tmp");
        fs::write(&temp, &text).map_err(|error| error.to_string())?;
        fs::rename(&temp, path).map_err(|error| error.to_string())
    }
}
