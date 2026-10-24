//! Registry-backed construction of facet values from prototype tokens.

use std::collections::BTreeMap;

use hactorio_data::{EntryId, Registry};
use thiserror::Error;

use crate::facets::{
    BeaconGroup, BeltFacet, Cap, CapacityFacet, Config, FacetValue, MachineFacet, ModuleCount,
    Monomial, RateFacet,
};
use crate::settings::Settings;
use crate::value::BaseUnit;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TokenError {
    #[error("`{0}` has no token facets")]
    EmptyToken(Box<str>),
    #[error("inserter `{0}` has an invalid rotation speed")]
    InvalidInserter(Box<str>),
}

/// Builds the facets a bare prototype token denotes.
pub fn token_value(
    entry_id: EntryId,
    registry: &Registry,
    settings: &Settings,
) -> Result<FacetValue, TokenError> {
    let entry = registry.entry_by_index(entry_id);
    let mut value = if entry.fluid {
        FacetValue::item(Monomial::new(1.0, [(BaseUnit::Fluid(entry_id), 1)]))
    } else {
        FacetValue::item(Monomial::new(1.0, [(BaseUnit::Item(entry_id), 1)]))
    };

    value.recipe = entry.recipe;

    if let Some(machine) = entry.machine() {
        let _ = machine;
        value.machine = Some(MachineFacet {
            machine: entry_id,
            config: Config::default(),
            count: None,
        });
    }

    if entry.module.is_some() {
        value.config = Some(Config {
            modules: vec![ModuleCount {
                module: entry_id,
                count: 1,
            }],
            beacons: Vec::new(),
        });
    }

    if entry.beacon.is_some() {
        value.config = Some(Config {
            modules: Vec::new(),
            beacons: vec![BeaconGroup {
                beacon: entry_id,
                count: 1,
                modules: Vec::new(),
            }],
        });
    }

    if let Some(item) = &entry.item {
        if let Some(belt) = item.belt {
            value.belt = Some(BeltFacet {
                count: 1.0,
                cap: Cap::Limit(1),
            });
            value.rate = Some(RateFacet::Items(BTreeMap::from([(
                Cap::Limit(1),
                belt.speed * 60.0 * 8.0,
            )])));
        }

        if let Some(inserter) = item.inserter {
            let capacity = inserter_capacity(inserter, settings);
            let cycles = inserter_cycles(inserter)
                .ok_or_else(|| TokenError::InvalidInserter(entry.id.clone()))?;
            value.rate = Some(RateFacet::Items(BTreeMap::from([(
                Cap::Limit(capacity.max(1)),
                cycles,
            )])));
        }

        if let Some(container) = item.container {
            value.capacity = if container.fluid_capacity > 0.0 {
                Some(CapacityFacet::Fluid(container.fluid_capacity))
            } else if container.slots > 0 {
                Some(CapacityFacet::Items(BTreeMap::from([(
                    Cap::Full,
                    f64::from(container.slots),
                )])))
            } else {
                None
            };
        }

        if let Some(pump) = item.pump {
            value.capacity = None;
            value.rate = Some(RateFacet::Fluid(pump.pumping_speed * 60.0));
        }
    }

    if value.is_empty() {
        return Err(TokenError::EmptyToken(entry.id.clone()));
    }
    Ok(value)
}

fn inserter_capacity(inserter: hactorio_data::Inserter, settings: &Settings) -> u32 {
    let bonus = settings.inserter_capacity;
    if inserter.stack_size_bonus > 0 {
        2 + inserter.stack_size_bonus + bonus
    } else if inserter.bulk {
        2 + bonus
    } else {
        1 + bonus
    }
}

