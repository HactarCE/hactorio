use super::*;
use crate::app::results::{ResultsContext, Token};
use crate::document::Tab;
use egui_kittest::kittest::Queryable;

fn token_tip_is_open(harness: &Harness<'_, Option<HactorioApp>>) -> bool {
    harness.ctx.memory(|memory| {
        memory
            .layer_ids()
            .any(|layer| layer.order == egui::Order::Tooltip)
    })
}

#[test]
fn hovering_an_editor_glyph_opens_the_token_tip() {
    let text = "# Main\n3 :iron-plate:\n";
    let document = editor_document(text, 1, 0);
    let (mut harness, dir) = completion_harness("hover", document);
    harness.run();

    let rect = editor_text_rect(&harness, editor_rect(&harness), text);
    assert!(
        hover_until_token_tip(&mut harness, rect),
        "no token tip appeared"
    );
    if std::env::var_os("HACTORIO_UI_SNAPSHOT").is_some() {
        harness.snapshot("token_tip");
    }

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hovering_a_single_line_result_glyph_opens_the_token_tip() {
    let document = editor_document("# Main\n3 :iron-plate:\n", 0, 0);
    let (mut harness, dir) = completion_harness("output-hover", document);
    harness.run();

    let editor = editor_rect(&harness);
    // The results column starts just right of the editor, and the glyph sits in
    // the second row's projection, near its start.
    let rect = egui::Rect::from_min_size(
        egui::pos2(editor.right() + 4.0, editor.top()),
        egui::vec2(200.0, 60.0),
    );
    assert!(
        hover_until_token_tip(&mut harness, rect),
        "no token tip appeared"
    );

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hovering_a_multiline_result_glyph_opens_the_token_tip() {
    let document = editor_document("# Main\n3 :iron-plate:\n", 1, 0);
    let (mut harness, dir) = completion_harness("multiline-hover", document);
    harness.run();

    let area = multiline_result_rect(&harness);
    let rect = egui::Rect::from_min_size(area.min, egui::vec2(120.0, 40.0));
    assert!(
        hover_until_token_tip(&mut harness, rect),
        "no token tip appeared"
    );

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn multiline_result_content_starts_at_the_anchor() {
    let style = egui::Style::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0));
    let anchor = egui::pos2(400.0, 300.0);
    let pos = multiline_result_pos(&style, anchor, screen, 300.0);
    let content = pos + multiline_result_inset(&style);
    assert!((content.x - anchor.x).abs() < 0.01, "{content:?}");
    assert!((content.y - anchor.y).abs() < 0.01, "{content:?}");
    assert!(multiline_result_inset(&style).x > 0.0);
}

#[test]
fn multiline_result_wraps_a_long_error_message() {
    let unknown = "x".repeat(100);
    let text = format!("# Main\n:{unknown}:\n");
    let document = editor_document(&text, 1, 0);
    let (mut harness, dir) = completion_harness("multiline-wide", document);
    harness.run();

    let area = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::unique("multiline_result")))
        .expect("multiline result");
    assert!(area.width() > 420.0, "popup stayed narrow: {area:?}");
    // The message is prose, so it wraps rather than widening the panel.
    assert!(area.height() > 60.0, "message did not wrap: {area:?}");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn multiline_result_columns_do_not_wrap_at_large_font() {
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
    let (mut harness, dir) = completion_harness_with("multiline-columns", document, FIXTURE);
    harness.run();

    let area = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::unique("multiline_result")))
        .expect("multiline result");
    assert!(area.height() < 200.0, "multiline columns wrapped: {area:?}");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn token_tip_for_a_prototype_lists_its_information() {
    let registry = fixture_registry();
    let settings = Settings::default();
    let glyph = registry.entry("iron-plate").unwrap().glyph;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(500.0, 400.0))
        .with_accessibility_check(false)
        .allow_missing_glyphs()
        .build_ui(move |ui| {
            let results = ResultsContext {
                tabs: &[],
                active_tab: 0,
                outputs: &hactorio_core::eval::Evaluation::empty(),
                settings: &settings,
                registry: &registry,
            };
            results.token_tip_ui(
                ui,
                &Token::Prototype {
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
fn token_tip_for_a_variable_shows_its_value() {
    let registry = fixture_registry();
    let settings = Settings::default();
    let text = "# Main\nrate = 3 :iron-plate: / second\n";
    let tabs = vec![Tab::from_text(text)];
    let outputs = hactorio_core::eval::evaluate(text, &registry, &settings, &|| false);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(500.0, 400.0))
        .with_accessibility_check(false)
        .allow_missing_glyphs()
        .build_ui(move |ui| {
            let results = ResultsContext {
                tabs: &tabs,
                active_tab: 0,
                outputs: &outputs,
                settings: &settings,
                registry: &registry,
            };
            results.token_tip_ui(
                ui,
                &Token::Variable {
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
fn token_tip_for_a_variable_shows_the_latest_definition() {
    let registry = fixture_registry();
    let settings = Settings::default();
    let text = "# Main\nrate = 3 :iron-plate: / second\nrate = 5 :iron-plate: / second\n";
    let tabs = vec![Tab::from_text(text)];
    let outputs = hactorio_core::eval::evaluate(text, &registry, &settings, &|| false);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(500.0, 400.0))
        .with_accessibility_check(false)
        .allow_missing_glyphs()
        .build_ui(move |ui| {
            let results = ResultsContext {
                tabs: &tabs,
                active_tab: 0,
                outputs: &outputs,
                settings: &settings,
                registry: &registry,
            };
            results.token_tip_ui(
                ui,
                &Token::Variable {
                    line: 3,
                    name: "rate".to_string(),
                    rect: egui::Rect::NOTHING,
                },
            );
        });
    harness.run();

    assert!(harness.query_by_label_contains("rate = 5").is_some());
}

#[test]
fn multiline_result_shows_error_heading_and_message() {
    let document = editor_document("# Main\n1 + 3 :iron-plate:\n", 1, 0);
    let (mut harness, dir) = completion_harness("multiline-error", document);
    harness.run();
    assert!(harness.query_by_label("error").is_some());
    assert!(
        harness
            .query_by_label_contains("`+` dropped every facet")
            .is_some()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// The editor's text area, from the focused text field's response.
fn editor_rect(harness: &Harness<'_, Option<HactorioApp>>) -> egui::Rect {
    harness
        .ctx
        .memory(|memory| memory.focused())
        .and_then(|id| harness.ctx.read_response(id))
        .map(|response| response.rect)
        .expect("focused editor")
}

/// Sweeps `rect` until a token tip opens, returning whether one did.
///
/// The tip is created in the same frame the pointer move is handled, so a
/// single `step` per probe is enough; `run` would settle several extra frames
/// per position.
fn hover_until_token_tip(harness: &mut Harness<'_, Option<HactorioApp>>, rect: egui::Rect) -> bool {
    for y in (rect.top() as i32..rect.bottom() as i32).step_by(3) {
        for x in (rect.left() as i32..rect.right() as i32).step_by(2) {
            harness.hover_at(egui::pos2(x as f32, y as f32));
            harness.step();
            if token_tip_is_open(harness) {
                return true;
            }
        }
    }
    false
}

/// The part of an editor's response rect that holds the laid-out text.
///
/// The response rect spans the whole editor, most of which is empty; sweeping
/// it for a token is thousands of probes with nothing to hit. The galley starts
/// at the editor's margin and each character advances by the monospace glyph
/// width.
fn editor_text_rect(
    harness: &Harness<'_, Option<HactorioApp>>,
    editor: egui::Rect,
    text: &str,
) -> egui::Rect {
    let style = harness.ctx.style_of(harness.ctx.theme());
    let font = egui::TextStyle::Monospace.resolve(&style);
    let (row_height, char_width) = harness
        .ctx
        .fonts_mut(|fonts| (fonts.row_height(&font), fonts.glyph_width(&font, ' ')));
    let lines = text.lines().count().max(1);
    let columns = text
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    egui::Rect::from_min_size(
        editor.min + egui::vec2(4.0, 4.0),
        egui::vec2(columns as f32 * char_width, lines as f32 * row_height),
    )
}

fn multiline_result_rect(harness: &Harness<'_, Option<HactorioApp>>) -> egui::Rect {
    harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::unique("multiline_result")))
        .expect("multiline result")
}

#[test]
fn hovering_a_variable_opens_the_token_tip() {
    let text = "# Main\nrate = 3 :iron-plate:\nrate * 2\n";
    let document = editor_document(text, 2, 0);
    let (mut harness, dir) = completion_harness("variable-hover", document);
    harness.run();
    let rect = editor_text_rect(&harness, editor_rect(&harness), text);
    assert!(hover_until_token_tip(&mut harness, rect));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hovering_a_keyword_opens_no_token_tip() {
    let text = "# Main\nwith\n";
    let document = editor_document(text, 1, 0);
    let (mut harness, dir) = completion_harness("keyword-hover", document);
    harness.run();
    let rect = editor_text_rect(&harness, editor_rect(&harness), text);
    assert!(
        !hover_until_token_tip(&mut harness, rect),
        "a keyword opened a token tip"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hovering_a_glyph_in_the_multiline_result_keeps_every_line() {
    let document = editor_document("# Main\n2 :assembling-machine-3:\n", 1, 0);
    let (mut harness, dir) = completion_harness("multiline-lines", document);
    harness.run();

    let baseline = multiline_result_rect(&harness).height();
    assert!(baseline > 0.0);
    let rect = multiline_result_rect(&harness);
    assert!(
        hover_until_token_tip(&mut harness, rect),
        "no glyph in the multiline result"
    );
    let after = multiline_result_rect(&harness).height();
    assert!(
        (after - baseline).abs() < 1.0,
        "panel lost lines while a glyph was hovered: {baseline} -> {after}"
    );

    std::fs::remove_dir_all(dir).unwrap();
}
