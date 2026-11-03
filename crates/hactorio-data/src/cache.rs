//! On-disk cache of the post-processed registry.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::registry::{Entry, InserterResearch, ProductivityResearch, Recipe, Registry};
use crate::{Error, Result};

/// Bumping this invalidates every existing cache.
pub(crate) const CACHE_VERSION: u32 = 9;

#[derive(Serialize)]
struct CacheRef<'a> {
    version: u32,
    entries: &'a [Entry],
    recipes: &'a [Recipe],
    researches: &'a [ProductivityResearch],
    inserter_research: &'a InserterResearch,
    belt_stacking: &'a [(Box<str>, u32)],
}

#[derive(Deserialize)]
struct CacheFile {
    version: u32,
    entries: Vec<Entry>,
    recipes: Vec<Recipe>,
    #[serde(default)]
    researches: Vec<ProductivityResearch>,
    #[serde(default)]
    inserter_research: InserterResearch,
    #[serde(default)]
    belt_stacking: Vec<(Box<str>, u32)>,
}

impl Registry {
    /// Serializes the registry to its cache representation.
    fn to_json(&self) -> Result<Vec<u8>> {
        let cache = CacheRef {
            version: CACHE_VERSION,
            entries: self.entries(),
            recipes: self.recipes(),
            researches: self.productivity_researches(),
            inserter_research: self.inserter_research(),
            belt_stacking: self.belt_stacking_research(),
        };
        Ok(serde_json::to_vec(&cache)?)
    }

    fn from_json(bytes: &[u8]) -> Result<Self> {
        let cache: CacheFile = serde_json::from_slice(bytes)?;
        if cache.version != CACHE_VERSION {
            return Err(Error::UnsupportedVersion(cache.version));
        }
        Registry::from_parts(
            cache.entries,
            cache.recipes,
            cache.researches,
            cache.inserter_research,
            cache.belt_stacking,
        )
    }

    /// Loads a cache written by [`Registry::save`].
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        Self::from_json(&bytes)
    }

    /// Writes the cache, replacing `path` only once the new contents are complete.
    pub fn save(&self, path: &Path) -> Result<()> {
        let bytes = self.to_json()?;
        let temp = path.with_extension("tmp");
        fs::write(&temp, &bytes)?;
        fs::rename(&temp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Locale, postprocess};

    const DUMP: &str = include_str!("../tests/fixtures/dump.json");

    fn registry() -> Registry {
        postprocess(DUMP, &Locale::empty()).unwrap()
    }

    #[test]
    fn cache_round_trips_through_bytes() {
        let registry = registry();
        let bytes = registry.to_json().unwrap();
        let loaded = Registry::from_json(&bytes).unwrap();

        assert_eq!(registry.entries(), loaded.entries());
        assert_eq!(registry.recipes(), loaded.recipes());
        assert_eq!(
            registry.productivity_researches(),
            loaded.productivity_researches()
        );
        assert_eq!(loaded.to_json().unwrap(), bytes);
        assert_eq!(
            loaded
                .entry("assembling-machine-3")
                .unwrap()
                .machine()
                .unwrap()
                .module_slots,
            4
        );
        assert_eq!(
            loaded.reference_count(loaded.entry_id("iron-ore").unwrap()),
            3
        );
    }

    #[test]
    fn save_and_load_round_trip() {
        let path = std::env::temp_dir().join(format!("hactorio-cache-{}.json", std::process::id()));
        let registry = registry();

        registry.save(&path).unwrap();
        let loaded = Registry::load(&path).unwrap();
        fs::remove_file(&path).unwrap();

        assert_eq!(registry.entries(), loaded.entries());
        assert_eq!(registry.recipes(), loaded.recipes());
        assert_eq!(
            registry.productivity_researches(),
            loaded.productivity_researches()
        );
    }

    #[test]
    fn unsupported_cache_versions_are_rejected() {
        let error =
            Registry::from_json(br#"{"version":99,"entries":[],"recipes":[]}"#).unwrap_err();
        assert!(matches!(error, Error::UnsupportedVersion(99)));
    }
}
