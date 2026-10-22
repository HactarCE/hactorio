//! The prototype namespace and its runtime registry.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// First codepoint of the BMP private use area, where glyphs are assigned.
pub const PUA_FIRST: u32 = 0xE000;
/// Number of codepoints in the BMP private use area.
pub const PUA_COUNT: u32 = 0xF8FF - PUA_FIRST + 1;

/// The glyph assigned to the entry at `index`, or `None` if the namespace is
/// too large to fit in the private use area.
pub fn glyph_for(index: usize) -> Option<char> {
    if index as u64 >= u64::from(PUA_COUNT) {
        return None;
    }
    char::from_u32(PUA_FIRST + index as u32)
}

/// An index into [`Registry::entries`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntryId(u32);

impl EntryId {
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// Builds an id from an index, for callers iterating entries.
    pub fn from_index(index: usize) -> Self {
        EntryId(index as u32)
    }
}

/// An index into [`Registry::recipes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RecipeId(u32);

impl RecipeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn from_index(index: usize) -> Self {
        RecipeId(index as u32)
    }
}

/// Which prototype table a base unit or recipe ingredient comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrototypeKind {
    Item,
    Fluid,
}

/// An item prototype, if a namespace entry names one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub stack_size: u32,
    /// Present when the item is also a machine prototype.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<Machine>,
}

/// A machine prototype: a crafting machine or a mining drill.
///
/// Both kinds are joined from the item of the same name to the entity that
/// carries the crafting or resource categories. `categories` is the union of
/// the entity's `crafting_categories` and `resource_categories`; a recipe is
/// craftable by the machine when any of the recipe's categories is in it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    /// `crafting_speed` for crafting machines, `mining_speed` for drills.
    pub speed: f64,
    pub module_slots: u32,
    pub allowed_effects: AllowedEffects,
    /// Active power in watts, from the prototype's `energy_usage`.
    pub energy_usage_w: f64,
    pub pollution_per_minute: f64,
    pub categories: Vec<Box<str>>,
}

/// Which module effects a machine accepts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowedEffects {
    pub speed: bool,
    pub productivity: bool,
    pub consumption: bool,
    pub pollution: bool,
    pub quality: bool,
}

/// The per-module bonuses of a module prototype.
///
/// Effects absent from the prototype are zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ModuleEffect {
    pub speed: f64,
    pub productivity: f64,
    pub consumption: f64,
    pub pollution: f64,
    pub quality: f64,
}

/// A module prototype.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module {
    /// Effect category, e.g. `speed`, `productivity`, `efficiency`, `quality`.
    pub category: Box<str>,
    pub tier: u32,
    pub stack_size: u32,
    pub effect: ModuleEffect,
}

/// One entry of the single namespace: an item, a fluid, a module, a recipe, or
/// some combination of those sharing an id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// The prototype id, as written by the plaintext token form.
    pub id: Box<str>,
    /// The private-use codepoint assigned to this entry.
    pub glyph: char,
    /// Localised name, when locale data was available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<Box<str>>,
    /// Icon path relative to game data, e.g. `__base__/graphics/icons/iron-plate.png`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Box<str>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<Item>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub fluid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<Module>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe: Option<RecipeId>,
}

impl Entry {
    /// Whether the entry names an item or a fluid, and can therefore be a base unit.
    pub fn is_item_or_fluid(&self) -> bool {
        self.item.is_some() || self.fluid
    }

    /// Whether the entry names a machine item.
    pub fn machine(&self) -> Option<&Machine> {
        self.item.as_ref()?.machine.as_ref()
    }
}

/// One side of a recipe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ingredient {
    pub kind: PrototypeKind,
    pub name: Box<str>,
    pub amount: f64,
}

/// A recipe product, with probabilities already folded into `amount`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Product {
    pub kind: PrototypeKind,
    pub name: Box<str>,
    /// Expected amount per craft: the prototype `amount` scaled by its
    /// probability, plus `extra_count_fraction`.
    pub amount: f64,
    /// The part of `amount` that productivity does not apply to.
    pub ignored_by_productivity: f64,
}

/// A recipe prototype, or a synthetic recipe for a minable resource.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub id: Box<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<Box<str>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Box<str>>,
    /// Categories the recipe belongs to; a machine must accept one of them.
    pub categories: Vec<Box<str>>,
    /// Seconds per craft.
    pub craft_time: f64,
    pub ingredients: Vec<Ingredient>,
    pub products: Vec<Product>,
    /// Whether productivity modules may be used.
    pub allow_productivity: bool,
    pub hidden: bool,
    /// True for recipes synthesized from a `resource` prototype, which has no
    /// recipe in the dump.
    pub synthetic: bool,
}

