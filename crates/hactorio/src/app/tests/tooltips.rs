use super::*;
use crate::app::detail::{DetailContext, HoverTarget};
use crate::document::Tab;
use egui_kittest::kittest::Queryable;
use hactorio_core::facet_eval::evaluate;

fn tooltip_is_open(harness: &Harness<'_, Option<HactorioApp>>) -> bool {
    harness.ctx.memory(|memory| {
        memory
            .layer_ids()
            .any(|layer| layer.order == egui::Order::Tooltip)
    })
}

#[test]
fn hovering_an_editor_icon_opens_the_details() {
    let document = editor_document("# Main\n3 :iron-plate:\n", 1, 0);
    let (mut harness, dir) = completion_harness("hover", document);
    harness.run();

    let rect = harness
        .ctx
        .memory(|memory| memory.focused())
        .and_then(|id| harness.ctx.read_response(id))
        .map(|response| response.rect)
        .expect("focused editor");

    let left = rect.left() as i32;
    let top = rect.top() as i32;
    let mut shown = false;
    'search: for y in (top..=top + 60).step_by(3) {
        for x in (left..=left + 100).step_by(2) {
            harness.hover_at(egui::pos2(x as f32, y as f32));
            harness.run();
            if tooltip_is_open(&harness) {
                shown = true;
                break 'search;
            }
        }
    }
    assert!(shown, "no details tooltip appeared");
    if std::env::var_os("HACTORIO_UI_SNAPSHOT").is_some() {
        harness.snapshot("details");
    }

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hovering_an_output_icon_opens_the_details() {
    let document = editor_document("# Main\n3 :iron-plate:\n", 0, 0);
    let (mut harness, dir) = completion_harness("output-hover", document);
    harness.run();

    let editor = harness
        .ctx
        .memory(|memory| memory.focused())
        .and_then(|id| harness.ctx.read_response(id))
        .map(|response| response.rect)
        .expect("focused editor");
    let left = editor.right() as i32 + 4;
    let top = editor.top() as i32;
    let mut shown = false;
    'search: for y in (top..=top + 60).step_by(3) {
        for x in (left..=left + 300).step_by(4) {
            harness.hover_at(egui::pos2(x as f32, y as f32));
            harness.run();
            if tooltip_is_open(&harness) {
                shown = true;
                break 'search;
            }
        }
    }
    assert!(shown, "no details tooltip appeared");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hovering_a_detail_popup_icon_opens_the_details() {
    let document = editor_document("# Main\n3 :iron-plate:\n", 1, 0);
    let (mut harness, dir) = completion_harness("detail-hover", document);
    harness.run();

    let area = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::unique("line_detail")))
        .expect("detail popup");
    let left = area.left() as i32;
    let top = area.top() as i32;
    let mut shown = false;
    'search: for y in (top..=top + 40).step_by(3) {
        for x in (left..=left + 120).step_by(3) {
            harness.hover_at(egui::pos2(x as f32, y as f32));
            harness.run();
            if tooltip_is_open(&harness) {
                shown = true;
                break 'search;
            }
        }
    }
    assert!(shown, "no details tooltip appeared");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn detail_popup_content_starts_at_the_anchor() {
    let style = egui::Style::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0));
    let anchor = egui::pos2(400.0, 300.0);
    let pos = detail_popup_pos(&style, anchor, screen, 300.0);
    let content = pos + detail_popup_inset(&style);
    assert!((content.x - anchor.x).abs() < 0.01, "{content:?}");
    assert!((content.y - anchor.y).abs() < 0.01, "{content:?}");
    assert!(detail_popup_inset(&style).x > 0.0);
}

