use super::*;
use egui::{Key, Modifiers};

#[test]
fn typing_an_open_paren_wraps_the_selection() {
    let document = editor_document("# Main\nab\n", 1, 2);
    let (mut harness, dir) = completion_harness("wrap-paren", document);
    harness.run();

    harness.key_press_modifiers(Modifiers::SHIFT, Key::ArrowLeft);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::ArrowLeft);
    harness.run();

    type_into_editor(&mut harness, "(");

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, "# Main\n(ab)\n");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn typing_an_open_paren_without_a_selection_inserts_it() {
    let document = editor_document("# Main\nab\n", 1, 2);
    let (mut harness, dir) = completion_harness("plain-paren", document);
    harness.run();

    type_into_editor(&mut harness, "(");

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, "# Main\nab(\n");

    std::fs::remove_dir_all(dir).unwrap();
}
