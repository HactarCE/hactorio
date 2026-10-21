//! Units, parser, evaluator, and settings for hactorio.
//!
//! This crate has no dependency on egui and is testable headlessly.

mod belt;
mod format;
mod settings;
mod value;

pub use belt::{BeltTier, belt_capacity, belt_value, stack_size};
pub use format::format_value;
pub use settings::{ItemRateDisplay, Settings, TimeUnit};
pub use value::{BaseUnit, Value, ValueError};
