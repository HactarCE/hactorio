//! Per-row rates for a process: machine modifiers and recipe arithmetic
//! (§6.3, §6.4).

use std::collections::BTreeMap;

use hactorio_data::{ModuleEffect, PrototypeKind, Recipe, Registry};

use super::value::{Config, Item, Quality, Resource, RowKey};
use crate::Settings;
use crate::settings::IdlePower;

/// What one machine of a row produces per second at one craft each.
#[derive(Debug, Clone, PartialEq)]
pub struct RowRates {
    pub inputs: BTreeMap<Resource, f64>,
    pub outputs: BTreeMap<Resource, f64>,
    pub craft_per_second: f64,
}

/// The machine's modifiers after modules, beacons, and research (§6.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Modifiers {
    pub speed: f64,
    /// Productivity bonus, not the multiplier: `0.5` is `+50%`.
    pub productivity: f64,
    pub consumption: f64,
    pub pollution: f64,
    pub power_w: f64,
    /// Idle draw in watts, unaffected by modules and beacons.
    pub idle_usage_w: f64,
    pub pollution_per_minute: f64,
    /// The chance of upgrading each product one quality level, from 0 to 1.
    pub quality_chance: f64,
}

/// The productivity cap: `+300%` (§6.3).
const PRODUCTIVITY_CAP: f64 = 3.0;

pub fn row_rates(key: &RowKey, registry: &Registry, settings: &Settings) -> Option<RowRates> {
    let recipe = registry.recipe(key.recipe);
    let modifiers = modifiers(key, registry, settings, Some(recipe))?;
    if recipe.craft_time <= 0.0 {
        return None;
    }
    let craft_per_second = modifiers.speed / recipe.craft_time;

    let mut inputs = BTreeMap::new();
    for ingredient in &recipe.ingredients {
        let resource = resource_of(
            ingredient.kind,
            &ingredient.name,
            key.recipe_quality,
            registry,
        )?;
        *inputs.entry(resource).or_insert(0.0) += ingredient.amount * craft_per_second;
    }
    let mut outputs = BTreeMap::new();
    for product in &recipe.products {
        let amount = product.amount
            + modifiers.productivity * (product.amount - product.ignored_by_productivity);
        let amount = amount * craft_per_second;
        let id = registry.entry_id(&product.name)?;
        match product.kind {
            PrototypeKind::Item => {
                // Quality modules spread each product over the qualities above
                // the recipe's own (§6.3).
                for (quality, share) in quality_shares(
                    key.recipe_quality,
                    modifiers.quality_chance,
                    settings.max_quality,
                ) {
                    *outputs
                        .entry(Resource::Item(Item { id, quality }))
                        .or_insert(0.0) += amount * share;
                }
            }
            PrototypeKind::Fluid => {
                *outputs.entry(Resource::Fluid(id)).or_insert(0.0) += amount;
            }
        }
    }

    Some(RowRates {
        inputs,
        outputs,
        craft_per_second,
    })
}

fn resource_of(
    kind: PrototypeKind,
    name: &str,
    quality: Quality,
    registry: &Registry,
) -> Option<Resource> {
    let id = registry.entry_id(name)?;
    Some(match kind {
        PrototypeKind::Item => Resource::Item(Item { id, quality }),
        PrototypeKind::Fluid => Resource::Fluid(id),
    })
}

