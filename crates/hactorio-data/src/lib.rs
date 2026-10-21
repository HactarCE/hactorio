//! Game prototypes: dumps, post-processed cache, and icons.
//!
//! The registry is the single namespace the language resolves tokens against:
//! items, fluids, modules, and recipes, with one private-use codepoint per
//! namespace entry.

mod cache;
mod postprocess;
mod registry;

use thiserror::Error;

pub use postprocess::{postprocess, Locale};
pub use registry::{
    AllowedEffects, Entry, EntryId, Ingredient, Item, Machine, Module, ModuleEffect,
    Product, PrototypeKind, Recipe, RecipeId, Registry, PUA_COUNT, PUA_FIRST, glyph_for,
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
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
