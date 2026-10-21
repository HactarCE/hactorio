//! Machine configurations, their modifiers, and recipe coefficients.

use std::collections::BTreeMap;

use hactorio_data::{EntryId, Recipe, RecipeId, Registry};

use crate::ast::Span;
use crate::eval::{EvalError, ModuleCount};
use crate::value::BaseUnit;

/// The effects of a machine's modules, with only allowed effects applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Modifiers {
    /// Machine speed including speed effects.
    pub speed: f64,
    /// Sum of productivity effects.
    pub productivity: f64,
    /// Active power of one machine in watts.
    pub power_w: f64,
    /// Pollution per minute of one machine.
    pub pollution_per_minute: f64,
    /// `1 + sum of consumption effects`.
    pub consumption_multiplier: f64,
}

/// Checks a machine configuration, and the recipe against it when one is known.
pub(crate) fn validate(
    registry: &Registry,
    machine: EntryId,
    modules: &[ModuleCount],
    recipe: Option<&Recipe>,
    span: Span,
) -> Result<(), EvalError> {
    let entry = registry.entry_by_index(machine);
    let Some(data) = entry.machine() else {
        return Err(EvalError::new(
            span,
            format!("`{}` is not a machine", entry.id),
        ));
    };

    let allowed_effects = data.allowed_effects;
    let total: f64 = modules.iter().map(|module| module.count).sum();
    if total > f64::from(data.module_slots) {
        return Err(EvalError::new(
            span,
            format!(
                "`{}` has {} module slots, but {total} modules were given",
                entry.id, data.module_slots
            ),
        ));
    }

    for module in modules {
        let module_entry = registry.entry_by_index(module.module);
        let Some(data) = module_entry.module.as_ref() else {
            return Err(EvalError::new(
                span,
                format!("`{}` is not a module", module_entry.id),
            ));
        };
        if !category_allowed(data.category.as_ref(), allowed_effects) {
            return Err(EvalError::new(
                span,
                format!(
                    "`{}` does not accept `{}` modules",
                    entry.id, module_entry.id
                ),
            ));
        }
    }

    if let Some(recipe) = recipe {
        if !data
            .categories
            .iter()
            .any(|category| recipe.categories.contains(category))
        {
            return Err(EvalError::new(
                span,
                format!("`{}` cannot craft `{}`", entry.id, recipe.id),
            ));
        }
        if !recipe.allow_productivity && modules.iter().any(|m| is_productivity(registry, m)) {
            return Err(EvalError::new(
                span,
                format!("`{}` does not allow productivity modules", recipe.id),
            ));
        }
    }

    Ok(())
}

/// Computes the modifiers of a validated configuration.
pub(crate) fn modifiers(
    registry: &Registry,
    machine: EntryId,
    modules: &[ModuleCount],
    recipe: Option<&Recipe>,
) -> Modifiers {
    let Some(data) = registry.entry_by_index(machine).machine() else {
        return Modifiers {
            speed: 0.0,
            productivity: 0.0,
            power_w: 0.0,
            pollution_per_minute: 0.0,
            consumption_multiplier: 1.0,
        };
    };
    let allowed = data.allowed_effects;

    let mut speed_sum = 0.0;
    let mut productivity = 0.0;
    let mut consumption_sum = 0.0;
    let mut pollution_sum = 0.0;
    for module in modules {
        let Some(module_data) = registry.entry_by_index(module.module).module.as_ref() else {
            continue;
        };
        let count = module.count;
        if allowed.speed {
            speed_sum += module_data.effect.speed * count;
        }
        if allowed.consumption {
            consumption_sum += module_data.effect.consumption * count;
        }
        if allowed.productivity {
            productivity += module_data.effect.productivity * count;
        }
        if allowed.pollution {
            pollution_sum += module_data.effect.pollution * count;
        }
    }

    let productivity_allowed = recipe.is_none_or(|recipe| recipe.allow_productivity);
    Modifiers {
        speed: data.speed * (1.0 + speed_sum),
        productivity: if productivity_allowed {
            productivity
        } else {
            0.0
        },
        power_w: data.energy_usage_w * (1.0 + consumption_sum),
        pollution_per_minute: data.pollution_per_minute
            * (1.0 + consumption_sum)
            * (1.0 + pollution_sum),
        consumption_multiplier: 1.0 + consumption_sum,
    }
}

