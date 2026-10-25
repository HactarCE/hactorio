//! Clipboard translation between plaintext tokens and glyphs.

use super::*;

fn iron_plate_glyph() -> char {
    fixture_registry()
        .entry("iron-plate")
        .expect("iron-plate in fixture")
        .glyph
}

#[test]
fn pasting_a_token_inserts_its_glyph() {
    let document = editor_document("# Main\n", 1, 0);
    let (mut harness, dir) = completion_harness("paste-token", document);
    harness.run();

    harness
        .input_mut()
        .events
        .push(egui::Event::Paste(":iron-plate:".to_string()));
    harness.run();

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, format!("# Main\n{}", iron_plate_glyph()));

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn pasting_an_unknown_token_leaves_it_alone() {
    let document = editor_document("# Main\n", 1, 0);
    let (mut harness, dir) = completion_harness("paste-unknown", document);
    harness.run();

    harness
        .input_mut()
        .events
        .push(egui::Event::Paste(":not-a-prototype: text".to_string()));
    harness.run();

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, "# Main\n:not-a-prototype: text");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn accesskit_replacement_inserts_a_glyph() {
    let document = editor_document("# Main\n", 1, 0);
    let (mut harness, dir) = completion_harness("accesskit-paste", document);
    harness.run();

    let id = harness
        .ctx
        .memory(|memory| memory.focused())
        .expect("editor");
    harness
        .input_mut()
        .events
        .push(egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::ReplaceSelectedText,
                target_tree: egui::accesskit::TreeId::ROOT,
                target_node: id.accesskit_id(),
                data: Some(egui::accesskit::ActionData::Value(":iron-plate:".into())),
            },
        ));
    harness.run();

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, format!("# Main\n{}", iron_plate_glyph()));

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn copying_a_glyph_copies_its_token() {
    let document = editor_document("# Main\n:iron-plate:\n", 1, 0);
    let (mut harness, dir) = completion_harness("copy-token", document);
    harness.run();

    let text = harness.state().as_ref().expect("app").tabs[0].text.clone();
    assert_eq!(text, format!("# Main\n{}\n", iron_plate_glyph()));

    let id = harness
        .ctx
        .memory(|memory| memory.focused())
        .expect("editor");
    let mut state = egui::text_edit::TextEditState::load(&harness.ctx, id).expect("text state");
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(text.chars().count()),
        )));
    state.store(&harness.ctx, id);

    harness.input_mut().events.push(egui::Event::Copy);
    harness.step();

    let copied = harness
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .expect("copy command");
    assert_eq!(copied, "# Main\n:iron-plate:\n");

    std::fs::remove_dir_all(dir).unwrap();
}
