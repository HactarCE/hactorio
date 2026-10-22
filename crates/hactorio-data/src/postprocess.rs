//! Turns Factorio's `--dump-data` output into a [`Registry`].

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

use crate::registry::{
    AllowedEffects, Entry, Ingredient, Item, Machine, Module, ModuleEffect, Product,
    ProductivityResearch, PrototypeKind, Recipe, RecipeId, RecipeProductivity, Registry, glyph_for,
};
use crate::{Error, Result};

/// Localised prototype names, keyed by prototype id.
///
/// Every table is optional: a cache built without locale data simply has no
/// display names.
#[derive(Debug, Clone, Default)]
pub struct Locale {
    items: HashMap<Box<str>, Box<str>>,
    fluids: HashMap<Box<str>, Box<str>>,
    recipes: HashMap<Box<str>, Box<str>>,
    technologies: HashMap<Box<str>, Box<str>>,
}

impl Locale {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Reads `item-locale.json`, `fluid-locale.json`, and `recipe-locale.json`
    /// from `dir`. Missing or unreadable files are ignored.
    pub fn load(dir: &Path) -> Self {
        Locale {
            items: read_names(&dir.join("item-locale.json")),
            fluids: read_names(&dir.join("fluid-locale.json")),
            recipes: read_names(&dir.join("recipe-locale.json")),
            technologies: read_names(&dir.join("technology-locale.json")),
        }
    }
}

/// Post-processes a raw `data-raw-dump.json` into a registry.
///
/// Only normal-quality gameplay data is represented: the dump has no quality
/// variants to filter, and quality is a runtime modifier.
///
/// Blueprint parameter prototypes (`parameter-N`) are excluded, and minable
/// resources get a synthetic recipe so that mining drills can be used as
/// machines.
pub fn postprocess(raw_dump_json: &str, locale: &Locale) -> Result<Registry> {
    let root: Value = serde_json::from_str(raw_dump_json)?;
    let root = root
        .as_object()
        .ok_or_else(|| Error::Invalid("dump root is not an object".into()))?;

    let machines = collect_machines(root);
    let has_item = |name: &str| has_prototype(root, "item", name);
    let has_fluid = |name: &str| has_prototype(root, "fluid", name);

    let mut recipes = collect_recipes(root, locale);
    collect_resource_recipes(root, &mut recipes, &has_item, &has_fluid);

    let recipe_ids: HashMap<Box<str>, RecipeId> = recipes
        .keys()
        .enumerate()
        .map(|(i, id)| (id.clone(), RecipeId::from_index(i)))
        .collect();

    let mut drafts: BTreeMap<Box<str>, Draft> = BTreeMap::new();

    // Items are split across several prototype types in 2.x (`item`, `ammo`,
    // `gun`, `armor`, `capsule`, ...). Anything with a stack size can be held,
    // so it belongs in the namespace; modules are handled separately below.
    for (table_name, table_value) in root {
        if matches!(
            table_name.as_str(),
            "fluid" | "recipe" | "resource" | "module"
        ) {
            continue;
        }
        let Some(prototypes) = table_value.as_object() else {
            continue;
        };
        for (id, proto) in prototypes {
            if is_parameter(proto) || proto.get("stack_size").is_none() {
                continue;
            }
            let machine = machines.get(id.as_str()).cloned();
            let draft = drafts.entry(id.as_str().into()).or_default();
            draft.display_name = locale.items.get(id.as_str()).cloned();
            draft.icon = icon_path(proto);
            draft.item = Some(Item {
                stack_size: uint(proto, "stack_size", 1) as u32,
                machine,
            });
        }
    }

    for (id, proto) in table(root, "fluid") {
        if is_parameter(proto) {
            continue;
        }
        let draft = drafts.entry(id.into()).or_default();
        draft.display_name = locale.fluids.get(id).cloned();
        draft.icon = icon_path(proto);
        draft.fluid = true;
    }

    for (id, proto) in table(root, "module") {
        let draft = drafts.entry(id.into()).or_default();
        draft.icon = icon_path(proto);
        draft.module = Some(Module {
            category: proto
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            tier: uint(proto, "tier", 1) as u32,
            stack_size: uint(proto, "stack_size", 1) as u32,
            effect: module_effect(proto),
        });
    }

    for (id, recipe) in &recipes {
        let icon = recipe.icon.clone().or_else(|| {
            let product = recipe.products.first()?;
            drafts.get(product.name.as_ref())?.icon.clone()
        });
        let draft = drafts.entry(id.clone()).or_default();
        draft.recipe_display_name = recipe.display_name.clone();
        draft.recipe_icon = icon;
    }

    let mut entries = Vec::with_capacity(drafts.len());
    for (index, (id, draft)) in drafts.into_iter().enumerate() {
        let glyph = glyph_for(index).ok_or_else(|| {
            Error::Invalid(format!(
                "namespace has more than {} entries",
                crate::PUA_COUNT
            ))
        })?;
        let recipe = recipe_ids.get(&id).copied();
        entries.push(Entry {
            id,
            glyph,
            display_name: draft.display_name.or(draft.recipe_display_name),
            icon: draft.icon.or(draft.recipe_icon),
            item: draft.item,
            fluid: draft.fluid,
            module: draft.module,
            recipe,
        });
    }

    let researches = collect_productivity_researches(root, locale);
    Registry::from_parts(entries, recipes.into_values().collect(), researches)
}

