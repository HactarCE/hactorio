//! Belt-rate literals. The belt rate is sugar for an item-per-second rate.

use hactorio_data::EntryId;
use serde::{Deserialize, Serialize};

use crate::value::{BaseUnit, Value};

/// The four belt tiers whose throughput is fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeltTier {
    Yellow,
    Red,
    Blue,
    Green,
}

impl BeltTier {
    /// The tiers from slowest to fastest.
    pub const ALL: [BeltTier; 4] = [
        BeltTier::Yellow,
        BeltTier::Red,
        BeltTier::Blue,
        BeltTier::Green,
    ];

    pub fn items_per_second(self) -> f64 {
        match self {
            BeltTier::Yellow => 15.0,
            BeltTier::Red => 30.0,
            BeltTier::Blue => 45.0,
            BeltTier::Green => 60.0,
        }
    }

    /// The display token for an unstacked belt rate.
    pub fn token(self) -> &'static str {
        match self {
            BeltTier::Yellow => "yb",
            BeltTier::Red => "rb",
            BeltTier::Blue => "bb",
            BeltTier::Green => "gb",
        }
    }

    /// The display token for a stacked belt rate.
    pub fn stacked_token(self) -> &'static str {
        match self {
            BeltTier::Yellow => "syb",
            BeltTier::Red => "srb",
            BeltTier::Blue => "sbb",
            BeltTier::Green => "sgb",
        }
    }
}

/// Effective stack size for a stacked belt.
///
/// Belt stacking is `min(stacking level, item stack size)`.
pub fn stack_size(stacking_level: u8, item_stack_size: u32) -> u32 {
    u32::from(stacking_level.clamp(1, 4)).min(item_stack_size.max(1))
}

/// Items per second carried by one belt.
pub fn belt_capacity(
    tier: BeltTier,
    stacked: bool,
    item_stack_size: u32,
    stacking_level: u8,
) -> f64 {
    let stacks = if stacked {
        f64::from(stack_size(stacking_level, item_stack_size))
    } else {
        1.0
    };
    tier.items_per_second() * stacks
}

/// `count` belts of `tier`, carrying `item`.
///
/// This is the evaluation of a parsed belt literal, so stacking is applied here
/// rather than during parsing.
pub fn belt_value(
    count: f64,
    tier: BeltTier,
    stacked: bool,
    item: EntryId,
    item_stack_size: u32,
    stacking_level: u8,
) -> Value {
    let capacity = belt_capacity(tier, stacked, item_stack_size, stacking_level);
    Value::new(
        count * capacity,
        [(BaseUnit::Item(item), 1), (BaseUnit::Second, -1)],
    )
}
