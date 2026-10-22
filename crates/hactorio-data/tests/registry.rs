use std::fs;
use std::path::PathBuf;

use hactorio_data::{Locale, PUA_FIRST, PrototypeKind, postprocess};

const DUMP: &str = include_str!("fixtures/dump.json");

fn registry() -> hactorio_data::Registry {
    postprocess(DUMP, &Locale::empty()).unwrap()
}

fn names(machine_categories: &[Box<str>]) -> Vec<&str> {
    machine_categories.iter().map(AsRef::as_ref).collect()
}

#[test]
fn entries_are_sorted_and_glyphs_are_contiguous() {
    let registry = registry();
    let ids: Vec<&str> = registry.entries().iter().map(|e| e.id.as_ref()).collect();
    assert_eq!(
        ids,
        [
            "advanced-circuit",
            "assembling-machine-3",
            "coal-liquefaction",
            "electric-mining-drill",
            "firearm-magazine",
            "iron-ore",
            "iron-plate",
            "speed-module-3",
            "uranium-235",
            "uranium-238",
            "uranium-processing",
            "water",
            "water-treatment",
        ]
    );
    for (index, entry) in registry.entries().iter().enumerate() {
        assert_eq!(entry.glyph as u32, PUA_FIRST + index as u32);
    }
}

#[test]
fn shared_name_is_one_entry_resolving_as_item_and_recipe() {
    let registry = registry();
    let entry = registry.entry("iron-plate").unwrap();

    assert_eq!(entry.item.as_ref().unwrap().stack_size, 100);
    assert_eq!(entry.display_name, None);
    // The item icon wins over the recipe icon.
    assert_eq!(
        entry.icon.as_deref(),
        Some("__base__/graphics/icons/iron-plate.png")
    );

    let recipe_id = entry.recipe.unwrap();
    let recipe = registry.recipe(recipe_id);
    assert_eq!(&*recipe.id, "iron-plate");
    assert_eq!(names(&recipe.categories), ["smelting"]);
    assert_eq!(recipe.craft_time, 3.2);
    assert!(recipe.allow_productivity);
    assert!(!recipe.synthetic);
}

#[test]
fn recipe_only_name_uses_the_recipe_icon() {
    let registry = registry();
    let entry = registry.entry("advanced-circuit").unwrap();

    assert!(entry.item.is_none());
    assert!(!entry.fluid);
    assert!(entry.recipe.is_some());
    assert_eq!(
        entry.icon.as_deref(),
        Some("__base__/graphics/icons/advanced-circuit.png")
    );
}

#[test]
fn non_item_prototype_types_are_still_items() {
    let registry = registry();
    let entry = registry.entry("firearm-magazine").unwrap();

    assert_eq!(entry.item.as_ref().unwrap().stack_size, 200);
    assert_eq!(
        entry.icon.as_deref(),
        Some("__base__/graphics/icons/firearm-magazine.png")
    );
    // Guns and ammo are not machines.
    assert!(entry.machine().is_none());
}

#[test]
fn blueprint_parameters_are_excluded() {
    let registry = registry();
    assert!(registry.entry("parameter-0").is_none());
    assert!(registry.find_recipe("parameter-0").is_none());
}

#[test]
fn modules_are_namespace_entries() {
    let registry = registry();
    let module = registry
        .entry("speed-module-3")
        .unwrap()
        .module
        .as_ref()
        .unwrap();

    assert_eq!(&*module.category, "speed");
    assert_eq!(module.tier, 3);
    assert_eq!(module.stack_size, 50);
    assert_eq!(module.effect.speed, 0.5);
    assert_eq!(module.effect.consumption, 0.7);
    assert_eq!(module.effect.quality, -0.025);
    assert_eq!(module.effect.productivity, 0.0);
}

#[test]
fn machine_joins_the_item_with_its_entity() {
    let registry = registry();
    let machine = registry
        .entry("assembling-machine-3")
        .unwrap()
        .machine()
        .unwrap();

    assert_eq!(machine.speed, 1.25);
    assert_eq!(machine.module_slots, 4);
    assert_eq!(
        machine.allowed_effects,
        hactorio_data::AllowedEffects {
            speed: true,
            productivity: true,
            ..Default::default()
        }
    );
    assert_eq!(machine.energy_usage_w, 375_000.0);
    assert_eq!(machine.pollution_per_minute, 2.0);
    assert_eq!(
        names(&machine.categories),
        ["crafting", "advanced-crafting"]
    );
}

