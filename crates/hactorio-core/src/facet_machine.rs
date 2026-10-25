//! Machine configuration validation, modifiers, and recipe coefficients for
//! facet values.

use std::collections::BTreeMap;

use hactorio_data::{EntryId, Recipe, RecipeId, Registry};

use crate::facets::{BeaconGroup, ConfigFacet, ModuleCount};
use crate::settings::Settings;
use crate::value::{BaseUnit, is_nonzero};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Modifiers {
    pub speed: f64,
    pub productivity: f64,
    pub power_w: f64,
    pub pollution_per_minute: f64,
    /// `1 + Σ consumption effects`, before the game's 0.2 floor.
    pub consumption_multiplier: f64,
    /// Σ speed effects from machine and beacon modules.
    pub speed_effect: f64,
    /// Σ productivity effects from machine and beacon modules.
    pub productivity_effect: f64,
}

impl Modifiers {
    /// The game clamps the consumption multiplier to at least 0.2.
    pub fn clamped_consumption_multiplier(&self) -> f64 {
        self.consumption_multiplier.max(0.2)
    }
}

/// Validates module slots, allowed effects, and recipe compatibility.
pub fn validate(
    registry: &Registry,
    machine: EntryId,
    config: &ConfigFacet,
    recipe: Option<&Recipe>,
) -> Result<(), String> {
    let entry = registry.entry_by_index(machine);
    let data = entry
        .machine()
        .ok_or_else(|| format!("`{}` is not a machine", entry.id))?;

    let total: u32 = config.modules.iter().map(|module| module.count).sum();
    if total > data.module_slots {
        return Err(format!(
            "`{}` has {} module slots, but {total} modules were given",
            entry.id, data.module_slots
        ));
    }
    for module in &config.modules {
        validate_module(registry, module, data.allowed_effects, entry.id.as_ref())?;
    }

    validate_beacon_groups(registry, config)?;

    if let Some(recipe) = recipe {
        if !data
            .categories
            .iter()
            .any(|category| recipe.categories.contains(category))
        {
            return Err(format!("`{}` cannot craft `{}`", entry.id, recipe.id));
        }
        if !recipe.allow_productivity && config_has_productivity(registry, config) {
            return Err(format!(
                "`{}` does not allow productivity modules",
                recipe.id
            ));
        }
    }
    Ok(())
}

/// Validates a config's beacon slots and allowed effects.
///
/// Called when a beacon group is constructed (config juxtaposition) and again
/// by `with`/`making`/`in`.
pub fn validate_beacon_groups(registry: &Registry, config: &ConfigFacet) -> Result<(), String> {
    if config.has_multiple_beacon_types() {
        return Err("a config may contain only one type of beacon".into());
    }
    for group in &config.beacons {
        let beacon_entry = registry.entry_by_index(group.beacon);
        let Some(beacon) = &beacon_entry.beacon else {
            return Err(format!("`{}` is not a beacon", beacon_entry.id));
        };
        let total: u32 = group.modules.iter().map(|module| module.count).sum();
        if total > beacon.module_slots {
            return Err(format!(
                "`{}` has {} module slots, but {total} modules were given",
                beacon_entry.id, beacon.module_slots
            ));
        }
        for module in &group.modules {
            validate_module(
                registry,
                module,
                beacon.allowed_effects,
                beacon_entry.id.as_ref(),
            )?;
        }
    }
    Ok(())
}

fn validate_module(
    registry: &Registry,
    module: &ModuleCount,
    allowed: hactorio_data::AllowedEffects,
    owner: &str,
) -> Result<(), String> {
    let entry = registry.entry_by_index(module.module);
    let Some(data) = &entry.module else {
        return Err(format!("`{}` is not a module", entry.id));
    };
    if !category_allowed(data.category.as_ref(), allowed) {
        return Err(format!("`{owner}` does not accept `{}` modules", entry.id));
    }
    Ok(())
}

pub fn config_has_productivity(registry: &Registry, config: &ConfigFacet) -> bool {
    config
        .modules
        .iter()
        .any(|module| is_productivity(registry, module))
        || config.beacons.iter().any(|group| {
            group
                .modules
                .iter()
                .any(|module| is_productivity(registry, module))
        })
}

