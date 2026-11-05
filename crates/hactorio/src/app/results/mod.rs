//! Results surfaces: the single-line result painted in the results column, the
//! multiline result panel, and the token tip.
//!
//! Three surfaces show what a line or a token evaluates to:
//!
//! - the **results column** paints a **single-line result** per row;
//! - the **multiline result panel** replaces that row for the line the caret is
//!   on, floating beside the column and following the caret;
//! - the **token tip** describes the token under the pointer, wherever it
//!   appears: in the editor, in a single-line result, or in the panel.
//!
//! A tip follows the pointer; the panel follows the caret. `tip` always means an
//! `egui::Tooltip`, `panel` always means a floating `egui::Area`, and `popup` is
//! reserved for the completion dropdown.

use hactorio_core::Settings;
use hactorio_core::eval::{Evaluation, Output, format_value};
use hactorio_data::Registry;

mod multiline;
mod token_tip;

#[cfg(test)]
pub(super) use multiline::{multiline_result_inset, multiline_result_pos};

use crate::document::Tab;

/// A token the pointer can describe: a prototype's glyph or a variable name.
#[derive(Clone)]
pub(super) enum Token {
    Prototype {
        glyph: char,
        rect: egui::Rect,
    },
    Variable {
        line: usize,
        name: String,
        rect: egui::Rect,
    },
}

impl Token {
    pub(super) fn rect(&self) -> egui::Rect {
        match self {
            Token::Prototype { rect, .. } | Token::Variable { rect, .. } => *rect,
        }
    }

    fn tip_id(&self) -> egui::Id {
        match self {
            Token::Prototype { glyph, rect } => egui::Id::unique((
                "token_tip",
                "prototype",
                *glyph,
                rect.left_top().x.to_bits(),
                rect.left_top().y.to_bits(),
            )),
            Token::Variable { line, name, .. } => {
                egui::Id::unique(("token_tip", "variable", *line, name))
            }
        }
    }
}

/// The parts of the app the results surfaces read.
pub(super) struct ResultsContext<'a> {
    pub tabs: &'a [Tab],
    pub active_tab: usize,
    pub outputs: &'a Evaluation,
    pub settings: &'a Settings,
    pub registry: &'a Registry,
}

/// The prototype glyph under `pointer` in a laid-out text, with its row and rect.
pub(super) fn glyph_at(
    registry: &Registry,
    galley: &egui::Galley,
    origin: egui::Pos2,
    pointer: egui::Pos2,
) -> Option<(usize, char, egui::Rect)> {
    for (row_index, row) in galley.rows.iter().enumerate() {
        let top = origin.y + row.pos.y;
        if pointer.y < top || pointer.y > top + row.size.y {
            continue;
        }
        for glyph in &row.glyphs {
            if registry.entry_by_glyph(glyph.chr).is_none() {
                continue;
            }
            let rect = egui::Rect::from_min_size(
                egui::pos2(origin.x + row.pos.x + glyph.pos.x, top),
                egui::vec2(glyph.advance_width, row.size.y),
            );
            if rect.contains(pointer) {
                return Some((row_index, glyph.chr, rect));
            }
        }
    }
    None
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

/// The identifier under `pointer` in a laid-out text, with its row and rect.
pub(super) fn identifier_at(
    galley: &egui::Galley,
    origin: egui::Pos2,
    pointer: egui::Pos2,
) -> Option<(usize, String, egui::Rect)> {
    for (row_index, row) in galley.rows.iter().enumerate() {
        let top = origin.y + row.pos.y;
        if pointer.y < top || pointer.y > top + row.size.y {
            continue;
        }
        for (index, glyph) in row.glyphs.iter().enumerate() {
            let left = origin.x + row.pos.x + glyph.pos.x;
            if pointer.x < left || pointer.x > left + glyph.advance_width {
                continue;
            }
            if !is_identifier_char(glyph.chr) {
                return None;
            }
            let mut first = index;
            while first > 0 && is_identifier_char(row.glyphs[first - 1].chr) {
                first -= 1;
            }
            let mut last = index;
            while last + 1 < row.glyphs.len() && is_identifier_char(row.glyphs[last + 1].chr) {
                last += 1;
            }
            let word: String = row.glyphs[first..=last]
                .iter()
                .map(|glyph| glyph.chr)
                .collect();
            if !word
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
            {
                return None;
            }
            let start = row.glyphs[first].pos.x;
            let end = row.glyphs[last].pos.x + row.glyphs[last].advance_width;
            let rect = egui::Rect::from_min_max(
                egui::pos2(origin.x + row.pos.x + start, top),
                egui::pos2(origin.x + row.pos.x + end, top + row.size.y),
            );
            return Some((row_index, word, rect));
        }
    }
    None
}

/// The one-line projection of a line's result, or `None` if it shows nothing.
pub(super) fn single_line_result(
    output: &Output,
    settings: &Settings,
    registry: &Registry,
) -> Option<String> {
    match output {
        Output::Blank | Output::None => None,
        Output::Error { .. } => Some("error".to_string()),
        Output::Value(value) => Some(format_value(value, settings, registry)),
    }
}