/// Collects technologies whose effects change recipe productivity.
///
/// Only `change-recipe-productivity` effects are kept: mining and laboratory
/// productivity are different effect types and do not target crafting recipes.
fn collect_productivity_researches(
    root: &Map<String, Value>,
    locale: &Locale,
) -> Vec<ProductivityResearch> {
    let mut researches = BTreeMap::new();
    for (id, proto) in table(root, "technology") {
        let mut effects = Vec::new();
        for effect in proto
            .get("effects")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if effect.get("type").and_then(Value::as_str) != Some("change-recipe-productivity") {
                continue;
            }
            let Some(recipe) = effect.get("recipe").and_then(Value::as_str) else {
                continue;
            };
            let change = effect.get("change").and_then(Value::as_f64).unwrap_or(0.0);
            if change == 0.0 {
                continue;
            }
            effects.push(RecipeProductivity {
                recipe: recipe.into(),
                change,
            });
        }
        if effects.is_empty() {
            continue;
        }
        researches.insert(
            id,
            ProductivityResearch {
                id: id.into(),
                display_name: locale.technologies.get(id).cloned(),
                effects,
            },
        );
    }
    researches.into_values().collect()
}

#[derive(Default)]
struct Draft {
    item: Option<Item>,
    fluid: bool,
    module: Option<Module>,
    display_name: Option<Box<str>>,
    icon: Option<Box<str>>,
    recipe_display_name: Option<Box<str>>,
    recipe_icon: Option<Box<str>>,
}

fn table<'a>(root: &'a Map<String, Value>, key: &str) -> Vec<(&'a str, &'a Value)> {
    root.get(key)
        .and_then(Value::as_object)
        .map(|table| {
            table
                .iter()
                .map(|(id, proto)| (id.as_str(), proto))
                .collect()
        })
        .unwrap_or_default()
}

fn collect_machines(root: &Map<String, Value>) -> HashMap<Box<str>, Machine> {
    let mut machines = HashMap::new();
    for (table_name, table_value) in root {
        if matches!(
            table_name.as_str(),
            "item" | "fluid" | "recipe" | "module" | "resource"
        ) {
            continue;
        }
        let Some(prototypes) = table_value.as_object() else {
            continue;
        };
        for (id, proto) in prototypes {
            if let Some(machine) = machine_from(proto) {
                machines.insert(id.as_str().into(), machine);
            }
        }
    }
    machines
}