pub fn modifiers(
    key: &RowKey,
    registry: &Registry,
    settings: &Settings,
    recipe: Option<&Recipe>,
) -> Option<Modifiers> {
    let entry = registry.entry_by_index(key.machine);
    let machine = entry.item.as_ref()?.machine.as_ref()?;
    let allowed = machine.allowed_effects;

    // Quality scales what a module does and what a machine is (§6.3).
    let mut effect = ModuleEffect::default();
    for ((module, quality), count) in &key.config.modules {
        effect = effect + module_effect(*module, *quality, *count, registry)?;
    }

    let total_beacons: u32 = key.config.total_beacons();
    if total_beacons > 0 {
        let first = key.config.beacons.keys().next()?;
        let beacon = registry.entry_by_index(first.beacon).beacon.as_ref()?;
        let index = usize::try_from(total_beacons - 1).ok()?;
        let profile = *beacon.profile.get(index)?;

        let mut beaconed = ModuleEffect::default();
        for (group, count) in &key.config.beacons {
            let beacon = registry.entry_by_index(group.beacon).beacon.as_ref()?;
            let mut per_beacon = ModuleEffect::default();
            for ((module, quality), module_count) in &group.modules {
                per_beacon =
                    per_beacon + module_effect(*module, *quality, *module_count, registry)?;
            }
            let factor = beacon.distribution_effectivity
                * group.quality.multiplier()
                * f64::from(*count)
                * profile;
            beaconed = beaconed + scale_effect(per_beacon, factor);
        }
        effect = effect + beaconed;
    }

    // A machine that does not allow an effect ignores it (§6.3).
    if !allowed.speed {
        effect.speed = 0.0;
    }
    if !allowed.productivity {
        effect.productivity = 0.0;
    }
    if !allowed.consumption {
        effect.consumption = 0.0;
    }
    if !allowed.pollution {
        effect.pollution = 0.0;
    }

    let research: f64 = recipe
        .map(|recipe| {
            registry
                .productivity_researches()
                .iter()
                .map(|productivity| {
                    let per_level: f64 = productivity
                        .effects
                        .iter()
                        .filter(|effect| effect.recipe.as_ref() == recipe.id.as_ref())
                        .map(|effect| effect.change)
                        .sum();
                    per_level * f64::from(settings.research_level(&productivity.id))
                })
                .sum()
        })
        .unwrap_or(0.0);

    // A recipe that disallows productivity contributes none (§6.3).
    let productivity = match recipe {
        Some(recipe) if !recipe.allow_productivity => 0.0,
        _ => (machine.base_productivity + effect.productivity + research).min(PRODUCTIVITY_CAP),
    };
    let consumption = (1.0 + effect.consumption).max(0.2);

    Some(Modifiers {
        speed: machine.speed * key.machine_quality.multiplier() * (1.0 + effect.speed),
        productivity,
        consumption,
        pollution: 1.0 + effect.pollution,
        power_w: machine.energy_usage_w * consumption,
        idle_usage_w: machine.idle_usage_w,
        pollution_per_minute: machine.pollution_per_minute * consumption * (1.0 + effect.pollution),
        quality_chance: effect.quality.clamp(0.0, 1.0),
    })
}

/// What a module contributes, at its quality (§6.3).
///
/// Quality improves the effect the module exists for — its speed, its
/// productivity, its efficiency, or its quality — and leaves its downsides
/// exactly where they were. A legendary productivity module is a better
/// productivity module, not a hungrier one.
fn module_effect(
    module: hactorio_data::EntryId,
    quality: Quality,
    count: u32,
    registry: &Registry,
) -> Option<ModuleEffect> {
    let entry = registry.entry_by_index(module);
    let prototype = entry.module.as_ref()?;
    let mut effect = scale_effect(prototype.effect, f64::from(count));
    let factor = quality.multiplier();
    match prototype.category.as_ref() {
        "speed" => effect.speed *= factor,
        "productivity" => effect.productivity *= factor,
        "efficiency" => effect.consumption *= factor,
        "quality" => effect.quality *= factor,
        _ => {}
    }
    Some(effect)
}

/// The speed every row of a process shares (§6.6).
pub fn speed(
    process: &super::value::Process,
    registry: &Registry,
    settings: &Settings,
) -> Option<f64> {
    let mut shared: Option<f64> = None;
    for key in process.rows.keys() {
        let speed = row_modifiers(key, registry, settings)?.speed;
        match shared {
            Some(existing) if (existing - speed).abs() > super::value::APPROX_ZERO => {
                return None;
            }
            _ => shared = Some(speed),
        }
    }
    shared
}

fn scale_effect(effect: ModuleEffect, factor: f64) -> ModuleEffect {
    ModuleEffect {
        speed: effect.speed * factor,
        productivity: effect.productivity * factor,
        consumption: effect.consumption * factor,
        pollution: effect.pollution * factor,
        quality: effect.quality * factor,
    }
}

/// Rows that differ only in recipe quality are alternatives, not a sum, so a
/// process of "two foundries making iron plates at any quality" is two machines
/// rather than ten (§6.4).
pub fn machines(rows: &BTreeMap<RowKey, f64>) -> f64 {
    let mut seen: BTreeMap<
        (
            &hactorio_data::EntryId,
            &Quality,
            &Config,
            &hactorio_data::RecipeId,
        ),
        f64,
    > = BTreeMap::new();
    for (key, scale) in rows {
        let slot = seen
            .entry((&key.machine, &key.machine_quality, &key.config, &key.recipe))
            .or_insert(0.0);
        *slot = slot.max(*scale);
    }
    seen.values().sum()
}