#[test]
fn drills_are_machines_with_mining_speed() {
    let registry = registry();
    let machine = registry
        .entry("electric-mining-drill")
        .unwrap()
        .machine()
        .unwrap();

    assert_eq!(machine.speed, 0.5);
    assert_eq!(machine.module_slots, 3);
    assert_eq!(machine.energy_usage_w, 90_000.0);
    assert_eq!(machine.allowed_effects, Default::default());
    assert_eq!(names(&machine.categories), ["basic-solid"]);
}

#[test]
fn minable_resources_get_synthetic_recipes() {
    let registry = registry();
    let entry = registry.entry("iron-ore").unwrap();
    let recipe = registry.recipe(entry.recipe.unwrap());

    assert_eq!(&*recipe.id, "iron-ore");
    assert!(recipe.synthetic);
    assert_eq!(recipe.craft_time, 1.0);
    assert_eq!(names(&recipe.categories), ["basic-solid"]);
    assert!(recipe.ingredients.is_empty());
    assert_eq!(recipe.products.len(), 1);
    assert_eq!(recipe.products[0].kind, PrototypeKind::Item);
    assert_eq!(&*recipe.products[0].name, "iron-ore");
    assert_eq!(recipe.products[0].amount, 1.0);
}

#[test]
fn dump_recipes_win_over_synthetic_ones() {
    let registry = registry();
    let recipe = registry.find_recipe("iron-plate").unwrap();
    assert!(!recipe.synthetic);
}

#[test]
fn product_amounts_fold_in_probability_and_extra_counts() {
    let registry = registry();

    let uranium = registry.find_recipe("uranium-processing").unwrap();
    let expected: Vec<f64> = uranium.products.iter().map(|p| p.amount).collect();
    assert_eq!(expected, [0.007, 0.993]);

    let liquefaction = registry.find_recipe("coal-liquefaction").unwrap();
    assert_eq!(liquefaction.products[0].amount, 90.0);
    assert_eq!(liquefaction.products[0].ignored_by_productivity, 25.0);
    assert_eq!(liquefaction.products[1].amount, 0.25 * 1.0 + 0.5);
    assert_eq!(liquefaction.products[1].ignored_by_productivity, 0.0);
}

#[test]
fn iconless_recipes_fall_back_to_their_product_icon() {
    let registry = registry();
    let entry = registry.entry("water-treatment").unwrap();

    assert!(entry.item.is_none());
    assert!(entry.recipe.is_some());
    assert_eq!(
        entry.icon.as_deref(),
        Some("__base__/graphics/icons/fluid/water.png")
    );
}

#[test]
fn glyphs_resolve_back_to_entries() {
    let registry = registry();
    let entry = registry.entry("iron-ore").unwrap();

    assert_eq!(registry.entry_by_glyph(entry.glyph).unwrap().id, entry.id);
    assert!(registry.entry_by_glyph('a').is_none());
    assert!(
        registry
            .entry_by_glyph(char::from_u32(PUA_FIRST + 1000).unwrap())
            .is_none()
    );
}

#[test]
fn postprocessing_is_deterministic() {
    let first = registry();
    let second = registry();
    assert_eq!(first.entries(), second.entries());
    assert_eq!(first.recipes(), second.recipes());
}

#[test]
fn locale_supplies_display_names() {
    let dir = temp_dir("locale");
    fs::write(
        dir.join("item-locale.json"),
        r#"{"names":{"iron-plate":"Iron plate"}}"#,
    )
    .unwrap();
    fs::write(
        dir.join("recipe-locale.json"),
        r#"{"names":{"advanced-circuit":"Advanced circuit"}}"#,
    )
    .unwrap();

    let registry = postprocess(DUMP, &Locale::load(&dir)).unwrap();

    assert_eq!(
        registry
            .entry("iron-plate")
            .unwrap()
            .display_name
            .as_deref(),
        Some("Iron plate")
    );
    assert_eq!(
        registry
            .entry("advanced-circuit")
            .unwrap()
            .display_name
            .as_deref(),
        Some("Advanced circuit")
    );
    // A missing locale file is not an error.
    assert_eq!(registry.entry("water").unwrap().display_name, None);

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reference_counts_track_recipe_usage() {
    let registry = registry();
    let count = |id: &str| registry.reference_count(registry.entry_id(id).unwrap());

    // Two recipe ingredients/products plus the synthetic mining recipe.
    assert_eq!(count("iron-ore"), 3);
    assert_eq!(count("iron-plate"), 2);
    assert_eq!(count("uranium-235"), 2);
    assert_eq!(count("advanced-circuit"), 1);
    assert_eq!(count("water"), 1);
    // Only referenced as a recipe in this dump.
    assert_eq!(count("uranium-processing"), 0);
    assert_eq!(count("assembling-machine-3"), 0);
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hactorio-data-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}