pub fn modifiers(
    registry: &Registry,
    machine: EntryId,
    config: &ConfigFacet,
    recipe: Option<&Recipe>,
    settings: &Settings,
) -> Result<Modifiers, String> {
    let Some(data) = registry.entry_by_index(machine).machine() else {
        return Ok(Modifiers {
            speed: 0.0,
            productivity: 0.0,
            power_w: 0.0,
            pollution_per_minute: 0.0,
            consumption_multiplier: 1.0,
            speed_effect: 0.0,
            productivity_effect: 0.0,
        });
    };
    let allowed = data.allowed_effects;

    let mut speed_sum = 0.0;
    let mut productivity_sum = 0.0;
    let mut consumption_sum = 0.0;
    let mut pollution_sum = 0.0;

    for module in &config.modules {
        if let Some(module_data) = module_effect(registry, module) {
            add_module_effects(
                module_data,
                module.count,
                1.0,
                allowed,
                &mut speed_sum,
                &mut productivity_sum,
                &mut consumption_sum,
                &mut pollution_sum,
            );
        }
    }

    let beacon_count: u32 = config.beacons.iter().map(|group| group.count).sum();
    if beacon_count > 0 {
        let mut beacon_speed = 0.0;
        let mut beacon_productivity = 0.0;
        let mut beacon_consumption = 0.0;
        let mut beacon_pollution = 0.0;
        for BeaconGroup {
            beacon,
            count,
            modules,
        } in &config.beacons
        {
            let beacon_entry = registry.entry_by_index(*beacon);
            let Some(beacon_data) = &beacon_entry.beacon else {
                continue;
            };
            for module in modules {
                if let Some(module_data) = module_effect(registry, module) {
                    add_module_effects(
                        module_data,
                        module.count,
                        f64::from(*count) * beacon_data.distribution_effectivity,
                        beacon_data.allowed_effects,
                        &mut beacon_speed,
                        &mut beacon_productivity,
                        &mut beacon_consumption,
                        &mut beacon_pollution,
                    );
                }
            }
        }
        let first_beacon = config
            .beacons
            .first()
            .map(|group| registry.entry_by_index(group.beacon));
        let profile = first_beacon
            .and_then(|entry| entry.beacon.as_ref())
            .map(|beacon| beacon.profile.as_slice())
            .unwrap_or(&[]);
        let index = (beacon_count as usize) - 1;
        let profile = profile.get(index).copied().ok_or_else(|| {
            let name = first_beacon.map_or("beacon", |entry| entry.id.as_ref());
            format!(
                "{beacon_count} beacons exceed the `{name}` distribution profile length ({})",
                profile.len()
            )
        })?;
        // Machine allowed effects filter beacon-transmitted effects.
        if allowed.speed {
            speed_sum += beacon_speed * profile;
        }
        if allowed.productivity {
            productivity_sum += beacon_productivity * profile;
        }
        if allowed.consumption {
            consumption_sum += beacon_consumption * profile;
        }
        if allowed.pollution {
            pollution_sum += beacon_pollution * profile;
        }
    }

    let productivity_allowed = recipe.is_none_or(|recipe| recipe.allow_productivity);
    let module_productivity = if productivity_allowed {
        productivity_sum
    } else {
        0.0
    };

    let consumption_multiplier = 1.0 + consumption_sum;
    let mut modifiers = Modifiers {
        speed: data.speed * (1.0 + speed_sum),
        productivity: data.base_productivity
            + module_productivity
            + recipe.map_or(0.0, |recipe| {
                research_productivity(registry, recipe, settings)
            }),
        power_w: 0.0,
        pollution_per_minute: 0.0,
        consumption_multiplier,
        speed_effect: speed_sum,
        productivity_effect: module_productivity,
    };
    let clamped_consumption = modifiers.clamped_consumption_multiplier();
    modifiers.power_w = data.energy_usage_w * clamped_consumption;
    modifiers.pollution_per_minute =
        data.pollution_per_minute * clamped_consumption * (1.0 + pollution_sum);
    Ok(modifiers)
}

fn module_effect<'a>(
    registry: &'a Registry,
    module: &ModuleCount,
) -> Option<&'a hactorio_data::ModuleEffect> {
    Some(
        &registry
            .entry_by_index(module.module)
            .module
            .as_ref()?
            .effect,
    )
}

fn add_module_effects(
    effect: &hactorio_data::ModuleEffect,
    count: u32,
    multiplier: f64,
    allowed: hactorio_data::AllowedEffects,
    speed: &mut f64,
    productivity: &mut f64,
    consumption: &mut f64,
    pollution: &mut f64,
) {
    let count = f64::from(count) * multiplier;
    if allowed.speed {
        *speed += effect.speed * count;
    }
    if allowed.productivity {
        *productivity += effect.productivity * count;
    }
    if allowed.consumption {
        *consumption += effect.consumption * count;
    }
    if allowed.pollution {
        *pollution += effect.pollution * count;
    }
}

pub fn coefficients(
    registry: &Registry,
    machine: EntryId,
    config: &ConfigFacet,
    recipe: &Recipe,
    settings: &Settings,
) -> Result<BTreeMap<BaseUnit, f64>, String> {
    let modifiers = modifiers(registry, machine, config, Some(recipe), settings)?;
    let crafts_per_second = modifiers.speed / recipe.craft_time;

    let mut net: BTreeMap<BaseUnit, f64> = BTreeMap::new();
    for product in &recipe.products {
        let unit = unit_of(registry, product.kind, &product.name)?;
        let productive = product.amount
            + modifiers.productivity * (product.amount - product.ignored_by_productivity);
        *net.entry(unit).or_insert(0.0) += crafts_per_second * productive;
    }
    for ingredient in &recipe.ingredients {
        let unit = unit_of(registry, ingredient.kind, &ingredient.name)?;
        *net.entry(unit).or_insert(0.0) -= crafts_per_second * ingredient.amount;
    }
    *net.entry(BaseUnit::Craft).or_insert(0.0) += crafts_per_second;
    net.retain(|_, amount| is_nonzero(*amount));
    Ok(net)
}

