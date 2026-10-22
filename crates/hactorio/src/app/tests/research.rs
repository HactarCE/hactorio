use super::*;
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn productivity_menu_lists_research_and_affected_recipes() {
    let mut document = editor_document("# Main\n", 0, 0);
    document
        .settings
        .research_levels
        .insert("iron-plate-productivity".into(), 3);
    let (mut harness, dir) = completion_harness("research-menu", document);
    harness.run();

    harness.get_by_label("Productivity…").click();
    harness.run();

    harness.get_by_label("iron-plate-productivity");
    let glyph = fixture_registry().entry("iron-plate").unwrap().glyph;
    let text = format!("+30% {glyph} iron-plate");
    harness.get_by_label(&text);

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn productivity_popup_resizes_without_wrapping_when_the_bonus_changes() {
    const RECIPE: &str = "short-recipe";
    // A huge gain makes the row wider than the popup's 260 px minimum, which is
    // the case where the cached popup width used to wrap the text. It is then
    // reduced again to check that the popup also shrinks back.
    let fixture = r#"{
  "item": {"iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100}},
  "recipe": {
    "short-recipe": {
      "type": "recipe", "name": "short-recipe", "categories": ["crafting"],
      "results": [{"type": "item", "name": "iron-plate", "amount": 1}]
    }
  },
  "technology": {
    "big-productivity": {
      "type": "technology", "name": "big-productivity", "max_level": "infinite",
      "effects": [{"type": "change-recipe-productivity", "recipe": "short-recipe", "change": 1e30}]
    }
  }
}"#;
    let document = editor_document("# Main\n", 0, 0);
    let (mut harness, dir) = completion_harness_with("research-resize", document, fixture);
    harness.run();

    harness.get_by_label("Productivity…").click();
    harness.run();

    let glyph = hactorio_data::postprocess(fixture, &hactorio_data::Locale::empty())
        .unwrap()
        .entry(RECIPE)
        .unwrap()
        .glyph;
    let before_text = format!("+0% {glyph} {RECIPE}");
    let before_label = harness.get_by_label(&before_text);
    let before = before_label.rect();
    let popup_before = popup_rect(&harness, before.center());

    let drag = before_label
        .parent()
        .expect("recipe list parent")
        .children_recursive()
        .find(|node| node.accesskit_node().role() == egui::accesskit::Role::SpinButton)
        .expect("research level drag value");
    drag.set_value_accesskit(1_000.0);
    harness.run();

    let percent = (1e30_f64 * 1_000.0 * 100.0 * 1e6).round() / 1e6;
    let after_text = format!("+{percent}% {glyph} {RECIPE}");
    let after_label = harness.get_by_label(&after_text);
    let after = after_label.rect();
    let popup_after = popup_rect(&harness, after.center());
    assert!(after.width() > before.width(), "{before:?} -> {after:?}");
    assert!(
        popup_after.width() > popup_before.width(),
        "{popup_before:?} -> {popup_after:?}"
    );
    assert!(
        after.height() < before.height() * 1.5,
        "label wrapped: {before:?} -> {after:?}"
    );

    let drag = after_label
        .parent()
        .expect("recipe list parent")
        .children_recursive()
        .find(|node| node.accesskit_node().role() == egui::accesskit::Role::SpinButton)
        .expect("research level drag value");
    drag.set_value_accesskit(0.0);
    harness.run();

    let back_label = harness.get_by_label(&before_text);
    let back_popup = popup_rect(&harness, back_label.rect().center());
    assert!(
        back_popup.width() <= popup_before.width() + 1.0,
        "popup stayed wide: {popup_before:?} -> {back_popup:?}"
    );

    std::fs::remove_dir_all(dir).unwrap();
}

fn popup_rect(harness: &Harness<'_, Option<HactorioApp>>, point: egui::Pos2) -> egui::Rect {
    harness.ctx.memory(|memory| {
        memory
            .layer_ids()
            .filter(|layer| layer.order == egui::Order::Foreground)
            .filter_map(|layer| memory.area_rect(layer.id))
            .find(|rect| rect.contains(point))
            .expect("popup area under the label")
    })
}