fn machine_from(proto: &Value) -> Option<Machine> {
    let crafting = string_list(proto.get("crafting_categories"));
    let resource = string_list(proto.get("resource_categories"));
    if crafting.is_empty() && resource.is_empty() {
        return None;
    }

    let speed = if crafting.is_empty() {
        proto
            .get("mining_speed")
            .and_then(Value::as_f64)
            .unwrap_or(1.0)
    } else {
        proto
            .get("crafting_speed")
            .and_then(Value::as_f64)
            .or_else(|| proto.get("mining_speed").and_then(Value::as_f64))
            .unwrap_or(1.0)
    };

    let mut categories = crafting;
    for category in resource {
        if !categories.contains(&category) {
            categories.push(category);
        }
    }

    Some(Machine {
        speed,
        module_slots: uint(proto, "module_slots", 0) as u32,
        allowed_effects: allowed_effects(proto.get("allowed_effects")),
        energy_usage_w: proto
            .get("energy_usage")
            .and_then(Value::as_str)
            .and_then(parse_watts)
            .unwrap_or(0.0),
        pollution_per_minute: proto
            .get("energy_source")
            .and_then(|source| source.get("emissions_per_minute"))
            .and_then(|emissions| emissions.get("pollution"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
        categories,
        base_productivity: proto
            .get("effect_receiver")
            .and_then(|receiver| receiver.get("base_effect"))
            .and_then(|effect| effect.get("productivity"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
    })
}

fn collect_recipes(root: &Map<String, Value>, locale: &Locale) -> BTreeMap<Box<str>, Recipe> {
    let mut recipes = BTreeMap::new();
    for (id, proto) in table(root, "recipe") {
        if is_parameter(proto) {
            continue;
        }
        let mut categories = string_list(proto.get("categories"));
        if categories.is_empty() {
            categories.push("crafting".into());
        }
        let products = parse_products(proto.get("results"));
        // A recipe without products can never contribute to a rate, and its
        // coefficients would all be zero.
        if products.is_empty() {
            continue;
        }
        recipes.insert(
            id.into(),
            Recipe {
                id: id.into(),
                display_name: locale.recipes.get(id).cloned(),
                icon: icon_path(proto),
                categories,
                craft_time: proto
                    .get("energy_required")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.5),
                ingredients: parse_ingredients(proto.get("ingredients")),
                products,
                allow_productivity: flag(proto, "allow_productivity"),
                hidden: flag(proto, "hidden"),
                synthetic: false,
            },
        );
    }
    recipes
}

/// Adds a recipe per minable resource. Mining has no recipe in the dump: the
/// drill's `mining_speed` and the resource's `mining_time` play the roles of
/// crafting speed and craft time.
fn collect_resource_recipes(
    root: &Map<String, Value>,
    recipes: &mut BTreeMap<Box<str>, Recipe>,
    has_item: &impl Fn(&str) -> bool,
    has_fluid: &impl Fn(&str) -> bool,
) {
    for (_id, proto) in table(root, "resource") {
        let Some(minable) = proto.get("minable") else {
            continue;
        };
        let mut products = parse_products(minable.get("results"));
        if products.is_empty() {
            let Some(result) = minable.get("result").and_then(Value::as_str) else {
                continue;
            };
            let kind = if has_fluid(result) {
                PrototypeKind::Fluid
            } else {
                PrototypeKind::Item
            };
            products.push(Product {
                kind,
                name: result.into(),
                amount: minable.get("amount").and_then(Value::as_f64).unwrap_or(1.0),
                ignored_by_productivity: 0.0,
            });
        }
        if products.len() != 1 {
            continue;
        }
        let product = &products[0];
        let ingredient_exists = match product.kind {
            PrototypeKind::Item => has_item(&product.name),
            PrototypeKind::Fluid => has_fluid(&product.name),
        };
        let recipe_id = product.name.clone();
        if !ingredient_exists || recipes.contains_key(&recipe_id) {
            continue;
        }
        recipes.insert(
            recipe_id.clone(),
            Recipe {
                id: recipe_id,
                display_name: None,
                icon: icon_path(proto),
                categories: vec![
                    proto
                        .get("category")
                        .and_then(Value::as_str)
                        .unwrap_or("basic-solid")
                        .into(),
                ],
                craft_time: minable
                    .get("mining_time")
                    .and_then(Value::as_f64)
                    .unwrap_or(1.0),
                ingredients: Vec::new(),
                products,
                allow_productivity: false,
                hidden: false,
                synthetic: true,
            },
        );
    }
}

fn parse_ingredients(value: Option<&Value>) -> Vec<Ingredient> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|ingredient| {
            Some(Ingredient {
                kind: kind_of(ingredient)?,
                name: ingredient.get("name")?.as_str()?.into(),
                amount: ingredient
                    .get("amount")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
            })
        })
        .collect()
}

fn parse_products(value: Option<&Value>) -> Vec<Product> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|product| {
            let probability = match product.get("shared_probability") {
                Some(range) => {
                    let max = range.get("max").and_then(Value::as_f64)?;
                    let min = range.get("min").and_then(Value::as_f64)?;
                    max - min
                }
                None => product
                    .get("independent_probability")
                    .and_then(Value::as_f64)
                    .unwrap_or(1.0),
            };
            let amount = product.get("amount").and_then(Value::as_f64).unwrap_or(1.0);
            let extra = product
                .get("extra_count_fraction")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            Some(Product {
                kind: kind_of(product)?,
                name: product.get("name")?.as_str()?.into(),
                amount: amount * probability + extra,
                ignored_by_productivity: product
                    .get("ignored_by_productivity")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
            })
        })
        .collect()
}