/// The immutable prototype registry shared by the evaluator.
#[derive(Debug, Clone)]
pub struct Registry {
    entries: Vec<Entry>,
    recipes: Vec<Recipe>,
    by_id: HashMap<Box<str>, EntryId>,
    recipe_by_id: HashMap<Box<str>, RecipeId>,
    /// How many recipe ingredients and products name each entry, indexed by
    /// entry. A rough popularity signal, used to rank autocomplete matches.
    references: Vec<u32>,
}

impl Registry {
    pub(crate) fn from_parts(entries: Vec<Entry>, recipes: Vec<Recipe>) -> Result<Self> {
        if entries.len() > PUA_COUNT as usize {
            return Err(Error::Invalid(format!(
                "{} namespace entries exceed the {} private-use codepoints",
                entries.len(),
                PUA_COUNT
            )));
        }
        for (i, entry) in entries.iter().enumerate() {
            if glyph_for(i) != Some(entry.glyph) {
                return Err(Error::Invalid(format!(
                    "entry {:?} at {i} has glyph {:?}, expected {:?}",
                    entry.id,
                    entry.glyph,
                    glyph_for(i)
                )));
            }
            if i > 0 && entries[i - 1].id >= entry.id {
                return Err(Error::Invalid(format!(
                    "entries are not sorted by id: {:?} before {:?}",
                    entries[i - 1].id,
                    entry.id
                )));
            }
        }
        for (i, recipe) in recipes.iter().enumerate() {
            if i > 0 && recipes[i - 1].id >= recipe.id {
                return Err(Error::Invalid(format!(
                    "recipes are not sorted by id: {:?} before {:?}",
                    recipes[i - 1].id,
                    recipe.id
                )));
            }
        }

        let by_id: HashMap<Box<str>, EntryId> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id.clone(), EntryId::from_index(i)))
            .collect();
        let recipe_by_id: HashMap<Box<str>, RecipeId> = recipes
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id.clone(), RecipeId(i as u32)))
            .collect();

        for entry in &entries {
            let Some(recipe) = entry.recipe else {
                continue;
            };
            let Some(found) = recipes.get(recipe.index()) else {
                return Err(Error::Invalid(format!(
                    "entry {:?} refers to missing recipe {}",
                    entry.id,
                    recipe.index()
                )));
            };
            if found.id != entry.id {
                return Err(Error::Invalid(format!(
                    "entry {:?} refers to recipe {:?}",
                    entry.id, found.id
                )));
            }
        }

        let mut references = vec![0u32; entries.len()];
        for recipe in &recipes {
            let names = recipe
                .ingredients
                .iter()
                .map(|ingredient| &ingredient.name)
                .chain(recipe.products.iter().map(|product| &product.name));
            for name in names {
                if let Some(id) = by_id.get(&**name) {
                    references[id.index()] += 1;
                }
            }
        }

        Ok(Registry {
            entries,
            recipes,
            by_id,
            recipe_by_id,
            references,
        })
    }

    /// Every namespace entry, sorted by id. Glyph order matches entry order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn recipes(&self) -> &[Recipe] {
        &self.recipes
    }

    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.by_id.get(id).map(|id| &self.entries[id.index()])
    }

    pub fn entry_id(&self, id: &str) -> Option<EntryId> {
        self.by_id.get(id).copied()
    }

    pub fn entry_by_index(&self, id: EntryId) -> &Entry {
        &self.entries[id.index()]
    }

    /// How many recipe ingredients and products name this entry.
    ///
    /// This is a rough popularity signal: items used across many recipes, such
    /// as iron plates, score much higher than niche ones.
    pub fn reference_count(&self, id: EntryId) -> u32 {
        self.references[id.index()]
    }

    /// Resolves a private-use codepoint back to its entry.
    pub fn entry_by_glyph(&self, glyph: char) -> Option<&Entry> {
        let index = (glyph as u32).checked_sub(PUA_FIRST)? as usize;
        let entry = self.entries.get(index)?;
        (entry.glyph == glyph).then_some(entry)
    }

    pub fn recipe(&self, id: RecipeId) -> &Recipe {
        &self.recipes[id.index()]
    }

    pub fn find_recipe(&self, id: &str) -> Option<&Recipe> {
        self.recipe_by_id
            .get(id)
            .map(|id| &self.recipes[id.index()])
    }

    pub fn recipe_id(&self, id: &str) -> Option<RecipeId> {
        self.recipe_by_id.get(id).copied()
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}
