use super::*;

fn tooltip_is_open(harness: &Harness<'_, Option<HactorioApp>>) -> bool {
    harness.ctx.memory(|memory| {
        memory
            .layer_ids()
            .any(|layer| layer.order == egui::Order::Tooltip)
    })
}

#[test]
fn hovering_an_editor_icon_opens_the_breakdown() {
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
    assert!(shown, "no breakdown tooltip appeared");
    if std::env::var_os("HACTORIO_UI_SNAPSHOT").is_some() {
        harness.snapshot("breakdown");
    }

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hovering_an_output_icon_opens_the_breakdown() {
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
    assert!(shown, "no breakdown tooltip appeared");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hovering_a_detail_popup_icon_opens_the_breakdown() {
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
    assert!(shown, "no breakdown tooltip appeared");

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