#[test]
fn detail_popup_does_not_wrap_long_results() {
    let unknown = "x".repeat(100);
    let text = format!("# Main\n:{unknown}:\n");
    let document = editor_document(&text, 1, 0);
    let (mut harness, dir) = completion_harness("detail-wide", document);
    harness.run();

    let area = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::unique("line_detail")))
        .expect("detail popup");
    assert!(area.width() > 420.0, "popup stayed narrow: {area:?}");
    assert!(area.height() < 60.0, "popup wrapped: {area:?}");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn detail_columns_do_not_wrap_at_large_font() {
    const FIXTURE: &str = r#"{
  "item": {
    "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100},
    "iron-gear-wheel": {"type": "item", "name": "iron-gear-wheel", "stack_size": 100},
    "assembling-machine-3": {"type": "item", "name": "assembling-machine-3", "stack_size": 50},
    "speed-module-3": {"type": "item", "name": "speed-module-3", "stack_size": 50}
  },
  "module": {
    "speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed",
      "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}}
  },
  "recipe": {
    "iron-gear-wheel": {"type": "recipe", "name": "iron-gear-wheel", "categories": ["crafting"],
      "energy_required": 0.5,
      "ingredients": [{"type": "item", "name": "iron-plate", "amount": 2}],
      "results": [{"type": "item", "name": "iron-gear-wheel", "amount": 1}],
      "allow_productivity": true}
  },
  "assembling-machine": {
    "assembling-machine-3": {"type": "assembling-machine", "name": "assembling-machine-3",
      "crafting_categories": ["crafting"], "crafting_speed": 1.25, "module_slots": 4,
      "allowed_effects": ["speed", "productivity", "consumption", "pollution"],
      "energy_usage": "375kW"}
  }
}"#;
    let text = ":transport-belt: :iron-plate: in (:assembling-machine-3: with 2 :speed-module-3: making :iron-gear-wheel:)";
    let mut document = editor_document(&format!("# prod test\n{text}\n"), 1, 0);
    document.font_size = 24.0;
    document.settings.item_rate_belt = Some(hactorio_core::BeltTier::Yellow);
    let (mut harness, dir) = completion_harness_with("detail-columns", document, FIXTURE);
    harness.run();

    let area = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::unique("line_detail")))
        .expect("detail popup");
    assert!(area.height() < 200.0, "detail columns wrapped: {area:?}");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn details_for_a_prototype_list_its_information() {
    let registry = fixture_registry();
    let settings = Settings::default();
    let glyph = registry.entry("iron-plate").unwrap().glyph;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(500.0, 400.0))
        .with_accessibility_check(false)
        .allow_missing_glyphs()
        .build_ui(move |ui| {
            let details = DetailContext {
                tabs: &[],
                active: 0,
                outputs: &[],
                settings: &settings,
                registry: &registry,
            };
            details.details_ui(
                ui,
                &HoverTarget::Entry {
                    glyph,
                    rect: egui::Rect::NOTHING,
                },
            );
        });
    harness.run();

    assert!(harness.query_by_label("stack size").is_some());
    assert!(harness.query_by_label("craft time").is_some());
    assert!(harness.query_by_label("item").is_some());
    assert!(harness.query_by_label("recipe").is_some());
    assert!(harness.query_by_label("id").is_some());
    assert!(harness.query_by_label_contains("products").is_some());
    assert!(
        harness
            .query_all_by_label_contains("iron-plate")
            .next()
            .is_some()
    );
}

#[test]
fn details_for_a_variable_show_its_value() {
    let registry = fixture_registry();
    let settings = Settings::default();
    let text = "# Main\nrate = 3 :iron-plate: / second\n";
    let tabs = vec![Tab::from_text(text)];
    let outputs = evaluate(text, &settings, &registry);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(500.0, 400.0))
        .with_accessibility_check(false)
        .allow_missing_glyphs()
        .build_ui(move |ui| {
            let details = DetailContext {
                tabs: &tabs,
                active: 0,
                outputs: &outputs,
                settings: &settings,
                registry: &registry,
            };
            details.details_ui(
                ui,
                &HoverTarget::Variable {
                    line: 2,
                    name: "rate".to_string(),
                    rect: egui::Rect::NOTHING,
                },
            );
        });
    harness.run();

    assert!(harness.query_by_label_contains("rate = 3").is_some());
}

#[test]
fn error_details_show_error_heading_and_message() {
    let document = editor_document("# Main\n3 :iron-plate: + 3 :copper-cable:\n", 1, 0);
    let (mut harness, dir) = completion_harness("error-details", document);
    harness.run();
    assert!(harness.query_by_label("error").is_some());
    assert!(
        harness
            .query_by_label_contains("`+` dropped every facet")
            .is_some()
    );
    std::fs::remove_dir_all(dir).unwrap();
}
