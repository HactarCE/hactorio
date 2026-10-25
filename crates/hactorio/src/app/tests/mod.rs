//! Tests for the document window, driven offscreen through `egui_kittest`.
//!
//! Shared fixtures and harness builders live here; the tests are split by
//! feature into submodules.

use crate::app::completion::*;
use crate::app::results::*;
use crate::app::*;
use crate::document::{Cursor, Document};
use egui_kittest::Harness;

mod clipboard;
mod completion;
mod hover;
mod keybinds;
mod layout;
mod persistence;
mod reload;
mod research;
mod snapshots;

fn sample_document() -> Document {
    let text = "# Main\n\
        rate = 3 :iron-plate: / second\n\
        100 :electronic-circuit: / second in (:assembling-machine-3: making :electronic-circuit:)\n\
        2 :electric-furnace: with 2 :productivity-module-3: making :iron-plate:\n\
        100 :iron-plate: in :electric-furnace:\n\
        :stack-size: :transport-belt: :iron-plate:\n\
        3 m + 4\n";
    Document {
        tabs: vec![text.to_string()],
        cursors: vec![Cursor {
            line: 3,
            column: 0,
            scroll: 0.0,
        }],
        ..Document::default()
    }
}

const FIXTURE: &str = r#"{
  "item": {
    "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100, "icon": "a.png"},
    "copper-cable": {"type": "item", "name": "copper-cable", "stack_size": 200, "icon": "b.png"},
    "assembling-machine-3": {"type": "item", "name": "assembling-machine-3", "stack_size": 50, "icon": "c.png"}
  },
  "fluid": {"water": {"type": "fluid", "name": "water", "icon": "d.png"}},
  "module": {"speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}}},
  "recipe": {
    "iron-plate": {"type": "recipe", "name": "iron-plate", "categories": ["smelting"], "results": [{"type": "item", "name": "iron-plate", "amount": 1}]},
    "electronic-circuit": {"type": "recipe", "name": "electronic-circuit", "categories": ["crafting"], "results": [{"type": "item", "name": "iron-plate", "amount": 1}]}
  },
  "assembling-machine": {
    "assembling-machine-3": {"type": "assembling-machine", "name": "assembling-machine-3", "crafting_categories": ["crafting"], "crafting_speed": 1.25, "module_slots": 4, "energy_usage": "375kW"}
  },
  "technology": {
    "iron-plate-productivity": {
      "type": "technology", "name": "iron-plate-productivity", "max_level": "infinite",
      "effects": [{"type": "change-recipe-productivity", "recipe": "iron-plate", "change": 0.1}]
    }
  }
}"#;

fn fixture_registry() -> Registry {
    hactorio_data::postprocess(FIXTURE, &hactorio_data::Locale::empty()).unwrap()
}

/// Builds a harness over the fixture registry, with the given cursor.
///
/// The returned directory holds the app's cache and must be removed by the test.
fn completion_harness(
    name: &str,
    document: Document,
) -> (Harness<'static, Option<HactorioApp>>, PathBuf) {
    completion_harness_with(name, document, FIXTURE)
}

fn completion_harness_with(
    name: &str,
    document: Document,
    fixture: &str,
) -> (Harness<'static, Option<HactorioApp>>, PathBuf) {
    harness_with(name, document, Some(fixture))
}

/// Builds a harness over an app directory that holds `fixture` as its cache.
///
/// The directory name is `name` plus the process id, so tests need unique
/// names; parallel tests that share one delete each other's directory.
///
/// `None` leaves the directory empty, as when game data has never been loaded.
fn harness_with(
    name: &str,
    document: Document,
    fixture: Option<&str>,
) -> (Harness<'static, Option<HactorioApp>>, PathBuf) {
    let app_dir =
        std::env::temp_dir().join(format!("hactorio-completion-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&app_dir);
    std::fs::create_dir_all(&app_dir).unwrap();
    if let Some(fixture) = fixture {
        hactorio_data::postprocess(fixture, &hactorio_data::Locale::empty())
            .unwrap()
            .save(&app_dir.join("cache.json"))
            .unwrap();
    }

    let builder = Harness::builder()
        .with_size(egui::vec2(800.0, 600.0))
        // Virtualized rows request a restyle the first time they scroll in.
        .with_max_steps(16)
        .with_accessibility_check(false)
        .allow_missing_glyphs();
    let builder = if std::env::var_os("HACTORIO_UI_SNAPSHOT").is_some() {
        builder.wgpu()
    } else {
        builder
    };
    let harness = builder.build_ui_state(
        {
            let app_dir = app_dir.clone();
            move |ui, app: &mut Option<HactorioApp>| {
                let app = app.get_or_insert_with(|| {
                    HactorioApp::new(ui.ctx(), document.clone(), app_dir.clone())
                });
                app.ui(ui);
            }
        },
        None,
    );
    (harness, app_dir)
}

fn editor_document(text: &str, line: usize, column: usize) -> Document {
    Document {
        tabs: vec![text.to_string()],
        cursors: vec![Cursor {
            line,
            column,
            scroll: 0.0,
        }],
        ..Document::default()
    }
}

fn type_into_editor(harness: &mut Harness<'_, Option<HactorioApp>>, text: &str) {
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(text.to_string()));
    harness.run();
}