fn kind_of(proto: &Value) -> Option<PrototypeKind> {
    match proto.get("type")?.as_str()? {
        "item" => Some(PrototypeKind::Item),
        "fluid" => Some(PrototypeKind::Fluid),
        _ => None,
    }
}

fn module_effect(proto: &Value) -> ModuleEffect {
    let get = |key: &str| {
        proto
            .get("effect")
            .and_then(|effect| effect.get(key))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    ModuleEffect {
        speed: get("speed"),
        productivity: get("productivity"),
        consumption: get("consumption"),
        pollution: get("pollution"),
        quality: get("quality"),
    }
}

fn allowed_effects(value: Option<&Value>) -> AllowedEffects {
    let effects = string_list(value);
    let has = |name: &str| effects.iter().any(|effect| effect.as_ref() == name);
    AllowedEffects {
        speed: has("speed"),
        productivity: has("productivity"),
        consumption: has("consumption"),
        pollution: has("pollution"),
        quality: has("quality"),
    }
}

fn icon_path(proto: &Value) -> Option<Box<str>> {
    if let Some(icon) = proto.get("icon").and_then(Value::as_str) {
        return Some(icon.into());
    }
    let first = proto.get("icons")?.as_array()?.first()?;
    first.get("icon").and_then(Value::as_str).map(Into::into)
}

fn string_list(value: Option<&Value>) -> Vec<Box<str>> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(Into::into)
                .collect()
        })
        .unwrap_or_default()
}

fn has_prototype(root: &Map<String, Value>, table_name: &str, id: &str) -> bool {
    root.get(table_name)
        .and_then(Value::as_object)
        .is_some_and(|table| table.contains_key(id))
}

fn is_parameter(proto: &Value) -> bool {
    flag(proto, "parameter")
}

fn flag(proto: &Value, key: &str) -> bool {
    proto.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn uint(proto: &Value, key: &str, default: u64) -> u64 {
    proto.get(key).and_then(Value::as_u64).unwrap_or(default)
}

/// Parses a Factorio energy or power string such as `375kW` or `2.5MW`.
fn parse_watts(value: &str) -> Option<f64> {
    let split = value
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let number: f64 = number.trim().parse().ok()?;
    let multiplier = match unit.trim() {
        "" | "W" => 1.0,
        "kW" => 1e3,
        "MW" => 1e6,
        "GW" => 1e9,
        "TW" => 1e12,
        _ => return None,
    };
    Some(number * multiplier)
}

fn read_names(path: &Path) -> HashMap<Box<str>, Box<str>> {
    let Ok(text) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return HashMap::new();
    };
    value
        .get("names")
        .and_then(Value::as_object)
        .map(|names| {
            names
                .iter()
                .filter_map(|(id, name)| Some((id.as_str().into(), name.as_str()?.into())))
                .collect()
        })
        .unwrap_or_default()
}
