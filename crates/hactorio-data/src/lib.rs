//! Game prototypes: dumps, post-processed cache, and icons.
//!
//! The registry is the single namespace the language resolves tokens against:
//! items, fluids, modules, and recipes, with one private-use codepoint per
//! namespace entry.

mod cache;
mod dump;
mod icons;
mod postprocess;
mod registry;

use thiserror::Error;

pub use dump::{GameInstall, locate};
pub use icons::{
    EMPTY_GLYPH, IconImage, IconSource, SIGNALS, STACK_SIZE_GLYPH, STACK_SIZE_ICON, SignalIcon,
    VARIABLE_GLYPH, signal_by_glyph, signal_by_id,
};
pub use postprocess::{Locale, postprocess};
pub use registry::{
    AllowedEffects, Beacon, Belt, Container, Entry, EntryId, Equipment, Ingredient, Inserter, Item,
    Machine, Module, ModuleEffect, PUA_COUNT, PUA_FIRST, PUA_LAST, Product, ProductivityResearch,
    PrototypeKind, Pump, Recipe, RecipeId, RecipeProductivity, Registry, glyph_for,
};

/// Errors from loading, validating, or post-processing prototype data.
#[derive(Debug, Error)]
pub enum Error {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported cache version {0}")]
    UnsupportedVersion(u32),
    #[error("invalid registry: {0}")]
    Invalid(String),
    #[error("game data: {0}")]
    GameData(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
