//! Global settings. Evaluation takes a snapshot, never the UI's copy.

use serde::{Deserialize, Serialize};

use crate::belt::BeltTier;
use crate::value::Value;

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

    /// The value of `count` of this unit, folded into seconds.
    pub fn value(self, count: f64) -> Value {
        match self {
            TimeUnit::Second => Value::seconds(count),
            TimeUnit::Minute => Value::minutes(count),
            TimeUnit::Hour => Value::hours(count),
        }
    }
}

/// How item rates are displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemRateDisplay {
    PerSecond,
    Yb,
    Rb,
    Bb,
    Gb,
    Syb,
    Srb,
    Sbb,
    Sgb,
}

impl ItemRateDisplay {
    /// The belt tier and stacking this display mode uses, if it is a belt mode.
    pub fn belt(self) -> Option<(BeltTier, bool)> {
        match self {
            ItemRateDisplay::PerSecond => None,
            ItemRateDisplay::Yb => Some((BeltTier::Yellow, false)),
            ItemRateDisplay::Rb => Some((BeltTier::Red, false)),
            ItemRateDisplay::Bb => Some((BeltTier::Blue, false)),
            ItemRateDisplay::Gb => Some((BeltTier::Green, false)),
            ItemRateDisplay::Syb => Some((BeltTier::Yellow, true)),
            ItemRateDisplay::Srb => Some((BeltTier::Red, true)),
            ItemRateDisplay::Sbb => Some((BeltTier::Blue, true)),
            ItemRateDisplay::Sgb => Some((BeltTier::Green, true)),
        }
    }

    /// The belt token, for belt display modes.
    pub fn token(self) -> Option<&'static str> {
        match self {
            ItemRateDisplay::PerSecond => None,
            ItemRateDisplay::Yb => Some("yb"),
            ItemRateDisplay::Rb => Some("rb"),
            ItemRateDisplay::Bb => Some("bb"),
            ItemRateDisplay::Gb => Some("gb"),
            ItemRateDisplay::Syb => Some("syb"),
            ItemRateDisplay::Srb => Some("srb"),
            ItemRateDisplay::Sbb => Some("sbb"),
            ItemRateDisplay::Sgb => Some("sgb"),
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
    pub item_rate_display: ItemRateDisplay,
    /// Effective belt stacking level, 1 through 4.
    pub belt_stacking: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_decimals: 3,
            metric_prefixes: true,
            time_unit: TimeUnit::Second,
            item_rate_display: ItemRateDisplay::PerSecond,
            belt_stacking: 4,
        }
    }
}
