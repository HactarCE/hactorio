//! Per-line parse cache.

use std::collections::{HashMap, HashSet};

use hactorio_data::Registry;

use crate::ast::Statement;
use crate::parser::{ParseError, parse_line};

/// Caches parsed lines by their text.
///
/// The cache stores syntax only, never evaluated values or settings snapshots,
/// so changing settings never invalidates it. Game-data reloads must clear it.
#[derive(Debug, Default)]
pub struct LineCache {
    statements: HashMap<Box<str>, Result<Statement, ParseError>>,
}

impl LineCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drops every entry. Call this when the registry changes.
    pub fn clear(&mut self) {
        self.statements.clear();
    }

    pub fn len(&self) -> usize {
        self.statements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }

    pub(crate) fn statement(
        &mut self,
        line: &str,
        registry: &Registry,
    ) -> Result<Statement, ParseError> {
        if let Some(cached) = self.statements.get(line) {
            return cached.clone();
        }
        let parsed = parse_line(line, registry);
        self.statements.insert(line.into(), parsed.clone());
        parsed
    }

    /// Drops entries not seen during the last pass.
    pub(crate) fn retain(&mut self, seen: &HashSet<&str>) {
        self.statements.retain(|key, _| seen.contains(&**key));
    }
}
