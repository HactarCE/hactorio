//! The editor palette, mimicking VS Code's 2026 Dark and Light themes.

use egui::{Color32, Visuals};
use hactorio_core::Highlight;

/// The editor's background, plain-text color, and token colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Palette {
    /// `editor.background`, also the text edit's fill.
    pub(super) background: Color32,
    /// `editor.foreground`: text outside any highlighted range.
    pub(super) plain: Color32,
    pub(super) operator_keyword: Color32,
    pub(super) unit_keyword: Color32,
    pub(super) number: Color32,
    pub(super) comment: Color32,
    pub(super) icon: Color32,
}

/// The palette for `visuals`' dark or light mode.
pub(super) fn palette(visuals: &Visuals) -> Palette {
    if visuals.dark_mode { DARK } else { LIGHT }
}

impl Palette {
    /// The color of `highlight`.
    pub(super) fn color(self, highlight: Highlight) -> Color32 {
        match highlight {
            Highlight::OperatorKeyword => self.operator_keyword,
            Highlight::UnitKeyword => self.unit_keyword,
            Highlight::Number => self.number,
            Highlight::Comment => self.comment,
            Highlight::Icon => self.icon,
        }
    }
}

/// 2026 Dark: `#BBBEBF` text on `#121314`, with the theme's token colors.
const DARK: Palette = Palette {
    background: Color32::from_rgb(0x12, 0x13, 0x14),
    plain: Color32::from_rgb(0xBB, 0xBE, 0xBF),
    // keyword
    operator_keyword: Color32::from_rgb(0xFF, 0x7B, 0x72),
    // constant
    unit_keyword: Color32::from_rgb(0x79, 0xC0, 0xFF),
    // constant
    number: Color32::from_rgb(0xB5, 0xCE, 0xA8),
    // comment
    comment: Color32::from_rgb(0x8B, 0x94, 0x9E),
    // string
    icon: Color32::from_rgb(0xA5, 0xD6, 0xFF),
};

/// 2026 Light: `#202020` text on `#FFFFFF`, with the theme's token colors.
const LIGHT: Palette = Palette {
    background: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    plain: Color32::from_rgb(0x20, 0x20, 0x20),
    // keyword
    operator_keyword: Color32::from_rgb(0xCF, 0x22, 0x2E),
    // constant
    unit_keyword: Color32::from_rgb(0x05, 0x50, 0xAE),
    // constant
    number: Color32::from_rgb(0x09, 0x86, 0x58),
    // comment
    comment: Color32::from_rgb(0x6E, 0x77, 0x81),
    // string
    icon: Color32::from_rgb(0x0A, 0x30, 0x69),
};
