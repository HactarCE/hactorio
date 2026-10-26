use std::collections::BTreeMap;

use hactorio_data::{EntryId, ModuleEffect, Registry};

use super::{Result, ValueError};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Config {
    /// Modules inserted into the machine.
    modules: Modules,
    /// Beacon groups.
    beacons: BTreeMap<Modules, u32>,
}

impl Config {
    pub fn new(modules: Modules) -> Self {
        Self {
            modules,
            beacons: BTreeMap::new(),
        }
    }

    pub fn scale(&self, scalar: f64) -> Result<Self> {
        // limit to u8 to avoid unreasonable O(n) algorithm
        let scalar =
            crate::approx::to_u8(scalar).ok_or(ValueError::ExpectedSmallInteger(scalar))? as usize;
        Ok(Self {
            modules: self.modules.scale(scalar),
            beacons: self
                .beacons
                .iter()
                .map(|(k, v)| {
                    Ok((
                        k.clone(),
                        v.checked_mul(scalar as u32).ok_or(ValueError::Overflow)?,
                    ))
                })
                .collect::<Result<_>>()?,
        })
    }
}

/// Sorted list of modules.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Modules(Vec<EntryId>);

impl Modules {
    pub fn new(modules: impl IntoIterator<Item = EntryId>) -> Self {
        let mut modules: Vec<_> = modules.into_iter().collect();
        modules.sort();
        Self(modules)
    }

    pub fn scale(&self, scalar: usize) -> Self {
        Self::new(
            self.iter()
                .flat_map(|m| std::iter::repeat_n(m, scalar as usize)),
        )
    }

    pub fn add(self, other: &Self) -> Self {
        Self::new(std::iter::chain(self.iter(), other.iter()))
    }

    pub fn iter(&self) -> impl Iterator<Item = EntryId> {
        self.0.iter().copied()
    }

    pub fn effects(&self, registry: &Registry) -> Result<ModuleEffect> {
        self.iter()
            .map(|id| {
                let entry = registry.entry_by_index(id);
                Ok(entry
                    .module
                    .as_ref()
                    .ok_or(ValueError::ExpectedModule(entry.id.clone()))?
                    .effect)
            })
            .sum()
    }
}