fn row_modifiers(key: &RowKey, registry: &Registry, settings: &Settings) -> Option<Modifiers> {
    modifiers(key, registry, settings, Some(registry.recipe(key.recipe)))
}

/// A machine's own modifiers, with no recipe in play (§6.6).
pub fn machine_modifiers(
    machine: &super::value::Machine,
    registry: &Registry,
    settings: &Settings,
) -> Option<Modifiers> {
    let key = RowKey {
        machine: machine.entry,
        machine_quality: machine.quality,
        config: machine.config.clone(),
        recipe: hactorio_data::RecipeId::from_index(0),
        recipe_quality: Quality::Normal,
    };
    modifiers(&key, registry, settings, None)
}

/// Watts drawn by every machine of the process (§6.6).
pub fn power(
    process: &super::value::Process,
    registry: &Registry,
    settings: &Settings,
) -> Option<f64> {
    let machine_seconds = process.extent == super::value::Extent::MachineSeconds;
    process
        .rows
        .iter()
        .map(|(key, scale)| {
            let modifiers = row_modifiers(key, registry, settings)?;
            Some(draw(
                &modifiers,
                *scale,
                settings.idle_power,
                machine_seconds,
            ))
        })
        .sum()
}

/// Active draw plus idle draw for `scale` machines (§6.3).
///
/// A machine-seconds process carries no machine count, so its idle draw always
/// follows the fractional scale.
pub fn draw(modifiers: &Modifiers, scale: f64, idle: IdlePower, machine_seconds: bool) -> f64 {
    let active = modifiers.power_w * scale;
    let idle = match idle {
        IdlePower::Ignored => 0.0,
        IdlePower::Fractional => modifiers.idle_usage_w * scale,
        IdlePower::RoundedUp if machine_seconds => modifiers.idle_usage_w * scale,
        IdlePower::RoundedUp => modifiers.idle_usage_w * whole_machines(scale),
    };
    active + idle
}

/// The machine count, rounded up unless it is already an integer.
fn whole_machines(count: f64) -> f64 {
    if crate::approx::is_integer(count) {
        count.round()
    } else {
        count.ceil()
    }
}

/// Pollution per minute, or the total pollution of a machine-seconds process.
pub fn pollution(
    process: &super::value::Process,
    registry: &Registry,
    settings: &Settings,
    total: bool,
) -> Option<f64> {
    let per_minute: f64 = process
        .rows
        .iter()
        .map(|(key, scale)| {
            Some(row_modifiers(key, registry, settings)?.pollution_per_minute * scale)
        })
        .sum::<Option<f64>>()?;
    Some(if total { per_minute / 60.0 } else { per_minute })
}

/// The productivity multiplier every row shares (§6.6).
pub fn productivity(
    process: &super::value::Process,
    registry: &Registry,
    settings: &Settings,
) -> Option<f64> {
    let mut shared: Option<f64> = None;
    for key in process.rows.keys() {
        let multiplier = 1.0 + row_modifiers(key, registry, settings)?.productivity;
        match shared {
            Some(existing) if (existing - multiplier).abs() > super::value::APPROX_ZERO => {
                return None;
            }
            _ => shared = Some(multiplier),
        }
    }
    shared
}

/// Which rows of a process to total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rows {
    Inputs,
    Outputs,
    Net,
}

/// How a product splits across qualities (§6.3).
///
/// The first roll upgrades by one level with the machine's quality chance; every
/// further level is a constant 10% roll. The share that reaches at least `n`
/// levels is therefore `Q · 0.1^(n-1)`, and the recipe's own quality takes
/// `1 - Q`. Only qualities up to `max` are part of the distribution: whatever
/// would have rolled past the highest researched quality stays at that quality.
/// A recipe already at the highest quality has nowhere to go, so it stays put.
pub fn quality_shares(base: Quality, chance: f64, max: Quality) -> Vec<(Quality, f64)> {
    let levels = Quality::ALL;
    let Some(base_index) = levels.iter().position(|quality| *quality == base) else {
        return vec![(base, 1.0)];
    };
    let chance = chance.clamp(0.0, 1.0);
    let top_index = levels
        .iter()
        .position(|quality| *quality == max)
        .unwrap_or(levels.len() - 1)
        .max(base_index);
    let above = &levels[base_index + 1..top_index + 1];
    let mut shares = vec![(base, if above.is_empty() { 1.0 } else { 1.0 - chance })];
    for (offset, quality) in above.iter().enumerate() {
        let level = offset + 1;
        let at_least =
            |level: usize| chance * 0.1_f64.powi(i32::try_from(level - 1).unwrap_or(i32::MAX));
        // The top tier keeps everything that reached it; a middle tier keeps
        // only what did not roll one level further.
        let share = if level == above.len() {
            at_least(level)
        } else {
            at_least(level) - at_least(level + 1)
        };
        if share > 0.0 {
            shares.push((*quality, share));
        }
    }
    shares
}

