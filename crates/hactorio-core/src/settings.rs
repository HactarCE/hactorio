//! Global settings. Evaluation takes a snapshot, never the UI's copy.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::belt::BeltTier;

/// The unit used to display time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeUnit {
    Second,
    Minute,
    Hour,
}

impl TimeUnit {
    pub fn seconds(self) -> f64 {
        match self {
            TimeUnit::Second => 1.0,
            TimeUnit::Minute => 60.0,
            TimeUnit::Hour => 3600.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            TimeUnit::Second => "second",
            TimeUnit::Minute => "minute",
            TimeUnit::Hour => "hour",
        }
    }

    /// The compact form used in outputs.
    pub fn short_name(self) -> &'static str {
        match self {
            TimeUnit::Second => "s",
            TimeUnit::Minute => "m",
            TimeUnit::Hour => "h",
        }
    }

    /// Resolves a time-unit word, including the `s`/`m`/`h` aliases.
    ///
    /// Matching is case-sensitive: `M` is a numeric suffix, not a minute.
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "second" | "s" => Some(TimeUnit::Second),
            "minute" | "m" => Some(TimeUnit::Minute),
            "hour" | "h" => Some(TimeUnit::Hour),
            _ => None,
        }
    }
}

/// Global display and evaluation settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Maximum decimal places to display.
    pub max_decimals: usize,
    /// Display item quantities and rates with `k`/`M`/`G`.
    pub metric_prefixes: bool,
    pub time_unit: TimeUnit,
    /// Belt tier used to display item rates, or `None` for items per second.
    pub item_rate_belt: Option<BeltTier>,
    /// Whether item rates in belt mode use stacked belts.
    pub stacked_belts: bool,
    /// Effective belt stacking level, 1 through 4.
    pub belt_stacking: u8,
    /// Inserter capacity research level.
    pub inserter_capacity: u32,
    /// Completed levels of each productivity research, keyed by technology id.
    #[serde(default)]
    pub research_levels: BTreeMap<Box<str>, u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_decimals: 3,
            metric_prefixes: true,
            time_unit: TimeUnit::Second,
            item_rate_belt: None,
            stacked_belts: false,
            belt_stacking: 4,
            inserter_capacity: 0,
            research_levels: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Completed levels of `research`, or zero if it was never set.
    pub fn research_level(&self, research: &str) -> u32 {
        self.research_levels.get(research).copied().unwrap_or(0)
    }
}
