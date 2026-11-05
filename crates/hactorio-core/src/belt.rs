//! Belt tiers for the belt-count display setting (§7.8).

use serde::{Deserialize, Serialize};

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

    /// The item whose icon represents this belt tier.
    pub fn item_id(self) -> &'static str {
        match self {
            BeltTier::Yellow => "transport-belt",
            BeltTier::Red => "fast-transport-belt",
            BeltTier::Blue => "express-transport-belt",
            BeltTier::Green => "turbo-transport-belt",
        }
    }

    pub fn items_per_second(self) -> f64 {
        match self {
            BeltTier::Yellow => 15.0,
            BeltTier::Red => 30.0,
            BeltTier::Blue => 45.0,
            BeltTier::Green => 60.0,
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