/// The net rows of a set of rows: outputs minus inputs, before any assignment
/// adjustment (§5.4).
pub fn net(
    rows: &BTreeMap<super::value::RowKey, f64>,
    registry: &Registry,
    settings: &Settings,
) -> Option<BTreeMap<Resource, f64>> {
    let mut total: BTreeMap<Resource, f64> = BTreeMap::new();
    for (key, scale) in rows {
        let rates = row_rates(key, registry, settings)?;
        for (resource, amount) in &rates.outputs {
            *total.entry(*resource).or_insert(0.0) += amount * scale;
        }
        for (resource, amount) in &rates.inputs {
            *total.entry(*resource).or_insert(0.0) -= amount * scale;
        }
    }
    total.retain(|_, amount| !super::value::is_zero(*amount));
    Some(total)
}

/// A process's net rows, or the part of them it produces or consumes (§5.4).
pub fn totals(process: &super::value::Process, which: Rows) -> BTreeMap<Resource, f64> {
    match which {
        Rows::Net => process.net.clone(),
        Rows::Inputs => process
            .net
            .iter()
            .filter(|(_, amount)| amount.is_sign_negative())
            .map(|(resource, amount)| (*resource, -amount))
            .collect(),
        Rows::Outputs => process
            .net
            .iter()
            .filter(|(_, amount)| amount.is_sign_positive())
            .map(|(resource, amount)| (*resource, *amount))
            .collect(),
    }
}

/// The craft rate or craft total of a process (§6.4).
pub fn crafts(
    process: &super::value::Process,
    registry: &Registry,
    settings: &Settings,
) -> Option<f64> {
    process
        .rows
        .iter()
        .map(|(key, scale)| Some(row_rates(key, registry, settings)?.craft_per_second * scale))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::quality_shares;
    use crate::eval::value::Quality;

    /// The quality chance of four quality module 3s, matching
    /// `1 craft in :assembling-machine-3:4:quality-module-3:`.
    const CHANCE: f64 = 0.1;

    fn assert_split(shares: &[(Quality, f64)], expected: &[(Quality, f64)]) {
        assert_eq!(shares.len(), expected.len(), "{shares:?}");
        for (quality, expected) in expected {
            let actual = shares
                .iter()
                .find(|(share_quality, _)| share_quality == quality)
                .map_or(0.0, |(_, share)| *share);
            assert!(
                (actual - expected).abs() < 1e-12,
                "{}: {actual} != {expected}",
                quality.name()
            );
        }
        let total: f64 = shares.iter().map(|(_, share)| share).sum();
        assert!(
            (total - 1.0).abs() < 1e-12,
            "the shares must total 1: {total}"
        );
    }

    #[test]
    fn max_quality_legendary_uses_every_quality() {
        let shares = quality_shares(Quality::Normal, CHANCE, Quality::Legendary);
        assert_split(
            &shares,
            &[
                (Quality::Normal, 0.9),
                (Quality::Uncommon, 0.09),
                (Quality::Rare, 0.009),
                (Quality::Epic, 0.0009),
                (Quality::Legendary, 0.0001),
            ],
        );
    }

    #[test]
    fn max_quality_epic_folds_legendary_into_epic() {
        let shares = quality_shares(Quality::Normal, CHANCE, Quality::Epic);
        assert_split(
            &shares,
            &[
                (Quality::Normal, 0.9),
                (Quality::Uncommon, 0.09),
                (Quality::Rare, 0.009),
                (Quality::Epic, 0.001),
            ],
        );
    }

    #[test]
    fn max_quality_rare_folds_epic_and_legendary_into_rare() {
        let shares = quality_shares(Quality::Normal, CHANCE, Quality::Rare);
        assert_split(
            &shares,
            &[
                (Quality::Normal, 0.9),
                (Quality::Uncommon, 0.09),
                (Quality::Rare, 0.01),
            ],
        );
    }

    #[test]
    fn a_recipe_at_the_max_quality_has_nowhere_to_go() {
        let shares = quality_shares(Quality::Rare, CHANCE, Quality::Rare);
        assert_split(&shares, &[(Quality::Rare, 1.0)]);
    }
}