pub fn resolve_recipe(
    registry: &Registry,
    machine: EntryId,
    config: &ConfigFacet,
    unit: BaseUnit,
) -> Result<RecipeId, String> {
    let entry = registry.entry_by_index(machine);
    let data = entry
        .machine()
        .ok_or_else(|| format!("`{}` is not a machine", entry.id))?;

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
                    && is_nonzero(product.amount)
                    && unit_of(registry, product.kind, &product.name).ok() == Some(unit)
            })
        })
        .filter(|(_, recipe)| {
            recipe.allow_productivity || !config_has_productivity(registry, config)
        })
        .map(|(index, _)| RecipeId::from_index(index))
        .collect();

    match candidates.as_slice() {
        [recipe] => Ok(*recipe),
        [] => Err(format!(
            "no recipe `{}` can craft produces this item",
            entry.id
        )),
        _ => {
            if let BaseUnit::Item(id) | BaseUnit::Fluid(id) = unit {
                let unit_entry = registry.entry_by_index(id);
                if let Some(recipe) = registry.recipe_id(unit_entry.id.as_ref())
                    && candidates.contains(&recipe)
                {
                    return Ok(recipe);
                }
            }
            Err(format!(
                "{} recipes `{}` can craft produce this; add `making`",
                candidates.len(),
                entry.id
            ))
        }
    }
}

pub fn unit_of(
    registry: &Registry,
    kind: hactorio_data::PrototypeKind,
    name: &str,
) -> Result<BaseUnit, String> {
    let Some(id) = registry.entry_id(name) else {
        return Err(format!("`{name}` is not a prototype"));
    };
    Ok(match kind {
        hactorio_data::PrototypeKind::Item => BaseUnit::Item(id),
        hactorio_data::PrototypeKind::Fluid => BaseUnit::Fluid(id),
    })
}

fn research_productivity(registry: &Registry, recipe: &Recipe, settings: &Settings) -> f64 {
    registry
        .productivity_researches()
        .iter()
        .map(|research| {
            let level = f64::from(settings.research_level(&research.id));
            research
                .effects
                .iter()
                .filter(|effect| effect.recipe == recipe.id)
                .map(|effect| effect.change * level)
                .sum::<f64>()
        })
        .sum()
}

fn is_productivity(registry: &Registry, module: &ModuleCount) -> bool {
    registry
        .entry_by_index(module.module)
        .module
        .as_ref()
        .is_some_and(|data| data.category.as_ref() == "productivity")
}

fn category_allowed(category: &str, allowed: hactorio_data::AllowedEffects) -> bool {
    match category {
        "speed" => allowed.speed,
        "productivity" => allowed.productivity,
        "efficiency" => allowed.consumption,
        "quality" => allowed.quality,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facets::ConfigFacet;
    use hactorio_data::{Locale, postprocess};

    #[test]
    fn consumption_is_clamped_at_twenty_percent() {
        const DUMP: &str = r#"{
          "item": {
            "assembling-machine": {"type": "item", "name": "assembling-machine", "stack_size": 50},
            "efficiency-module": {"type": "module", "name": "efficiency-module", "category": "efficiency", "tier": 1, "stack_size": 50, "effect": {"consumption": -1.0}}
          },
          "fluid": {},
          "module": {
            "efficiency-module": {"type": "module", "name": "efficiency-module", "category": "efficiency", "tier": 1, "stack_size": 50, "effect": {"consumption": -1.0}}
          },
          "assembling-machine": {
            "assembling-machine": {"type": "assembling-machine", "name": "assembling-machine", "crafting_categories": ["crafting"], "crafting_speed": 1.0, "module_slots": 1, "allowed_effects": ["consumption"], "energy_usage": "100kW", "energy_source": {"emissions_per_minute": {"pollution": 10}}}
          }
        }"#;
        let registry = postprocess(DUMP, &Locale::empty()).unwrap();
        let machine = registry.entry_id("assembling-machine").unwrap();
        let module = registry.entry_id("efficiency-module").unwrap();
        let config = ConfigFacet {
            modules: vec![ModuleCount { module, count: 1 }],
            beacons: Vec::new(),
        };
        let modifiers = modifiers(&registry, machine, &config, None, &Settings::default()).unwrap();
        assert_eq!(modifiers.consumption_multiplier, 0.0);
        assert_eq!(modifiers.clamped_consumption_multiplier(), 0.2);
        assert_eq!(modifiers.power_w, 20_000.0);
        assert_eq!(modifiers.pollution_per_minute, 2.0);
    }
}