/// Net amounts per machine-second, keyed by base unit.
///
/// Products are positive, ingredients negative, and `craft` holds `S/T`.
pub(crate) fn coefficients(
    registry: &Registry,
    machine: EntryId,
    modules: &[ModuleCount],
    recipe: &Recipe,
    span: Span,
) -> Result<BTreeMap<BaseUnit, f64>, EvalError> {
    let modifiers = modifiers(registry, machine, modules, Some(recipe));
    let crafts_per_second = modifiers.speed / recipe.craft_time;

    let mut net: BTreeMap<BaseUnit, f64> = BTreeMap::new();
    for product in &recipe.products {
        let unit = unit_of(registry, product.kind, &product.name, span)?;
        let productive = product.amount
            + modifiers.productivity * (product.amount - product.ignored_by_productivity);
        *net.entry(unit).or_insert(0.0) += crafts_per_second * productive;
    }
    for ingredient in &recipe.ingredients {
        let unit = unit_of(registry, ingredient.kind, &ingredient.name, span)?;
        *net.entry(unit).or_insert(0.0) -= crafts_per_second * ingredient.amount;
    }
    *net.entry(BaseUnit::Craft).or_insert(0.0) += crafts_per_second;

    net.retain(|_, amount| *amount != 0.0);
    Ok(net)
}

/// Finds the one recipe a machine can craft that produces `unit`.
pub(crate) fn resolve_recipe(
    registry: &Registry,
    machine: EntryId,
    unit: BaseUnit,
    span: Span,
) -> Result<RecipeId, EvalError> {
    let entry = registry.entry_by_index(machine);
    let Some(data) = entry.machine() else {
        return Err(EvalError::new(
            span,
            format!("`{}` is not a machine", entry.id),
        ));
    };

    let candidates: Vec<RecipeId> = registry
        .recipes()
        .iter()
        .enumerate()
        .filter(|(_, recipe)| {
            recipe
                .categories
                .iter()
                .any(|category| data.categories.contains(category))
        })
        .filter(|(_, recipe)| {
            recipe.products.iter().any(|product| {
                product.amount > 0.0
                    && unit_of(registry, product.kind, &product.name, span).ok() == Some(unit)
            })
        })
        .map(|(index, _)| RecipeId::from_index(index))
        .collect();

    match candidates.as_slice() {
        [recipe] => Ok(*recipe),
        [] => Err(EvalError::new(
            span,
            format!("no recipe `{}` can craft produces this item", entry.id),
        )),
        _ => Err(EvalError::new(
            span,
            format!(
                "{} recipes `{}` can craft produce this; add `making`",
                candidates.len(),
                entry.id
            ),
        )),
    }
}

fn unit_of(
    registry: &Registry,
    kind: hactorio_data::PrototypeKind,
    name: &str,
    span: Span,
) -> Result<BaseUnit, EvalError> {
    let Some(id) = registry.entry_id(name) else {
        return Err(EvalError::new(span, format!("`{name}` is not a prototype")));
    };
    Ok(match kind {
        hactorio_data::PrototypeKind::Item => BaseUnit::Item(id),
        hactorio_data::PrototypeKind::Fluid => BaseUnit::Fluid(id),
    })
}

fn is_productivity(registry: &Registry, module: &ModuleCount) -> bool {
    registry
        .entry_by_index(module.module)
        .module
        .as_ref()
        .is_some_and(|data| data.category.as_ref() == "productivity")
}

/// Module categories map to the effect that must be allowed.
fn category_allowed(category: &str, allowed: hactorio_data::AllowedEffects) -> bool {
    match category {
        "speed" => allowed.speed,
        "productivity" => allowed.productivity,
        "efficiency" => allowed.consumption,
        "quality" => allowed.quality,
        _ => true,
    }
}
