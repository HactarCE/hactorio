//! Translates clipboard text between plaintext `:prototype-id:` tokens and
//! private-use glyphs.
//!
//! egui has no copy/paste hook on `TextEdit`, so translation happens at the two
//! data seams: the events entering a frame, and the clipboard commands leaving
//! it. Both are global, so every text field gets the same treatment.

use eframe::egui::{self, Event};
use hactorio_core::{from_plaintext, to_plaintext};
use hactorio_data::Registry;

/// Replaces tokens in pasted text with glyphs for this frame's widgets.
pub(crate) fn translate_input(input: &mut egui::InputState, registry: &Registry) {
    for event in &mut input.events {
        match event {
            Event::Paste(text) | Event::MiddleClickPaste { text, .. } => {
                if text.contains(':') {
                    *text = from_plaintext(text, registry);
                }
            }
            // Screen readers paste through `ReplaceSelectedText`, which `TextEdit`
            // inserts without going through `Event::Paste`.
            Event::AccessKitActionRequest(request) => {
                if request.action == egui::accesskit::Action::ReplaceSelectedText
                    && let Some(egui::accesskit::ActionData::Value(text)) = &mut request.data
                    && text.contains(':')
                {
                    *text = from_plaintext(text, registry).into_boxed_str();
                }
            }
            _ => {}
        }
    }
}

/// Replaces glyphs in copied selections with their plaintext token form.
pub(crate) fn translate_output(output: &mut egui::PlatformOutput, registry: &Registry) {
    for command in &mut output.commands {
        match command {
            egui::OutputCommand::CopyText(text)
            | egui::OutputCommand::TextSelectionSettled(text) => {
                *text = to_plaintext(text, registry);
            }
            _ => {}
        }
    }
}