fn inserter_cycles(inserter: hactorio_data::Inserter) -> Option<f64> {
    let rotation_speed = inserter.rotation_speed;
    if !(rotation_speed > 0.0 && rotation_speed < 0.5) {
        return None;
    }
    let ticks_half = (0.5 / rotation_speed).ceil() - 1.0;
    let cycle_ticks = 2.0 * ticks_half;
    Some(60.0 / cycle_ticks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hactorio_data::{Locale, postprocess};

    const DUMP: &str = r#"{
      "item": {
        "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100},
        "transport-belt": {"type": "item", "name": "transport-belt", "stack_size": 100},
        "inserter": {"type": "item", "name": "inserter", "stack_size": 50},
        "steel-chest": {"type": "item", "name": "steel-chest", "stack_size": 50},
        "storage-tank": {"type": "item", "name": "storage-tank", "stack_size": 50},
        "speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}},
        "beacon": {"type": "item", "name": "beacon", "stack_size": 20}
      },
      "fluid": {},
      "module": {
        "speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}}
      },
      "recipe": {
        "iron-plate": {"type": "recipe", "name": "iron-plate", "categories": ["smelting"], "energy_required": 3.2, "results": [{"type": "item", "name": "iron-plate", "amount": 1}]}
      },
      "transport-belt": {
        "transport-belt": {"type": "transport-belt", "name": "transport-belt", "speed": 0.03125}
      },
      "inserter": {
        "inserter": {"type": "inserter", "name": "inserter", "rotation_speed": 0.014, "bulk": false}
      },
      "container": {
        "steel-chest": {"type": "container", "name": "steel-chest", "inventory_size": 48}
      },
      "storage-tank": {
        "storage-tank": {"type": "storage-tank", "name": "storage-tank", "fluid_box": {"volume": 25000}}
      },
      "beacon": {
        "beacon": {"type": "beacon", "name": "beacon", "module_slots": 2, "distribution_effectivity": 1.5, "profile": [1.0], "energy_usage": "480kW"}
      }
    }"#;

    fn registry() -> Registry {
        postprocess(DUMP, &Locale::empty()).unwrap()
    }

    fn entry_id(registry: &Registry, id: &str) -> EntryId {
        registry.entry_id(id).unwrap()
    }

    #[test]
    fn item_token_keeps_recipe() {
        let registry = registry();
        let value = token_value(
            entry_id(&registry, "iron-plate"),
            &registry,
            &Settings::default(),
        )
        .unwrap();
        assert!(value.recipe.is_some());
        assert_eq!(value.item.unwrap().coef(), 1.0);
    }

    #[test]
    fn belt_token_has_item_belt_and_rate() {
        let registry = registry();
        let value = token_value(
            entry_id(&registry, "transport-belt"),
            &registry,
            &Settings::default(),
        )
        .unwrap();
        assert!(value.belt.is_some());
        let RateFacet::Items(items) = value.rate.unwrap() else {
            panic!("expected item rate");
        };
        assert_eq!(items[&Cap::Limit(1)], 15.0);
    }

    #[test]
    fn inserter_token_has_rate() {
        let registry = registry();
        let value = token_value(
            entry_id(&registry, "inserter"),
            &registry,
            &Settings::default(),
        )
        .unwrap();
        assert!(matches!(value.rate, Some(RateFacet::Items(_))));
    }

    #[test]
    fn container_token_has_capacity() {
        let registry = registry();
        let value = token_value(
            entry_id(&registry, "steel-chest"),
            &registry,
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(
            value.capacity,
            Some(CapacityFacet::Items(BTreeMap::from([(Cap::Full, 48.0)])))
        );
    }

    #[test]
    fn fluid_container_uses_volume() {
        let registry = registry();
        let value = token_value(
            entry_id(&registry, "storage-tank"),
            &registry,
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(value.capacity, Some(CapacityFacet::Fluid(25000.0)));
    }

    #[test]
    fn beacon_token_starts_one_empty_group() {
        let registry = registry();
        let value = token_value(
            entry_id(&registry, "beacon"),
            &registry,
            &Settings::default(),
        )
        .unwrap();
        let config = value.config.unwrap();
        assert_eq!(config.beacons.len(), 1);
        assert!(config.beacons[0].modules.is_empty());
    }
}
