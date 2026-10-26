use std::{collections::BTreeMap, hash::Hash};

use hactorio_data::EntryId;

use super::{Config, Recipe, Resource};

#[derive(Debug, Clone)]
pub struct Process {
    kind: ProcessExtent,
    rates: BTreeMap<MachineMakingRecipe, f64>,
}

impl Process {
    pub fn scale(&self, scalar: f64) -> Self {
        Self {
            kind: self.kind,
            rates: self
                .rates
                .iter()
                .map(|(k, &v)| (k.clone(), v * scalar))
                .collect(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ProcessExtent {
    Machines,
    MachineSeconds,
}

#[derive(Debug, Clone)]
pub struct MachineMakingRecipe {
    machine: MachineWithConfig,
    recipe: Recipe,

    /// Input resources per second per machine.
    ///
    /// Computed from `machine` and `recipe`; not used for
    /// hash/equality/comparison.
    inputs: BTreeMap<Resource, f64>,
    /// Output resources per second per machine.
    ///
    /// Computed from `machine` and `recipe`; not used for
    /// hash/equality/comparison.
    outputs: BTreeMap<Resource, f64>,
}

impl Hash for MachineMakingRecipe {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.machine.hash(state);
        self.recipe.hash(state);
    }
}

impl PartialEq for MachineMakingRecipe {
    fn eq(&self, other: &Self) -> bool {
        self.machine == other.machine && self.recipe == other.recipe
    }
}
impl Eq for MachineMakingRecipe {}

impl PartialOrd for MachineMakingRecipe {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MachineMakingRecipe {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.machine, self.recipe).cmp(&(&other.machine, other.recipe))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MachineWithConfig {
    machine: EntryId,
    config: Config,
}
